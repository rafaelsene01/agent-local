use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct DbState(pub Mutex<Option<Connection>>);

const MIGRATION_1_INITIAL: &str = "
CREATE TABLE IF NOT EXISTS chats (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (chat_id) REFERENCES chats(id)
);

CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);

CREATE TABLE IF NOT EXISTS connections (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    base_url TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_configs (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    context_length INTEGER,
    gpu_offload TEXT,
    is_active INTEGER NOT NULL DEFAULT 0,
    UNIQUE(connection_id, model_name)
);

CREATE INDEX IF NOT EXISTS idx_model_configs_connection ON model_configs(connection_id);
";

/// Only one connection may be active at a time (AD-021). The rename carries
/// the previously enabled flags over, and the normalization collapses any
/// pre-existing "several enabled" state down to the oldest one. The keeper is
/// snapshotted into a temp table first so the UPDATE cannot depend on when
/// SQLite evaluates the subquery relative to the rows it is rewriting.
const MIGRATION_2_SINGLE_ACTIVE_CONNECTION: &str = "
ALTER TABLE connections RENAME COLUMN enabled TO is_active;

CREATE TEMP TABLE _keep_active AS
    SELECT id FROM connections WHERE is_active = 1 ORDER BY created_at ASC, id ASC LIMIT 1;

UPDATE connections SET is_active = 0
    WHERE is_active = 1 AND id NOT IN (SELECT id FROM _keep_active);

DROP TABLE _keep_active;
";

/// What the embedded runtime needs lives in its own singleton row instead of
/// columns on `connections`, which only one provider would ever use.
const MIGRATION_3_EMBEDDED_RUNTIME: &str = "
CREATE TABLE IF NOT EXISTS embedded_runtime (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    release_tag TEXT,
    backend TEXT,
    binary_path TEXT,
    model_path TEXT,
    context_length INTEGER,
    gpu_layers INTEGER
);
";

/// `status` is the document's position in the parse → chunk → embed pipeline;
/// only `ready` documents are searchable, so a crash mid-processing leaves a
/// row that can be re-queued instead of a half-indexed document.
const MIGRATION_4_DOCUMENTS: &str = "
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(status);
";

/// Attachments live per chat and die with it (AD-004): the files sit under
/// `chats/<id>/tmp/` and their vectors under the `chat:<id>` namespace.
/// `injected_whole` is a terminal status for files small enough to go into the
/// prompt verbatim, which skips chunking and embedding entirely.
const MIGRATION_5_CHAT_ATTACHMENTS: &str = "
ALTER TABLE chats ADD COLUMN use_global_rag INTEGER NOT NULL DEFAULT 1;

CREATE TABLE IF NOT EXISTS chat_attachments (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL,
    message_id TEXT,
    filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    status TEXT NOT NULL,
    extracted_text TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_attachments_chat_id ON chat_attachments(chat_id);
";

/// Ordered list of schema versions. A migration is applied only when
/// `PRAGMA user_version` is below its number, which is what makes a column
/// change reach databases that already exist on disk — `CREATE TABLE IF NOT
/// EXISTS` alone would silently no-op there.
const MIGRATIONS: &[(u32, &str)] = &[
    (1, MIGRATION_1_INITIAL),
    (2, MIGRATION_2_SINGLE_ACTIVE_CONNECTION),
    (3, MIGRATION_3_EMBEDDED_RUNTIME),
    (4, MIGRATION_4_DOCUMENTS),
    (5, MIGRATION_5_CHAT_ATTACHMENTS),
];

fn user_version(conn: &Connection) -> Result<u32, String> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())
}

/// `PRAGMA user_version` does not accept bound parameters, hence the format!.
/// The value is a `u32` we control, never user input.
pub fn apply_migrations(conn: &mut Connection) -> Result<(), String> {
    let current = user_version(conn)?;

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql).map_err(|e| e.to_string())?;
        tx.pragma_update(None, "user_version", *version)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// The database only exists after the user finishes onboarding (AD-011), so
/// every command has to answer "is there a database yet?" the same way.
pub fn require_conn<'a>(
    guard: &'a std::sync::MutexGuard<'a, Option<Connection>>,
) -> Result<&'a Connection, String> {
    guard
        .as_ref()
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())
}

pub fn open(db_file: &Path) -> Result<Connection, String> {
    let mut conn = Connection::open(db_file).map_err(|e| e.to_string())?;
    apply_migrations(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated_in_memory() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();
        conn
    }

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn open_creates_connections_and_model_configs_tables() {
        let conn = migrated_in_memory();
        let tables = table_names(&conn);

        assert!(tables.contains(&"connections".to_string()));
        assert!(tables.contains(&"model_configs".to_string()));
    }

    #[test]
    fn fresh_database_reaches_latest_version() {
        let conn = migrated_in_memory();
        let latest = MIGRATIONS.last().unwrap().0;
        assert_eq!(user_version(&conn).unwrap(), latest);

        let tables = table_names(&conn);
        for expected in ["chats", "messages", "connections", "model_configs"] {
            assert!(tables.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn fresh_database_uses_is_active_column() {
        let conn = migrated_in_memory();
        let mut stmt = conn.prepare("PRAGMA table_info(connections)").unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(columns.contains(&"is_active".to_string()));
        assert!(!columns.contains(&"enabled".to_string()));
    }

    #[test]
    fn migrating_a_v1_database_keeps_only_the_oldest_enabled_connection() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_1_INITIAL).unwrap();
        conn.pragma_update(None, "user_version", 1u32).unwrap();
        conn.execute_batch(
            "INSERT INTO connections (id, provider, base_url, enabled, created_at) VALUES
                ('a', 'ollama', 'http://localhost:11434', 1, '2026-07-01T00:00:00Z'),
                ('b', 'lmstudio', 'http://localhost:1234', 1, '2026-07-02T00:00:00Z');",
        )
        .unwrap();

        apply_migrations(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), MIGRATIONS.last().unwrap().0);
        let active: Vec<String> = conn
            .prepare("SELECT id FROM connections WHERE is_active = 1")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(active, vec!["a".to_string()]);

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM connections", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, 2, "migration must not drop existing connections");
    }

    #[test]
    fn migrating_a_v2_database_adds_embedded_runtime_without_losing_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_1_INITIAL).unwrap();
        conn.execute_batch(MIGRATION_2_SINGLE_ACTIVE_CONNECTION).unwrap();
        conn.pragma_update(None, "user_version", 2u32).unwrap();
        conn.execute_batch(
            "INSERT INTO connections (id, provider, base_url, is_active, created_at)
                VALUES ('a', 'ollama', 'http://localhost:11434', 1, '2026-07-01T00:00:00Z');",
        )
        .unwrap();

        apply_migrations(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), MIGRATIONS.last().unwrap().0);
        assert!(table_names(&conn).contains(&"embedded_runtime".to_string()));
        let kept: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM connections WHERE id = 'a' AND is_active = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(kept, 1);
    }

    #[test]
    fn embedded_runtime_row_is_a_singleton() {
        let conn = migrated_in_memory();
        conn.execute("INSERT INTO embedded_runtime (id, backend) VALUES (1, 'cpu')", [])
            .unwrap();

        let second = conn.execute("INSERT INTO embedded_runtime (id, backend) VALUES (2, 'cpu')", []);

        assert!(second.is_err(), "CHECK (id = 1) must reject a second row");
    }

    #[test]
    fn applying_migrations_twice_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();
        let version_after_first = user_version(&conn).unwrap();
        let tables_after_first = table_names(&conn);

        apply_migrations(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), version_after_first);
        assert_eq!(table_names(&conn), tables_after_first);
    }
}
