use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

/// A flag the streaming loop checks between tokens. A plain atomic keeps the
/// dependency footprint at zero — cancellation here only has to stop a loop,
/// not unwind a task tree.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// One generation per chat at a time (design decision in chat-messaging):
/// registering a second one replaces the first, so a stale token can never
/// cancel a newer generation.
#[derive(Default)]
pub struct CancellationRegistry(Mutex<HashMap<String, CancellationToken>>);

impl CancellationRegistry {
    pub fn new() -> Self {
        CancellationRegistry::default()
    }

    pub fn register(&self, chat_id: &str) -> CancellationToken {
        let token = CancellationToken::default();
        if let Ok(mut map) = self.0.lock() {
            map.insert(chat_id.to_string(), token.clone());
        }
        token
    }

    pub fn cancel(&self, chat_id: &str) {
        if let Ok(map) = self.0.lock() {
            if let Some(token) = map.get(chat_id) {
                token.cancel();
            }
        }
    }

    pub fn finish(&self, chat_id: &str) {
        if let Ok(mut map) = self.0.lock() {
            map.remove(chat_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_flips_only_the_requested_chat() {
        let registry = CancellationRegistry::new();
        let a = registry.register("chat-a");
        let b = registry.register("chat-b");

        registry.cancel("chat-a");

        assert!(a.is_cancelled());
        assert!(!b.is_cancelled());
    }

    #[test]
    fn re_registering_a_chat_leaves_the_old_token_uncancelled() {
        let registry = CancellationRegistry::new();
        let first = registry.register("chat");
        let second = registry.register("chat");

        registry.cancel("chat");

        assert!(second.is_cancelled(), "the live generation must stop");
        assert!(!first.is_cancelled(), "a stale token must not be reused");
    }

    #[test]
    fn cancelling_an_unknown_chat_is_a_no_op() {
        let registry = CancellationRegistry::new();
        registry.cancel("nobody");
        registry.finish("nobody");
    }
}
