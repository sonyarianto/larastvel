//! First-party AI SDK — the Rust equivalent of Laravel 13's `laravel/ai`.
//!
//! Provides a unified, provider-agnostic interface for text generation
//! (with streaming and structured output) and embeddings, along with
//! testing fakes and config-driven wiring:
//!
//! ```rust,ignore
//! use larastvel_core::ai::{Ai, Message};
//!
//! let ai = Ai::from_config(&config)?;
//!
//! let summary = ai.generate("Summarize the docs").await?;
//!
//! let response = ai.chat(&[
//!     Message::system("You are a helpful assistant."),
//!     Message::user("What is larastvel?"),
//! ]).await?;
//!
//! let embedding = ai.embed("search this text").await?;
//! ```

mod agent;
mod fake;
mod messages;
mod openai;
mod provider;

pub use agent::{
    Agent, AgentResult, AgentTask, AgentTaskStatus, AgentTool, ToolError, DEFAULT_AGENT_MAX_TURNS,
};
pub use fake::FakeAi;
pub use messages::{
    ChatOptions, ChatResponse, EmbeddingOptions, Message, ResponseFormat, Role, ToolCall,
    ToolDefinition, Usage,
};
pub use openai::OpenAICompatibleProvider;
pub use provider::{AiProvider, ChatStream, ProviderError};

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::cache::CacheManager;

/// How long embeddings are cached, mirroring the Laravel AI SDK's 30-day
/// embedding cache.
pub const DEFAULT_EMBEDDING_CACHE_TTL_DAYS: u64 = 30;

const EMBEDDING_CACHE_PREFIX: &str = "ai:embedding:";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";
const DEFAULT_OPENAI_EMBEDDING_MODEL: &str = "text-embedding-3-small";

/// The AI manager — the Rust equivalent of Laravel's `Ai` facade.
///
/// Wrap a provider ([`OpenAICompatibleProvider`] or [`FakeAi`]) and call
/// [`Ai::generate`], [`Ai::chat`], [`Ai::chat_stream`], [`Ai::structured`],
/// or [`Ai::embed`].
#[derive(Debug)]
pub struct Ai {
    provider: Arc<dyn AiProvider>,
    default_model: Option<String>,
    default_embedding_model: Option<String>,
    cache: Option<Arc<CacheManager>>,
    embedding_cache_ttl: Duration,
}

impl Ai {
    /// Create an AI manager around the given provider.
    pub fn new(provider: Arc<dyn AiProvider>) -> Self {
        Self {
            provider,
            default_model: None,
            default_embedding_model: None,
            cache: None,
            embedding_cache_ttl: Duration::from_secs(DEFAULT_EMBEDDING_CACHE_TTL_DAYS * 86400),
        }
    }

    /// Create an AI manager backed by a [`FakeAi`] for tests.
    pub fn fake() -> Self {
        Self::new(Arc::new(FakeAi::new()))
    }

    /// Build an AI manager from `config/ai.toml`, with `AI_*` environment
    /// variable fallbacks:
    ///
    /// ```toml
    /// [ai]
    /// provider = "openai"
    /// api_key = ""
    /// base_url = "https://api.openai.com/v1"
    /// model = "gpt-4o-mini"
    /// embedding_model = "text-embedding-3-small"
    ///
    /// [ai.caching.embeddings]
    /// cache = true
    /// ```
    pub fn from_config(config: &crate::config::Config) -> Result<Self, ProviderError> {
        Self::from_config_with_cache(config, None)
    }

