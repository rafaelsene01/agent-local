pub mod chunking;
pub mod embedding;
pub mod onnxruntime;
pub mod parsing;
pub mod pipeline;
pub mod store;

/// Chunk size and overlap, in approximate tokens. 512 keeps a chunk well
/// inside multilingual-e5-small's window while staying big enough to hold a
/// whole idea; the overlap is what keeps a sentence cut at a boundary
/// retrievable from the neighbouring chunk.
pub const CHUNK_MAX_TOKENS: usize = 512;
pub const CHUNK_OVERLAP_TOKENS: usize = 64;
