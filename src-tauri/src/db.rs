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

/// Which vector namespace a document's chunks belong to.
///
/// Without this column, every row in `documents` looked global — including the
/// temporary row a large chat attachment creates while it is being indexed
/// (see `chat::attachments`). A crash during that window left the row behind,
/// and the boot-time requeue re-indexed a private attachment into the **global**
/// knowledge base, where every chat could retrieve it (CHAT-11). The default is
/// `global` because that is what every pre-existing row is.
const MIGRATION_6_DOCUMENT_NAMESPACE: &str = "
ALTER TABLE documents ADD COLUMN namespace TEXT NOT NULL DEFAULT 'global';
";

/// There is one runtime now, so there is nothing to connect to and nothing to
/// disambiguate: `embedded_runtime` holds the model, the context length and the
/// GPU setting, and `connections`/`model_configs` have no reader left (SELF-06).
///
/// Dropped in this order because `model_configs.connection_id` references
/// `connections(id)` — and foreign keys are enforced now, so the reverse order
/// would fail rather than silently succeed.
///
/// **Numbered 7, not 6 as the plan said:** migration 6 was already spent on the
/// `documents.namespace` column (AD-040), which landed first.
const MIGRATION_7_SINGLE_RUNTIME: &str = "
DROP TABLE IF EXISTS model_configs;
DROP TABLE IF EXISTS connections;
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
    (6, MIGRATION_6_DOCUMENT_NAMESPACE),
    (7, MIGRATION_7_SINGLE_RUNTIME),
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
    // SQLite defaults foreign keys to OFF, per connection. Without this, every
    // `REFERENCES ... ON DELETE CASCADE` in the schema is decorative: deleting a
    // connection would orphan its `model_configs`, and a message could be
    // inserted into a chat that no longer exists (which is exactly what happens
    // when a chat is deleted while it is still generating).
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|e| e.to_string())?;
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

    fn temp_db() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("localmind-db-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("{}.db", uuid::Uuid::new_v4()))
    }

    #[test]
    fn open_enables_foreign_keys() {
        // SQLite defaults this to OFF per connection, which silently turned
        // every ON DELETE CASCADE in the schema into decoration.
        let file = temp_db();
        let conn = open(&file).unwrap();

        let enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);

        let _ = std::fs::remove_file(&file);
    }

    // `deleting_a_connection_now_takes_its_model_configs_with_it` lived here.
    // It proved that the declared `ON DELETE CASCADE` actually fired (AD-040) —
    // on `model_configs`, a table migration 7 drops. Its subject is gone, so
    // the test is gone with it. Foreign key enforcement itself is still covered
    // by `open_enables_foreign_keys` and by the orphan-message test below,
    // which is the constraint that still has a table to protect.

    #[test]
    fn a_message_cannot_be_written_into_a_chat_that_no_longer_exists() {
        // What happens when a chat is deleted while it is still generating:
        // the answer used to be inserted anyway, as an orphan nobody sees.
        let file = temp_db();
        let conn = open(&file).unwrap();

        let result = conn.execute(
            "INSERT INTO messages (id, chat_id, role, content, created_at)
             VALUES ('m1', 'gone', 'assistant', 'resposta', 'now')",
            [],
        );

        assert!(result.is_err(), "an orphan message must be refused");

        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn a_pre_existing_database_gains_the_namespace_column_as_global() {
        // Rows written before migration 6 are all global by definition; the
        // default is what keeps them out of the "interrupted attachment" sweep.
        let mut conn = Connection::open_in_memory().unwrap();
        for (version, sql) in MIGRATIONS.iter().take_while(|(v, _)| *v <= 5) {
            conn.execute_batch(sql).unwrap();
            conn.pragma_update(None, "user_version", *version).unwrap();
        }
        conn.execute(
            "INSERT INTO documents (id, filename, file_path, size_bytes, status, created_at, updated_at)
             VALUES ('old', 'a.pdf', '/tmp/a.pdf', 1, 'ready', 'now', 'now')",
            [],
        )
        .unwrap();

        apply_migrations(&mut conn).unwrap();

        let namespace: String = conn
            .query_row("SELECT namespace FROM documents WHERE id = 'old'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(namespace, "global");
    }

    #[test]
    fn an_existing_database_upgrades_with_foreign_keys_already_on() {
        // The real upgrade path for a machine that already has data: `open`
        // turns foreign keys on and *then* migrates. Renaming a column and
        // dropping a referenced table under an enforced constraint is exactly
        // the combination that could fail only on a user's disk.
        let file = temp_db();
        {
            let mut conn = Connection::open(&file).unwrap();
            for (version, sql) in MIGRATIONS.iter().take_while(|(v, _)| *v <= 5) {
                conn.execute_batch(sql).unwrap();
                conn.pragma_update(None, "user_version", *version).unwrap();
            }
            conn.execute_batch(
                "INSERT INTO connections (id, provider, base_url, is_active, created_at)
                    VALUES ('c1', 'ollama', 'http://localhost:11434', 1, 'now');
                 INSERT INTO model_configs (id, connection_id, model_name, is_active)
                    VALUES ('m1', 'c1', 'llama3', 1);
                 INSERT INTO chats (id, title, created_at, updated_at)
                    VALUES ('chat-1', 'conversa', 'now', 'now');
                 INSERT INTO messages (id, chat_id, role, content, created_at)
                    VALUES ('msg-1', 'chat-1', 'user', 'oi', 'now');",
            )
            .unwrap();
        }

        let conn = open(&file).unwrap();

        assert_eq!(user_version(&conn).unwrap(), MIGRATIONS.last().unwrap().0);
        // The connection tables are gone on purpose; the conversation is not.
        let tables = table_names(&conn);
        assert!(!tables.contains(&"connections".to_string()));
        assert!(!tables.contains(&"model_configs".to_string()));
        let messages: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(messages, 1, "the upgrade must not touch the user's chats");

        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn the_connection_tables_do_not_survive_a_fresh_migration() {
        // Replaces `open_creates_connections_and_model_configs_tables`, which
        // asserted the opposite: migration 1 still creates the two tables and
        // migration 7 drops them, so a database that runs the whole list ends
        // without them.
        let conn = migrated_in_memory();
        let tables = table_names(&conn);

        assert!(!tables.contains(&"connections".to_string()));
        assert!(!tables.contains(&"model_configs".to_string()));
    }

    #[test]
    fn fresh_database_reaches_latest_version() {
        let conn = migrated_in_memory();
        let latest = MIGRATIONS.last().unwrap().0;
        assert_eq!(user_version(&conn).unwrap(), latest);

        let tables = table_names(&conn);
        for expected in ["chats", "messages", "embedded_runtime", "documents"] {
            assert!(tables.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn a_v1_database_with_several_enabled_connections_migrates_all_the_way() {
        // Was `migrating_a_v1_database_keeps_only_the_oldest_enabled_connection`.
        // Migration 2 still normalizes the flags, but migration 7 drops the
        // table, so the outcome is no longer observable — what is still worth
        // asserting is that the whole chain runs over that data without error
        // and leaves the conversation intact.
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_1_INITIAL).unwrap();
        conn.pragma_update(None, "user_version", 1u32).unwrap();
        conn.execute_batch(
            "INSERT INTO connections (id, provider, base_url, enabled, created_at) VALUES
                ('a', 'ollama', 'http://localhost:11434', 1, '2026-07-01T00:00:00Z'),
                ('b', 'lmstudio', 'http://localhost:1234', 1, '2026-07-02T00:00:00Z');
             INSERT INTO chats (id, title, created_at, updated_at)
                VALUES ('chat-1', 'conversa', 'now', 'now');",
        )
        .unwrap();

        apply_migrations(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), MIGRATIONS.last().unwrap().0);
        let chats: i64 = conn
            .query_row("SELECT COUNT(*) FROM chats", [], |row| row.get(0))
            .unwrap();
        assert_eq!(chats, 1);
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
        // `connections` is dropped by migration 7, so what this asserts now is
        // that the singleton row survives to the end of the chain.
        assert!(!table_names(&conn).contains(&"connections".to_string()));
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

/// Dry run of the upgrade against a copy of a real database.
///
/// Run with:
///   set LOCALMIND_REAL_DB=<path to a COPY> && cargo test real_database -- --ignored --nocapture
///
/// Deliberately takes the path from the environment and never guesses one: this
/// must never be pointed at a database someone is actually using.
#[cfg(test)]
mod real_database {
    use super::*;

    #[test]
    #[ignore = "needs LOCALMIND_REAL_DB pointing at a copy"]
    fn a_copy_of_the_real_database_upgrades_without_losing_rows() {
        let Ok(path) = std::env::var("LOCALMIND_REAL_DB") else {
            panic!("set LOCALMIND_REAL_DB to a COPY of the database");
        };
        let file = std::path::PathBuf::from(&path);

        let before = {
            let conn = Connection::open(&file).unwrap();
            let version = user_version(&conn).unwrap();
            let counts: Vec<(String, i64)> = ["chats", "messages", "documents", "chat_attachments"]
                .iter()
                .map(|t| {
                    let n: i64 = conn
                        .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                        .unwrap_or(-1);
                    (t.to_string(), n)
                })
                .collect();
            println!("antes: user_version={version} {counts:?}");
            counts
        };

        let conn = open(&file).unwrap();
        println!("depois: user_version={}", user_version(&conn).unwrap());
        assert_eq!(user_version(&conn).unwrap(), MIGRATIONS.last().unwrap().0);

        for (table, expected) in before {
            let actual: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(-1);
            assert_eq!(actual, expected, "{table} lost rows in the upgrade");
            println!("{table}: {actual} linhas preservadas");
        }

        // Every pre-existing document is global, and the sweep must not touch it.
        let non_global: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM documents WHERE namespace <> 'global'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(non_global, 0, "existing documents must all be global");
        println!("nenhum documento marcado como não-global");
    }
}
