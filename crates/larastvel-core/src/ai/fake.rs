use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;

use super::messages::{ChatOptions, ChatResponse, EmbeddingOptions, Message};
use super::provider::{AiProvider, ChatStream, ProviderError};

/// A fake AI provider for tests, mirroring the Laravel AI SDK's faking
/// utilities (`Ai::fake()`).
///
/// Queue canned chat responses with [`FakeAi::add_response`] (or streamed
/// chunks with [`FakeAi::add_stream_response`]). When the queue is empty the
/// provider answers with `"Fake response"`. Embeddings are deterministic
/// hash-derived vectors, so tests can assert on shapes and equality without
/// a network call.
#[derive(Debug, Default)]
pub struct FakeAi {
    responses: Mutex<VecDeque<String>>,
    stream_responses: Mutex<VecDeque<Vec<String>>>,
    calls: AtomicUsize,
}

impl FakeAi {
    /// Create an empty fake.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a canned chat completion response.
    pub fn add_response(&self, text: impl Into<String>) -> &Self {
        self.responses.lock().unwrap().push_back(text.into());
        self
    }

    /// Queue a canned streaming response, yielded as the given chunks.
    pub fn add_stream_response(&self, chunks: Vec<String>) -> &Self {
        self.stream_responses.lock().unwrap().push_back(chunks);
        self
    }

    /// The total number of provider calls made (chat + embeddings).
    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// Assert the total number of provider calls.
    pub fn assert_call_count(&self, expected: usize) {
        assert_eq!(
            self.call_count(),
            expected,
            "expected {} provider calls, got {}",
            expected,
            self.call_count()
        );
    }

    fn next_response(&self) -> String {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| "Fake response".to_string())
    }
}

/// Deterministic pseudo-embedding derived from the input (8 dimensions),
/// stable across calls so equality assertions work in tests.
pub(crate) fn fake_embedding(input: &str) -> Vec<f32> {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|&byte| (byte as f32 / 255.0) * 2.0 - 1.0)
        .collect()
}

#[async_trait]
impl AiProvider for FakeAi {
    fn name(&self) -> &str {
        "fake"
    }

    async fn chat(
        &self,
        _messages: &[Message],
        _options: &ChatOptions,
    ) -> Result<ChatResponse, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ChatResponse {
            text: self.next_response(),
            usage: None,
            finish_reason: Some("stop".into()),
            tool_calls: Vec::new(),
        })
    }

    async fn chat_stream(
        &self,
        _messages: &[Message],
        _options: &ChatOptions,
    ) -> Result<ChatStream, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let chunks = self
            .stream_responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| vec!["Fake response".to_string()]);
        Ok(Box::pin(futures_util::stream::iter(
            chunks.into_iter().map(Ok),
        )))
    }

    async fn embed(
        &self,
        input: &str,
        _options: &EmbeddingOptions,
    ) -> Result<Vec<f32>, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(fake_embedding(input))
    }

    async fn embed_many(
        &self,
        inputs: &[String],
        _options: &EmbeddingOptions,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(inputs.iter().map(|input| fake_embedding(input)).collect())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[test]
    fn test_fake_embedding_is_deterministic() {
        let a = fake_embedding("hello");
        let b = fake_embedding("hello");
        let c = fake_embedding("world");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 8);
        assert!(a.iter().all(|v| (-1.0..=1.0).contains(v)));
    }

    #[tokio::test]
    async fn test_fake_chat_queued_responses() {
        let fake = FakeAi::new();
        fake.add_response("first").add_response("second");

        let ai_response = fake
            .chat(&[Message::user("Hi")], &ChatOptions::default())
            .await
            .unwrap();
        assert_eq!(ai_response.text, "first");
        let ai_response = fake
            .chat(&[Message::user("Hi")], &ChatOptions::default())
            .await
            .unwrap();
        assert_eq!(ai_response.text, "second");
        let ai_response = fake
            .chat(&[Message::user("Hi")], &ChatOptions::default())
            .await
            .unwrap();
        assert_eq!(ai_response.text, "Fake response");
        fake.assert_call_count(3);
    }

    #[tokio::test]
    async fn test_fake_stream_queued_chunks() {
        let fake = FakeAi::new();
        fake.add_stream_response(vec!["Hello ".into(), "world".into()]);

        let stream = fake
            .chat_stream(&[Message::user("Hi")], &ChatOptions::default())
            .await
            .unwrap();
        let chunks: Vec<String> = stream.map(|c| c.unwrap()).collect().await;
        assert_eq!(chunks, vec!["Hello ".to_string(), "world".to_string()]);
    }
}
