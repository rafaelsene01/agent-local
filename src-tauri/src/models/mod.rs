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
}

#[derive(Debug, Serialize, Clone)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
