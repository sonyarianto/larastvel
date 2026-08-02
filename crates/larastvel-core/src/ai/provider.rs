use std::fmt::Debug;
use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;

use super::image::{ImageOptions, ImageResponse};
use super::media::AudioOptions;
use super::messages::{ChatOptions, ChatResponse, EmbeddingOptions, Message};
use super::moderation::ModerationResponse;
use super::rerank::{RerankOptions, RerankResponse};

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
    #[error("The configured AI provider does not support {0}")]
    Unsupported(String),
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

    /// Generate an image from a text prompt. Defaults to an "unsupported"
    /// error so providers without image support stay valid.
    async fn image_create(
        &self,
        _prompt: &str,
        _options: &ImageOptions,
    ) -> Result<ImageResponse, ProviderError> {
        Err(ProviderError::Unsupported("image generation".into()))
    }

    /// Edit an image with a text prompt. Defaults to "unsupported".
    async fn image_edit(
        &self,
        _image: &super::media::Media,
        _prompt: &str,
        _options: &ImageOptions,
    ) -> Result<ImageResponse, ProviderError> {
        Err(ProviderError::Unsupported("image editing".into()))
    }

    /// Create a variation of an image. Defaults to "unsupported".
    async fn image_variation(
        &self,
        _image: &super::media::Media,
        _options: &ImageOptions,
    ) -> Result<ImageResponse, ProviderError> {
        Err(ProviderError::Unsupported("image variations".into()))
    }

    /// Synthesize speech from text, returning audio bytes. Defaults to
    /// "unsupported".
    async fn tts(&self, _text: &str, _options: &AudioOptions) -> Result<Vec<u8>, ProviderError> {
        Err(ProviderError::Unsupported("text-to-speech".into()))
    }

    /// Transcribe speech to text. Defaults to "unsupported".
    async fn stt(
        &self,
        _audio: &super::media::Media,
        _options: &AudioOptions,
    ) -> Result<String, ProviderError> {
        Err(ProviderError::Unsupported("speech-to-text".into()))
    }

    /// Moderate content, flagging policy violations. Defaults to
    /// "unsupported".
    async fn moderate(&self, _content: &str) -> Result<ModerationResponse, ProviderError> {
        Err(ProviderError::Unsupported("moderation".into()))
    }

    /// Rerank documents by relevance to a query. Defaults to
    /// "unsupported".
    async fn rerank(
        &self,
        _query: &str,
        _documents: &[String],
        _options: &RerankOptions,
    ) -> Result<RerankResponse, ProviderError> {
        Err(ProviderError::Unsupported("reranking".into()))
    }
}
