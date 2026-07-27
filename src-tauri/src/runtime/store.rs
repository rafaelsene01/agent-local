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
    /// The engine half alone: a probed backend whose binary is still on disk.
    /// Kept apart from `is_ready` because "prepared but no model" is the normal
    /// state of a fresh install now that preparing downloads nothing (SELF-11).
    pub fn is_prepared(&self) -> bool {
        self.binary_path
            .as_deref()
            .is_some_and(|bin| std::path::Path::new(bin).exists())
    }

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

/// The active model, as the chat needs to see it (SELF-07).
///
/// There is no `model_configs` table any more, and no "active pair": there is
/// one runtime, so the model it is configured to start with **is** the active
/// model. The name is the file name, which is what the user picked and what
/// the citations and the model list show.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ActiveModel {
    pub name: String,
    pub path: String,
    pub context_length: Option<u32>,
    pub gpu_layers: Option<i32>,
}

/// `None` covers both "never chose one" and "the file is gone".
///
/// The second case matters: a model deleted from the folder used to leave a
/// configured path pointing at nothing, and the failure surfaced much later as
/// a sidecar that would not start. Reporting it as "no active model" sends the
/// user to the one screen that can fix it.
pub fn active_model(sql: &SqlConnection) -> Result<Option<ActiveModel>, String> {
    let row = load(sql)?;
    let Some(path) = row.model_path else {
        return Ok(None);
    };
    let as_path = std::path::Path::new(&path);
    if !as_path.exists() {
        return Ok(None);
    }
    let name = as_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    Ok(Some(ActiveModel {
        name,
        path,
        context_length: row.context_length,
        gpu_layers: row.gpu_layers,
    }))
}

/// Choosing a model must not silently reset the context/GPU the user tuned, so
/// this writes one field of the singleton row rather than replacing it.
///
/// Returns the row and whether the model actually changed — the caller needs
/// the second half to decide if the sidecar has to be restarted.
pub fn set_active_model(
    sql: &SqlConnection,
    model_path: &str,
) -> Result<(EmbeddedRuntimeRow, bool), String> {
    let mut row = load(sql)?;
    let changed = row.model_path.as_deref() != Some(model_path);
    if changed {
        row.model_path = Some(model_path.to_string());
        save(sql, &row)?;
    }
    Ok((row, changed))
}

/// `context_length: None` is a real value — "let the server pick" — so it is
/// written. `gpu_layers: None` is the absence of an opinion and leaves the
/// current offload alone; the two nulls mean different things here.
pub fn set_config(
    sql: &SqlConnection,
    context_length: Option<u32>,
    gpu_layers: Option<i32>,
) -> Result<EmbeddedRuntimeRow, String> {
    let mut row = load(sql)?;
    row.context_length = context_length;
    if let Some(layers) = gpu_layers {
        row.gpu_layers = Some(layers);
    }
    save(sql, &row)?;
    Ok(row)
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

    /// Creates a real file, because `active_model` deliberately checks the disk.
    fn a_model_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("localmind-active-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, b"gguf").unwrap();
        path
    }

    #[test]
    fn no_model_configured_means_no_active_model() {
        let sql = setup();
        assert_eq!(active_model(&sql).unwrap(), None);
    }

    #[test]
    fn the_active_model_is_read_back_with_its_name_and_config() {
        let sql = setup();
        let path = a_model_file("Phi-3.5-mini-instruct-Q4_K_M.gguf");
        set_active_model(&sql, &path.to_string_lossy()).unwrap();
        set_config(&sql, Some(8192), Some(-1)).unwrap();

        let active = active_model(&sql).unwrap().expect("a model was chosen");
        assert_eq!(active.name, "Phi-3.5-mini-instruct-Q4_K_M.gguf");
        assert_eq!(active.context_length, Some(8192));
        assert_eq!(active.gpu_layers, Some(-1));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_model_deleted_from_disk_reads_as_none_not_as_a_broken_active() {
        let sql = setup();
        let path = a_model_file("gone.gguf");
        set_active_model(&sql, &path.to_string_lossy()).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(
            active_model(&sql).unwrap(),
            None,
            "a path pointing at nothing is not an active model"
        );
    }

    #[test]
    fn choosing_a_model_does_not_reset_the_tuning() {
        let sql = setup();
        set_config(&sql, Some(4096), Some(0)).unwrap();
        let path = a_model_file("outro.gguf");
        set_active_model(&sql, &path.to_string_lossy()).unwrap();

        let active = active_model(&sql).unwrap().unwrap();
        assert_eq!(active.context_length, Some(4096));
        assert_eq!(active.gpu_layers, Some(0));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn setting_the_config_does_not_drop_the_installed_paths() {
        let sql = setup();
        save(
            &sql,
            &EmbeddedRuntimeRow {
                release_tag: Some("b10107".to_string()),
                binary_path: Some("/bin/llama-server".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        set_config(&sql, Some(2048), None).unwrap();

        let row = load(&sql).unwrap();
        assert_eq!(row.binary_path.as_deref(), Some("/bin/llama-server"));
        assert_eq!(row.release_tag.as_deref(), Some("b10107"));
    }

    /// The state a fresh install sits in now that preparing downloads no
    /// model: the engine is there, the model is the user's next move.
    #[test]
    fn an_engine_without_a_model_is_prepared_but_not_ready() {
        let binary = a_model_file("llama-server-stub");
        let row = EmbeddedRuntimeRow {
            binary_path: Some(binary.to_string_lossy().to_string()),
            model_path: None,
            ..Default::default()
        };
        assert!(row.is_prepared(), "the binary is on disk");
        assert!(!row.is_ready(), "but there is nothing to load");

        let _ = std::fs::remove_file(&binary);
    }

    #[test]
    fn a_binary_path_pointing_at_nothing_is_not_prepared() {
        let row = EmbeddedRuntimeRow {
            binary_path: Some("/nope/llama-server".to_string()),
            ..Default::default()
        };
        assert!(!row.is_prepared());
    }

    #[test]
    fn set_config_leaves_the_gpu_choice_alone_when_none_is_given() {
        let sql = setup();
        set_config(&sql, Some(4096), Some(-1)).unwrap();
        set_config(&sql, Some(8192), None).unwrap();

        let row = load(&sql).unwrap();
        assert_eq!(row.context_length, Some(8192));
        assert_eq!(
            row.gpu_layers,
            Some(-1),
            "no opinion about the GPU must not read as 'turn it off'"
        );
    }

    #[test]
    fn set_active_model_reports_whether_the_choice_actually_changed() {
        let sql = setup();
        let path = a_model_file("same.gguf");
        let as_str = path.to_string_lossy().to_string();

        let (_, first) = set_active_model(&sql, &as_str).unwrap();
        let (_, second) = set_active_model(&sql, &as_str).unwrap();
        assert!(first, "the first choice is a change");
        assert!(!second, "re-picking the same file must not restart the sidecar");

        let _ = std::fs::remove_file(&path);
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
