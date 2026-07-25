use crate::providers::{custom::CustomClient, lmstudio::LmStudioClient, ollama::OllamaClient, ProviderClient};
use chrono::Utc;
use rusqlite::{params, Connection as SqlConnection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Serialize, Clone)]
pub struct Connection {
    pub id: String,
    pub provider: String,
    pub base_url: String,
    pub is_active: bool,
    pub status: ConnectionStatus,
}

/// A known runtime the app can offer to add — not yet persisted, no I/O.
#[derive(Debug, Serialize, Clone)]
pub struct ConnectionCandidate {
    pub provider: String,
    pub base_url: String,
}

pub struct ConnectionManager;

impl ConnectionManager {
    pub fn new() -> Self {
        ConnectionManager
    }

    pub fn detect_known_connections(&self) -> Vec<ConnectionCandidate> {
        vec![
            ConnectionCandidate {
                provider: "ollama".to_string(),
                base_url: "http://localhost:11434".to_string(),
            },
            ConnectionCandidate {
                provider: "lmstudio".to_string(),
                base_url: "http://localhost:1234".to_string(),
            },
        ]
    }

    // SPEC_DEVIATION: tasks.md didn't assign a task for a "custom" provider
    // client, but connections.provider allows 'custom' (schema + CONN-01
    // AC4 "URL customizada"), so provider_for must be total. Routed to a
    // minimal generic OpenAI-compatible client (providers::custom).
    pub fn provider_for(&self, conn: &Connection) -> Box<dyn ProviderClient> {
        match conn.provider.as_str() {
            "ollama" => Box::new(OllamaClient::new(conn.base_url.clone())),
            "lmstudio" => Box::new(LmStudioClient::new(conn.base_url.clone())),
            _ => Box::new(CustomClient::new(conn.base_url.clone())),
        }
    }

