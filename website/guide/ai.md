# AI (Laravel AI SDK)

Larastvel ships a first-party AI SDK mirroring Laravel 13's `laravel/ai`:
a unified, provider-agnostic interface for text generation (with streaming
and structured output) and embeddings, plus testing fakes.

The foundation is implemented: `Ai` facade, `AiProvider` trait, an
OpenAI-compatible HTTP provider, 30-day embedding caching, and `FakeAi`.
Agents and media (images, audio, TTS/STT, reranking, vector stores) are the
next phase.

## Configuration

`config/ai.toml`:

```toml
[ai]
provider = "openai"
api_key = ""                      # or set AI_API_KEY
base_url = "https://api.openai.com/v1"
model = "gpt-4o-mini"
embedding_model = "text-embedding-3-small"

[ai.caching.embeddings]
cache = true                      # or set AI_EMBEDDING_CACHE=true
```

Every key has an `AI_*` environment variable fallback (`AI_API_KEY`,
`AI_BASE_URL`, `AI_MODEL`, `AI_EMBEDDING_MODEL`, `AI_EMBEDDING_CACHE`).

The `openai` provider speaks the standard OpenAI-compatible API, so it also
works with OpenAI-compatible endpoints such as Groq, DeepSeek, xAI,
OpenRouter, and local Ollama servers — point `base_url` at them and set
`api_key` accordingly.

## Getting Started

```rust
use larastvel_core::{ai::Ai, config::Config};

let config = Config::load("config");
let ai = Ai::from_config(&config)?;

// Simple text generation — Laravel's Ai::generate()
let summary = ai.generate("Summarize this changelog in one sentence").await?;

// Chat with messages
use larastvel_core::ai::Message;

let response = ai
    .chat(&[
        Message::system("You are a helpful assistant."),
        Message::user("What is Larastvel?"),
    ])
    .await?;

println!("{}", response.text);
```

`ChatResponse` exposes `text`, `usage` (token counts), and `finish_reason`.

## Options

Pass `ChatOptions` to fine-tune a request:

```rust
use larastvel_core::ai::{ChatOptions, ResponseFormat};

let options = ChatOptions {
    model: Some("gpt-4o".into()),          // override the default model
    temperature: Some(0.7),
    max_tokens: Some(2000),
    top_p: Some(0.9),
    stop: Some(vec!["END".into()]),
    response_format: Some(ResponseFormat::JsonObject),
    ..Default::default()
};

let response = ai.chat_with(&[Message::user("Hi")], &options).await?;
```

`ResponseFormat` also offers `Text` and `JsonSchema { name, schema }` for
schema-constrained JSON.

## Streaming

```rust
use futures_util::StreamExt;

let mut stream = ai.chat_stream(&[Message::user("Write a haiku")]).await?;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;   // each chunk is a text piece
    print!("{chunk}");
}
```

## Structured Output

Request JSON and deserialize directly into a typed value:

```rust
#[derive(serde::Deserialize)]
struct Weather {
    city: String,
    temp: u64,
}

let weather: Weather = ai
    .structured("Weather in Jakarta? Reply as JSON only.")
    .await?;
```

## Embeddings

```rust
let vector = ai.embed("search this document").await?;        // Vec<f32>
let vectors = ai.embed_many(&["one".into(), "two".into()]).await?;
```

Embeddings are cached for 30 days when `ai.caching.embeddings.cache` is
enabled, so identical inputs never hit the provider twice. Pair embeddings
with `VectorSimilarityQuery` for semantic search.

## Custom Providers

Implement `AiProvider` (chat, `chat_stream`, `embed`, `embed_many`) and
wrap it with `Ai::new`:

```rust
use std::sync::Arc;
use larastvel_core::ai::{Ai, AiProvider};

struct MyProvider;

// impl AiProvider for MyProvider { /* ... */ }

let ai = Ai::new(Arc::new(MyProvider))
    .with_model("my-model")
    .with_embedding_model("my-embedding-model");
```

`OpenAICompatibleProvider::new(base_url, api_key, model, embedding_model)`
is the built-in HTTP provider; use it directly for full control.

## Testing with FakeAi

`Ai::fake()` returns an `Ai` backed by a `FakeAi` — no network needed:

```rust
use std::sync::Arc;
use larastvel_core::ai::{Ai, FakeAi};

let fake = Arc::new(FakeAi::new());
fake.add_response("Hello, world!");          // queue canned chat responses
fake.add_stream_response(vec!["one ".into(), "two".into()]);

let ai = Ai::new(fake.clone());

assert_eq!(ai.generate("Say hi").await.unwrap(), "Hello, world!");
fake.assert_call_count(1);

// When the queue is empty, the fake answers with "Fake response".
```

Embeddings are deterministic hash-derived vectors, so tests can assert on
shapes and equality.