    /// Like [`Ai::from_config`], but wires an existing [`CacheManager`] for
    /// embedding caching when `ai.caching.embeddings.cache` is enabled.
    pub fn from_config_with_cache(
        config: &crate::config::Config,
        cache: Option<Arc<CacheManager>>,
    ) -> Result<Self, ProviderError> {
        let provider_name = env_or(config, "ai.provider", "AI_PROVIDER").unwrap_or_default();

        let ai = match provider_name.as_str() {
            "" => return Err(ProviderError::NoProvider),
            "openai" | "openai-compatible" | "groq" | "deepseek" | "xai" | "ollama"
            | "openrouter" => {
                let api_key = env_or(config, "ai.api_key", "AI_API_KEY")
                    .ok_or_else(|| ProviderError::MissingApiKey("AI_API_KEY".to_string()))?;
                let base_url = env_or(config, "ai.base_url", "AI_BASE_URL")
                    .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string());
                let model = env_or(config, "ai.model", "AI_MODEL")
                    .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string());
                let embedding_model = env_or(config, "ai.embedding_model", "AI_EMBEDDING_MODEL")
                    .unwrap_or_else(|| DEFAULT_OPENAI_EMBEDDING_MODEL.to_string());
                let provider =
                    OpenAICompatibleProvider::new(base_url, api_key, &model, &embedding_model);
                Self::new(Arc::new(provider))
                    .with_model(model)
                    .with_embedding_model(embedding_model)
            }
            other => return Err(ProviderError::UnsupportedProvider(other.to_string())),
        };

        let caching_enabled = env_or(config, "ai.caching.embeddings.cache", "AI_EMBEDDING_CACHE")
            .map(|value| value == "true" || value == "1")
            .unwrap_or(false);
        if caching_enabled {
            if let Some(cache) = cache {
                return Ok(ai.with_embedding_cache(cache, DEFAULT_EMBEDDING_CACHE_TTL_DAYS));
            }
        }
        Ok(ai)
    }

    /// The underlying provider.
    pub fn provider(&self) -> &Arc<dyn AiProvider> {
        &self.provider
    }

    /// Swap the underlying provider (e.g. install a fake in tests).
    pub fn set_provider(&mut self, provider: Arc<dyn AiProvider>) {
        self.provider = provider;
    }

    /// Set the default chat model for convenience methods.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Set the default embedding model for convenience methods.
    pub fn with_embedding_model(mut self, model: impl Into<String>) -> Self {
        self.default_embedding_model = Some(model.into());
        self
    }

    /// Enable embedding caching backed by the given [`CacheManager`].
    pub fn with_embedding_cache(mut self, cache: Arc<CacheManager>, ttl_days: u64) -> Self {
        self.cache = Some(cache);
        self.embedding_cache_ttl = Duration::from_secs(ttl_days * 86400);
        self
    }

    /// Generate text from a single prompt — Laravel's `Ai::generate()`.
    pub async fn generate(&self, prompt: &str) -> Result<String, ProviderError> {
        self.chat(&[Message::user(prompt)])
            .await
            .map(|response| response.text)
    }

    /// Complete a chat conversation with default options.
    pub async fn chat(&self, messages: &[Message]) -> Result<ChatResponse, ProviderError> {
        self.chat_with(messages, &ChatOptions::default()).await
    }

    /// Complete a chat conversation with explicit options.
    pub async fn chat_with(
        &self,
        messages: &[Message],
        options: &ChatOptions,
    ) -> Result<ChatResponse, ProviderError> {
        let mut options = options.clone();
        if options.model.is_none() {
            options.model = self.default_model.clone();
        }
        self.provider.chat(messages, &options).await
    }

    /// Complete a chat conversation, falling back to another call when the
    /// primary provider fails — Laravel's `withFallback(...)`.
    ///
    /// The fallback receives the primary error and returns a full
    /// `ChatResponse` (e.g. from a secondary provider):
    ///
    /// ```rust,ignore
    /// let response = ai
    ///     .chat_with_fallback(&[Message::user("Hi")], |error| async move {
    ///         let backup = OpenAICompatibleProvider::new("https://…", key, "m", "e");
    ///         Ai::new(Arc::new(backup)).chat(&[Message::user("Hi")]).await
    ///     })
    ///     .await?;
    /// ```
    pub async fn chat_with_fallback<F, Fut>(
        &self,
        messages: &[Message],
        fallback: F,
    ) -> Result<ChatResponse, ProviderError>
    where
        F: FnOnce(ProviderError) -> Fut,
        Fut: std::future::Future<Output = Result<ChatResponse, ProviderError>>,
    {
        match self.chat(messages).await {
            Ok(response) => Ok(response),
            Err(error) => fallback(error).await,
        }
    }

    /// Generate text from a single prompt, with a fallback when the primary
    /// provider fails — Laravel's `withFallback(...)`.
    pub async fn generate_with_fallback<F, Fut>(
        &self,
        prompt: &str,
        fallback: F,
    ) -> Result<String, ProviderError>
    where
        F: FnOnce(ProviderError) -> Fut,
        Fut: std::future::Future<Output = Result<String, ProviderError>>,
    {
        match self.generate(prompt).await {
            Ok(text) => Ok(text),
            Err(error) => fallback(error).await,
        }
    }

    /// Stream a chat conversation, yielding incremental text chunks.
    pub async fn chat_stream(&self, messages: &[Message]) -> Result<ChatStream, ProviderError> {
        let options = ChatOptions {
            model: self.default_model.clone(),
            ..Default::default()
        };
        self.provider.chat_stream(messages, &options).await
    }

    /// Request structured (JSON) output and deserialize it into a typed value.
    pub async fn structured<T: DeserializeOwned>(&self, prompt: &str) -> Result<T, ProviderError> {
        let options = ChatOptions {
            model: self.default_model.clone(),
            response_format: Some(ResponseFormat::JsonObject),
            ..Default::default()
        };
        let response = self
            .provider
            .chat(&[Message::user(prompt)], &options)
            .await?;
        serde_json::from_str(&response.text).map_err(|error| {
            ProviderError::InvalidResponse(format!("structured output was not valid JSON: {error}"))
        })
    }

    /// Embed a single text input into a vector.
    ///
    /// When embedding caching is enabled, identical inputs reuse the cached
    /// embedding for 30 days instead of calling the provider.
    pub async fn embed(&self, input: &str) -> Result<Vec<f32>, ProviderError> {
        let key = embedding_cache_key(input);
        let cache = self.cache.as_deref();

        if let Some(cache) = cache {
            if let Ok(Some(cached)) = cache.get(&key).await {
                if let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&cached) {
                    return Ok(embedding);
                }
            }
        }

        let options = EmbeddingOptions {
            model: self.default_embedding_model.clone(),
        };
        let embedding = self.provider.embed(input, &options).await?;

        if let Some(cache) = cache {
            if let Ok(json) = serde_json::to_string(&embedding) {
                let _ = cache
                    .set(&key, &json, Some(self.embedding_cache_ttl.as_secs()))
                    .await;
            }
        }
        Ok(embedding)
    }

    /// Embed multiple text inputs into vectors.
    pub async fn embed_many(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        let options = EmbeddingOptions {
            model: self.default_embedding_model.clone(),
        };
        self.provider.embed_many(inputs, &options).await
    }

    /// Create an AI agent — Laravel's `Ai::agent('name')`. The agent inherits
    /// this manager's provider and default model; configure it with
    /// [`Agent::prompt`], [`Agent::using_tools`], and [`Agent::using_model`],
    /// then run it with [`Agent::ask`].
    pub fn agent(&self, name: impl Into<String>) -> Agent {
        let mut agent = Agent::new(name, self.provider.clone());
        if let Some(model) = &self.default_model {
            agent = agent.using_model(model.clone());
        }
        agent
    }
}

