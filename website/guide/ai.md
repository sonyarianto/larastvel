# AI (Laravel AI SDK)

Larastvel ships a first-party AI SDK mirroring Laravel 13's `laravel/ai`:
a unified, provider-agnostic interface for text generation (with streaming
and structured output) and embeddings, plus testing fakes.

The foundation is implemented: `Ai` facade, `AiProvider` trait, an
OpenAI-compatible HTTP provider, agents with tool calling, 30-day embedding
caching, and `FakeAi`. Media (images, audio, TTS/STT, reranking, vector
stores) is the next phase.

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

## Agents

Agents wrap a provider with a persona prompt, an optional model, and tools
the model can call — Laravel 13's `Ai::agent(...)->ask(...)`:

```rust
use std::sync::Arc;
use larastvel_core::ai::{AgentTool, Ai};
use serde_json::Value;

let ai = Ai::from_config(&config)?;

let tool = AgentTool::from("get_weather", "Get the weather for a city.")
    .with_parameters(serde_json::json!({
        "type": "object",
        "properties": { "city": { "type": "string" } },
        "required": ["city"],
    }))
    .using(|arguments: Value| {
        let city = arguments["city"].as_str().unwrap_or("Jakarta");
        Ok(serde_json::json!({ "city": city, "temp": 31 }))
    });

let agent = ai
    .agent("weather")
    .prompt("You are a helpful weather assistant.")
    .using_tools(vec![tool])
    .using_model("gpt-4o");

let task = agent.ask("What is the weather in Jakarta?").await?;

println!("{}", task.completion());   // the agent's final answer
```

`Agent::ask` runs the tool-calling loop: the model may request tool calls,
the framework executes them, feeds results back into the conversation, and
repeats until the model produces a final completion. Each run returns an
`AgentTask` with `id()`, `status()` (`AgentTaskStatus`), `messages()`, and
`result()` (`AgentResult::completion`). `Agent::run` is the Laravel 12
compatibility alias.

Details:

- **Tool handlers** receive the parsed JSON arguments and return a JSON
  value. Returning `Err(ToolError::new(name, msg))` feeds the error text
  back into the conversation so the model can recover; unknown tool names
  are likewise surfaced as tool messages.
- **Turn limit**: agents stop after 10 model turns by default (guarding
  against a model that never stops calling tools) — configure with
  `with_max_turns(n)`.
- **Agent model**: `using_model()` overrides the provider's default for
  that agent only.

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

`Ai::fake()` returns an `Ai` backed by a `FakeAi` — no network needed.
Fakes ignore tool definitions, so agent runs complete in a single turn:

```rust
use std::sync::Arc;
use larastvel_core::ai::{Ai, FakeAi};

let fake = Arc::new(FakeAi::new());
fake.add_response("Hello, world!");          // queue canned chat responses
fake.add_stream_response(vec!["one ".into(), "two".into()]);

let ai = Ai::new(fake.clone());

assert_eq!(ai.generate("Say hi").await.unwrap(), "Hello, world!");
fake.assert_call_count(1);

// Agents work against the fake too — one turn, canned completion.
let task = ai.agent("greeter").ask("Hi").await.unwrap();
assert_eq!(task.completion(), "Hello, world!");

// When the queue is empty, the fake answers with "Fake response".
```

Embeddings are deterministic hash-derived vectors, so tests can assert on
shapes and equality.
