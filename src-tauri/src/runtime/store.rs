use rusqlite::{params, Connection as SqlConnection, OptionalExtension};

/// The persisted half of the embedded runtime: what was downloaded and how it
/// should be started. Kept in its own singleton row so `connections` doesn't
/// grow columns only one provider uses.
#[derive(Debug, Clone, Default)]
pub struct EmbeddedRuntimeRow {
    pub release_tag: Option<String>,
    pub backend: Option<String>,
    pub binary_path: Option<String>,
    pub model_path: Option<String>,
    pub context_length: Option<u32>,
    pub gpu_layers: Option<i32>,
}

impl EmbeddedRuntimeRow {
    /// Both halves are required: a binary with no model can't answer, and a
    /// model with no binary can't run.
    pub fn is_ready(&self) -> bool {
        match (&self.binary_path, &self.model_path) {
            (Some(bin), Some(model)) => {
                std::path::Path::new(bin).exists() && std::path::Path::new(model).exists()
            }
            _ => false,
        }
    }
}

pub fn load(sql: &SqlConnection) -> Result<EmbeddedRuntimeRow, String> {
    sql.query_row(
        "SELECT release_tag, backend, binary_path, model_path, context_length, gpu_layers
         FROM embedded_runtime WHERE id = 1",
        [],
        |row| {
            Ok(EmbeddedRuntimeRow {
                release_tag: row.get(0)?,
                backend: row.get(1)?,
                binary_path: row.get(2)?,
                model_path: row.get(3)?,
                context_length: row.get(4)?,
                gpu_layers: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
    .map(Option::unwrap_or_default)
}

pub fn save(sql: &SqlConnection, row: &EmbeddedRuntimeRow) -> Result<(), String> {
    sql.execute(
        "INSERT INTO embedded_runtime (id, release_tag, backend, binary_path, model_path, context_length, gpu_layers)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
            release_tag = excluded.release_tag,
            backend = excluded.backend,
            binary_path = excluded.binary_path,
            model_path = excluded.model_path,
            context_length = excluded.context_length,
            gpu_layers = excluded.gpu_layers",
        params![
            row.release_tag,
            row.backend,
            row.binary_path,
            row.model_path,
            row.context_length,
            row.gpu_layers
        ],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> SqlConnection {
        let mut conn = SqlConnection::open_in_memory().unwrap();
        crate::db::apply_migrations(&mut conn).unwrap();
        conn
    }

    #[test]
    fn missing_row_loads_as_empty_and_not_ready() {
        let sql = setup();
        let row = load(&sql).unwrap();
        assert!(row.release_tag.is_none());
        assert!(!row.is_ready());
    }

    #[test]
    fn saving_twice_updates_the_same_singleton_row() {
        let sql = setup();
        save(
            &sql,
            &EmbeddedRuntimeRow {
                release_tag: Some("b10107".to_string()),
                backend: Some("vulkan".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        save(
            &sql,
            &EmbeddedRuntimeRow {
                release_tag: Some("b10107".to_string()),
                backend: Some("cpu".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let count: i64 = sql
            .query_row("SELECT COUNT(*) FROM embedded_runtime", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(load(&sql).unwrap().backend.unwrap(), "cpu");
    }

    #[test]
    fn is_ready_requires_both_paths_to_exist_on_disk() {
        let row = EmbeddedRuntimeRow {
            binary_path: Some("/nope/llama-server".to_string()),
            model_path: Some("/nope/phi.gguf".to_string()),
            ..Default::default()
        };
        assert!(!row.is_ready());
    }
}
