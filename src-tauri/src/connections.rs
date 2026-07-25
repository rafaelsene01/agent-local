use crate::providers::{custom::CustomClient, lmstudio::LmStudioClient, ollama::OllamaClient, ProviderClient};
use chrono::Utc;
use rusqlite::{params, Connection as SqlConnection};
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
    pub enabled: bool,
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

pub fn create_connection(
    sql: &SqlConnection,
    provider: String,
    base_url: String,
) -> Result<Connection, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sql.execute(
        "INSERT INTO connections (id, provider, base_url, enabled, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
        params![id, provider, base_url, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(Connection {
        id,
        provider,
        base_url,
        enabled: true,
        status: ConnectionStatus::Unknown,
    })
}

pub fn list_connections(sql: &SqlConnection) -> Result<Vec<Connection>, String> {
    let mut stmt = sql
        .prepare("SELECT id, provider, base_url, enabled FROM connections ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;

    let connections = stmt
        .query_map([], |row| {
            let enabled: i64 = row.get(3)?;
            Ok(Connection {
                id: row.get(0)?,
                provider: row.get(1)?,
                base_url: row.get(2)?,
                enabled: enabled != 0,
                status: ConnectionStatus::Unknown,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Connection>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(connections)
}

pub fn toggle_connection(sql: &SqlConnection, id: &str, enabled: bool) -> Result<(), String> {
    let updated = sql
        .execute(
            "UPDATE connections SET enabled = ?1 WHERE id = ?2",
            params![enabled as i64, id],
        )
        .map_err(|e| e.to_string())?;

    if updated == 0 {
        return Err("Conexão não encontrada".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> SqlConnection {
        let conn = SqlConnection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE connections (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                base_url TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn create_list_and_toggle_connection() {
        let sql = setup();
        let created = create_connection(&sql, "ollama".to_string(), "http://localhost:11434".to_string()).unwrap();
        assert!(created.enabled);

        let listed = list_connections(&sql).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        toggle_connection(&sql, &created.id, false).unwrap();
        let listed = list_connections(&sql).unwrap();
        assert!(!listed[0].enabled);
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