fn env_or(config: &crate::config::Config, key: &str, env: &str) -> Option<String> {
    config
        .get(key)
        .map(|value| decode_toml_string(&value))
        .or_else(|| std::env::var(env).ok())
}

/// `Config::get` returns TOML-encoded values (`"openai"` for strings); decode
/// them back to the raw value.
fn decode_toml_string(value: &str) -> String {
    match value.parse::<toml::Value>() {
        Ok(toml::Value::String(s)) => s,
        _ => value.to_string(),
    }
}

fn embedding_cache_key(input: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{}{}",
        EMBEDDING_CACHE_PREFIX,
        hex::encode(Sha256::digest(input.as_bytes()))
    )
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_trait;
    use crate::cache::CacheManager;

    /// A provider that always fails, for exercising fallback behaviour.
    #[derive(Debug)]
    struct FailingProvider;

    #[async_trait]
    impl AiProvider for FailingProvider {
        fn name(&self) -> &str {
            "failing"
        }

        async fn chat(
            &self,
            _messages: &[Message],
            _options: &ChatOptions,
        ) -> Result<ChatResponse, ProviderError> {
            Err(ProviderError::Request("primary is down".into()))
        }

        async fn chat_stream(
            &self,
            _messages: &[Message],
            _options: &ChatOptions,
        ) -> Result<ChatStream, ProviderError> {
            Err(ProviderError::Request("primary is down".into()))
        }

        async fn embed(
            &self,
            _input: &str,
            _options: &EmbeddingOptions,
        ) -> Result<Vec<f32>, ProviderError> {
            Err(ProviderError::Request("primary is down".into()))
        }

        async fn embed_many(
            &self,
            _inputs: &[String],
            _options: &EmbeddingOptions,
        ) -> Result<Vec<Vec<f32>>, ProviderError> {
            Err(ProviderError::Request("primary is down".into()))
        }
    }

    #[tokio::test]
    async fn test_generate_and_chat_with_fake() {
        let fake = FakeAi::new();
        fake.add_response("Hello, world!");
        let ai = Ai::new(Arc::new(fake));

        let text = ai.generate("Say hi").await.unwrap();
        assert_eq!(text, "Hello, world!");

        let response = ai
            .chat(&[Message::system("Be terse."), Message::user("Hi")])
            .await
            .unwrap();
        assert_eq!(response.text, "Fake response");
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    }

    #[tokio::test]
    async fn test_stream_with_fake() {
        use futures_util::StreamExt;
        let fake = FakeAi::new();
        fake.add_stream_response(vec!["one ".into(), "two".into()]);
        let ai = Ai::new(Arc::new(fake));

        let mut stream = ai.chat_stream(&[Message::user("Hi")]).await.unwrap();
        let mut chunks = Vec::new();
        while let Some(chunk) = stream.next().await {
            chunks.push(chunk.unwrap());
        }
        assert_eq!(chunks, vec!["one ".to_string(), "two".to_string()]);
    }

    #[tokio::test]
    async fn test_structured_with_fake() {
        let fake = FakeAi::new();
        fake.add_response(r#"{"city": "Jakarta", "temp": 31}"#);
        let ai = Ai::new(Arc::new(fake));

        #[derive(Debug, PartialEq, serde::Deserialize)]
        struct Weather {
            city: String,
            temp: u64,
        }

        let weather: Weather = ai.structured("Weather in Jakarta?").await.unwrap();
        assert_eq!(
            weather,
            Weather {
                city: "Jakarta".into(),
                temp: 31
            }
        );
    }

    #[tokio::test]
    async fn test_chat_with_fallback_primary_wins() {
        let fake = Arc::new(FakeAi::new());
        fake.add_response("primary");
        let ai = Ai::new(fake.clone());

        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        let response = ai
            .chat_with_fallback(&[Message::user("Hi")], move |_error| {
                called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                async { Err(ProviderError::Request("fallback".into())) }
            })
            .await
            .unwrap();

        assert_eq!(response.text, "primary");
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_chat_with_fallback_used_on_failure() {
        let ai = Ai::new(Arc::new(FailingProvider));

        let response = ai
            .chat_with_fallback(&[Message::user("Hi")], |error| async move {
                assert_eq!(error.to_string(), "AI provider error: primary is down");
                Ok(ChatResponse {
                    text: "fallback answer".into(),
                    usage: None,
                    finish_reason: Some("stop".into()),
                    tool_calls: Vec::new(),
                })
            })
            .await
            .unwrap();

        assert_eq!(response.text, "fallback answer");
    }

    #[tokio::test]
    async fn test_chat_with_fallback_error_propagates() {
        let ai = Ai::new(Arc::new(FailingProvider));

        let error = ai
            .chat_with_fallback(&[Message::user("Hi")], |_error| async {
                Err(ProviderError::Request("backup is down too".into()))
            })
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "AI provider error: backup is down too");
    }

    #[tokio::test]
    async fn test_generate_with_fallback() {
        let fake = Arc::new(FakeAi::new());
        fake.add_response("primary text");
        let ai = Ai::new(fake.clone());

        let text = ai
            .generate_with_fallback("Hi", |_error| async { Ok("fallback text".to_string()) })
            .await
            .unwrap();
        assert_eq!(text, "primary text");

        let ai = Ai::new(Arc::new(FailingProvider));
        let text = ai
            .generate_with_fallback("Hi", |_error| async { Ok("fallback text".to_string()) })
            .await
            .unwrap();
        assert_eq!(text, "fallback text");
    }

    #[tokio::test]
    async fn test_embed_uses_provider() {
        let fake = Arc::new(FakeAi::new());
        let ai = Ai::new(fake.clone());

        let embedding = ai.embed("hello").await.unwrap();
        assert_eq!(embedding.len(), 8);
        assert!(
            ai.embed_many(&["a".into(), "b".into()])
                .await
                .unwrap()
                .len()
                == 2
        );
        fake.assert_call_count(2);
    }

    #[tokio::test]
    async fn test_embed_caching_skips_provider() {
        let fake = Arc::new(FakeAi::new());
        let mut cache = CacheManager::new("array");
        cache.register("array", crate::cache::array::ArrayStore::new("array"));
        let ai = Ai::new(fake.clone())
            .with_model("gpt-4o-mini")
            .with_embedding_cache(Arc::new(cache), DEFAULT_EMBEDDING_CACHE_TTL_DAYS);

        let first = ai.embed("cache me").await.unwrap();
        let second = ai.embed("cache me").await.unwrap();
        assert_eq!(first, second);
        // Only one provider call — the second hit the cache.
        fake.assert_call_count(1);
    }

    #[test]
    fn test_from_config_openai_provider() {
        let toml = r#"
            [ai]
            provider = "openai"
            api_key = "sk-test"
            base_url = "https://api.example.test/v1"
            model = "custom-model"
            embedding_model = "custom-embed"
        "#;
        let config: crate::config::Config = toml::from_str(toml).unwrap();
        let ai = Ai::from_config(&config).unwrap();
        assert_eq!(ai.provider().name(), "openai");
        assert_eq!(ai.default_model.as_deref(), Some("custom-model"));
        assert_eq!(ai.default_embedding_model.as_deref(), Some("custom-embed"));
    }

    #[test]
    fn test_from_config_no_provider() {
        let config = crate::config::Config::default();
        match Ai::from_config(&config) {
            Err(ProviderError::NoProvider) => {}
            other => panic!("expected NoProvider, got {other:?}"),
        }
    }

    #[test]
    fn test_embedding_cache_key_stable() {
        let a = embedding_cache_key("same input");
        let b = embedding_cache_key("same input");
        let c = embedding_cache_key("different");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with(EMBEDDING_CACHE_PREFIX));
    }
}
