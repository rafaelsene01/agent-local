// SPEC: app-shell (SHELL-04), chat-messaging (CHAT-14), conversation-memory (MEM-14)

pub mod catalog;
pub mod memory_estimate;

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    /// Whether this chat also searches the global knowledge base (CHAT-14).
    /// Travels with the chat so the UI can show the persisted choice instead
    /// of assuming a default per render.
    pub use_global_rag: bool,
    /// Whether completed turns of this chat are remembered and recalled
    /// (MEM-14). Same reasoning as above: it travels with the chat so the
    /// toggle shows the stored choice, not a default.
    pub use_memory: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
