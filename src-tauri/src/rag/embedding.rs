use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// `MultilingualE5Small` (intfloat/multilingual-e5-small) — confirmed present
/// in fastembed 5.17's `EmbeddingModel` enum on 2026-07-25. Chosen over the
/// English-only defaults because the app ships EN and PT (AD-007) and a
/// Portuguese document embedded by an English-only model retrieves badly.
/// Small keeps the download near ~120MB and the vectors at 384 dimensions.
const MODEL: EmbeddingModel = EmbeddingModel::MultilingualE5Small;

pub const EMBEDDING_DIM: usize = 384;

#[derive(Debug)]
pub enum EmbeddingError {
    ModelUnavailable(String),
    Failed(String),
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingError::ModelUnavailable(msg) => {
                write!(f, "não foi possível carregar o modelo de embedding: {msg}")
            }
            EmbeddingError::Failed(msg) => write!(f, "falha ao gerar embeddings: {msg}"),
        }
    }
}

impl std::error::Error for EmbeddingError {}

static MODEL_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
static EMBEDDER: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();

/// Points the model download at the user's base folder (AD-008) instead of a
/// hidden cache. Must be called before the first `embed_batch`; ignored
/// afterwards, since the model is only loaded once.
pub fn set_cache_dir(dir: PathBuf) {
    let _ = MODEL_CACHE_DIR.set(dir);
}

/// Loaded lazily and kept for the process lifetime: initialization downloads
/// (first run) and unpacks an ONNX model, far too expensive per document.
/// `TextEmbedding::embed` takes `&mut self`, hence the Mutex.
fn embedder() -> Result<&'static Mutex<TextEmbedding>, EmbeddingError> {
    if let Some(existing) = EMBEDDER.get() {
        return Ok(existing);
    }

    let mut options = TextInitOptions::new(MODEL).with_show_download_progress(false);
    if let Some(dir) = MODEL_CACHE_DIR.get() {
        options = options.with_cache_dir(dir.clone());
    }

    let model =
        TextEmbedding::try_new(options).map_err(|e| EmbeddingError::ModelUnavailable(e.to_string()))?;
    let _ = EMBEDDER.set(Mutex::new(model));
    EMBEDDER
        .get()
        .ok_or_else(|| EmbeddingError::ModelUnavailable("modelo não inicializado".to_string()))
}

pub fn embed_batch(texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let cell = embedder()?;
    let mut model = cell
        .lock()
        .map_err(|e| EmbeddingError::Failed(e.to_string()))?;
    model
        .embed(texts.to_vec(), None)
        .map_err(|e| EmbeddingError::Failed(e.to_string()))
}

/// E5 models are trained with `query:` / `passage:` prefixes and lose accuracy
/// without them, so the asymmetry is encoded here rather than at every call
/// site.
pub fn embed_passages(texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
    embed_batch(&prefixed)
}

pub fn embed_query(text: &str) -> Result<Vec<f32>, EmbeddingError> {
    let mut vectors = embed_batch(&[format!("query: {text}")])?;
    vectors
        .pop()
        .ok_or_else(|| EmbeddingError::Failed("nenhum vetor retornado".to_string()))
}
