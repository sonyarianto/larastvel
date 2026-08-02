use std::fmt::Debug;
use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;

use super::messages::{ChatOptions, ChatResponse, EmbeddingOptions, Message};

/// An error raised by an AI provider.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("AI provider error: {0}")]
    Request(String),
    #[error("AI provider returned HTTP {0}: {1}")]
    Status(u16, String),
    #[error("AI provider returned an invalid response: {0}")]
    InvalidResponse(String),
    #[error(
        "No API key configured. Set `ai.api_key` in config/ai.toml or the {0} environment variable."
    )]
    MissingApiKey(String),
    #[error("No AI provider configured. Set `ai.provider` in config/ai.toml.")]
    NoProvider,
    #[error("Unsupported AI provider: {0}")]
    UnsupportedProvider(String),
}

/// A stream of incremental text chunks from a streaming chat completion.
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String, ProviderError>> + Send>>;

/// A unified AI provider, mirroring the Laravel AI SDK's provider
/// abstraction. Providers implement chat completions (text and streaming)
/// and embeddings; the framework keeps a consistent interface regardless of
/// the underlying vendor.
#[async_trait]
pub trait AiProvider: Send + Sync + Debug {
    /// The provider's name (e.g. `openai`, `anthropic`, `fake`).
    fn name(&self) -> &str;

    /// Complete a chat conversation.
    async fn chat(
        &self,
        messages: &[Message],
        options: &ChatOptions,
    ) -> Result<ChatResponse, ProviderError>;

    /// Stream a chat completion, yielding incremental text chunks.
    async fn chat_stream(
        &self,
        messages: &[Message],
        options: &ChatOptions,
    ) -> Result<ChatStream, ProviderError>;

    /// Embed a single text input into a vector.
    async fn embed(
        &self,
        input: &str,
        options: &EmbeddingOptions,
    ) -> Result<Vec<f32>, ProviderError>;

    /// Embed multiple text inputs into vectors.
    async fn embed_many(
        &self,
        inputs: &[String],
        options: &EmbeddingOptions,
    ) -> Result<Vec<Vec<f32>>, ProviderError>;
}