    pub async fn refresh_status(&self, conn: &Connection) -> ConnectionStatus {
        let client = self.provider_for(conn);
        match client.health_check().await {
            Ok(()) => ConnectionStatus::Available,
            Err(_) => ConnectionStatus::Unavailable,
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Connections are always created inactive: activation is exclusive
/// (AD-021) and belongs to `set_active_connection`, which is the only place
/// that can enforce it in a single transaction.
pub fn create_connection(
    sql: &SqlConnection,
    provider: String,
    base_url: String,
) -> Result<Connection, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sql.execute(
        "INSERT INTO connections (id, provider, base_url, is_active, created_at) VALUES (?1, ?2, ?3, 0, ?4)",
        params![id, provider, base_url, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(Connection {
        id,
        provider,
        base_url,
        is_active: false,
        status: ConnectionStatus::Unknown,
    })
}

fn row_to_connection(row: &rusqlite::Row) -> rusqlite::Result<Connection> {
    let is_active: i64 = row.get(3)?;
    Ok(Connection {
        id: row.get(0)?,
        provider: row.get(1)?,
        base_url: row.get(2)?,
        is_active: is_active != 0,
        status: ConnectionStatus::Unknown,
    })
}

pub fn list_connections(sql: &SqlConnection) -> Result<Vec<Connection>, String> {
    let mut stmt = sql
        .prepare(
            "SELECT id, provider, base_url, is_active FROM connections ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let connections = stmt
        .query_map([], row_to_connection)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Connection>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(connections)
}

pub fn active_connection(sql: &SqlConnection) -> Result<Option<Connection>, String> {
    sql.query_row(
        "SELECT id, provider, base_url, is_active FROM connections WHERE is_active = 1",
        [],
        row_to_connection,
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Deactivating every other connection and dropping an active model that
/// belongs to one of them must happen together, so there is never a moment
/// where two connections are active or where the active model points
/// somewhere the chat can't reach (ACTIVE-01, ACTIVE-06).
///
/// Takes no transaction of its own so callers that already opened one — like
/// `set_active_model`, which activates the pair atomically — can reuse it;
/// SQLite rejects a nested `BEGIN`.
pub fn apply_active_connection(tx: &SqlConnection, id: &str) -> Result<(), String> {
    let exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM connections WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err("Conexão não encontrada".to_string());
    }

    tx.execute("UPDATE connections SET is_active = (id = ?1)", params![id])
        .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE model_configs SET is_active = 0 WHERE is_active = 1 AND connection_id <> ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn set_active_connection(sql: &SqlConnection, id: &str) -> Result<(), String> {
    let tx = sql.unchecked_transaction().map_err(|e| e.to_string())?;
    apply_active_connection(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())
}

/// A model can't stay active without a connection to reach it through, so
/// clearing the connection clears the model too (spec edge case).
pub fn clear_active_connection(sql: &SqlConnection) -> Result<(), String> {
    let tx = sql.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("UPDATE connections SET is_active = 0", [])
        .map_err(|e| e.to_string())?;
    tx.execute("UPDATE model_configs SET is_active = 0", [])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> SqlConnection {
        let mut conn = SqlConnection::open_in_memory().unwrap();
        crate::db::apply_migrations(&mut conn).unwrap();
        conn
    }

    fn activate_model(sql: &SqlConnection, connection_id: &str) {
        sql.execute(
            "INSERT INTO model_configs (id, connection_id, model_name, is_active) VALUES (?1, ?2, 'm', 1)",
            params![Uuid::new_v4().to_string(), connection_id],
        )
        .unwrap();
    }

    fn active_model_count(sql: &SqlConnection) -> i64 {
        sql.query_row(
            "SELECT COUNT(*) FROM model_configs WHERE is_active = 1",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn create_and_list_connection() {
        let sql = setup();
        let created = create_connection(
            &sql,
            "ollama".to_string(),
            "http://localhost:11434".to_string(),
        )
        .unwrap();
        assert!(!created.is_active);

        let listed = list_connections(&sql).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert!(active_connection(&sql).unwrap().is_none());
    }

    #[test]
    fn activating_a_connection_deactivates_the_previous_one() {
        let sql = setup();
        let a = create_connection(&sql, "ollama".to_string(), "http://a".to_string()).unwrap();
        let b = create_connection(&sql, "lmstudio".to_string(), "http://b".to_string()).unwrap();

        set_active_connection(&sql, &a.id).unwrap();
        assert_eq!(active_connection(&sql).unwrap().unwrap().id, a.id);

        set_active_connection(&sql, &b.id).unwrap();
        let listed = list_connections(&sql).unwrap();
        assert_eq!(
            listed.iter().filter(|c| c.is_active).count(),
            1,
            "exactly one connection may be active"
        );
        assert_eq!(active_connection(&sql).unwrap().unwrap().id, b.id);
    }

    #[test]
    fn switching_connection_drops_a_model_owned_by_another_one() {
        let sql = setup();
        let a = create_connection(&sql, "ollama".to_string(), "http://a".to_string()).unwrap();
        let b = create_connection(&sql, "lmstudio".to_string(), "http://b".to_string()).unwrap();
        set_active_connection(&sql, &a.id).unwrap();
        activate_model(&sql, &a.id);

        set_active_connection(&sql, &b.id).unwrap();

        assert_eq!(active_model_count(&sql), 0);
    }

    #[test]
    fn clearing_the_active_connection_clears_the_active_model() {
        let sql = setup();
        let a = create_connection(&sql, "ollama".to_string(), "http://a".to_string()).unwrap();
        set_active_connection(&sql, &a.id).unwrap();
        activate_model(&sql, &a.id);

        clear_active_connection(&sql).unwrap();

        assert!(active_connection(&sql).unwrap().is_none());
        assert_eq!(active_model_count(&sql), 0);
    }

    #[test]
    fn detect_known_connections_returns_ollama_and_lmstudio() {
        let mgr = ConnectionManager::new();
        let candidates = mgr.detect_known_connections();
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|c| c.provider == "ollama"));
        assert!(candidates.iter().any(|c| c.provider == "lmstudio"));
    }
}
