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

/// Ordered list of schema versions. A migration is applied only when
/// `PRAGMA user_version` is below its number, which is what makes a column
/// change reach databases that already exist on disk — `CREATE TABLE IF NOT
/// EXISTS` alone would silently no-op there.
const MIGRATIONS: &[(u32, &str)] = &[(1, MIGRATION_1_INITIAL)];

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
