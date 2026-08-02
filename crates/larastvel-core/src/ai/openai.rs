use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};

use super::messages::{ChatOptions, ChatResponse, EmbeddingOptions, Message, ResponseFormat};
use super::provider::{AiProvider, ChatStream, ProviderError};

/// An OpenAI-compatible chat / embeddings provider.
///
/// Speaks the OpenAI REST API (`/chat/completions`, `/embeddings`), which is
/// also implemented by Groq, DeepSeek, xAI, OpenRouter, Ollama, LM Studio,
/// and most local / hosted LLM gateways — mirroring the Laravel AI SDK's
/// provider-agnostic design.
#[derive(Debug, Clone)]
pub struct OpenAICompatibleProvider {
    base_url: String,
    api_key: String,
    model: String,
    embedding_model: String,
    client: reqwest::Client,
}

impl OpenAICompatibleProvider {
    /// Create a provider for an OpenAI-compatible endpoint.
    ///
    /// `base_url` should include the API prefix, e.g. `https://api.openai.com/v1`.
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        embedding_model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            embedding_model: embedding_model.into(),
            client: reqwest::Client::new(),
        }
    }

    /// The default chat model.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The default embedding model.
    pub fn embedding_model(&self) -> &str {
        &self.embedding_model
    }

    /// Override the default chat model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Override the default embedding model.
    pub fn with_embedding_model(mut self, model: impl Into<String>) -> Self {
        self.embedding_model = model.into();
        self
    }

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn embeddings_url(&self) -> String {
        format!("{}/embeddings", self.base_url)
    }

    fn chat_body(&self, messages: &[Message], options: &ChatOptions, streaming: bool) -> Value {
        let model = options.model.clone().unwrap_or_else(|| self.model.clone());

        let mut body = json!({
            "model": model,
            "messages": serde_json::to_value(messages).unwrap_or_default(),
        });

        if let Some(temperature) = options.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(max_tokens) = options.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(top_p) = options.top_p {
            body["top_p"] = json!(top_p);
        }
        if !options.stop.is_empty() {
            body["stop"] = json!(options.stop);
        }
        if let Some(format) = &options.response_format {
            body["response_format"] = match format {
                ResponseFormat::Text => json!({ "type": "text" }),
                ResponseFormat::JsonObject => json!({ "type": "json_object" }),
                ResponseFormat::JsonSchema { name, schema } => json!({
                    "type": "json_schema",
                    "json_schema": { "name": name, "schema": schema },
                }),
            };
        }
        if !options.tools.is_empty() {
            body["tools"] = serde_json::to_value(&options.tools).unwrap_or_default();
            body["tool_choice"] = json!("auto");
        }
        if streaming {
            body["stream"] = json!(true);
        }
        body
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, ProviderError> {
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, text));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))
    }
}

#[async_trait]
impl AiProvider for OpenAICompatibleProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn chat(
        &self,
        messages: &[Message],
        options: &ChatOptions,
    ) -> Result<ChatResponse, ProviderError> {
        let body = self.chat_body(messages, options, false);
        let result = self.post_json(&self.chat_url(), &body).await?;

        let text = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let finish_reason = result["choices"][0]["finish_reason"]
            .as_str()
            .map(str::to_string);
        let usage = result
            .get("usage")
            .cloned()
            .and_then(|usage| serde_json::from_value(usage).ok());
        let tool_calls = parse_tool_calls(&result);

        Ok(ChatResponse {
            text,
            usage,
            finish_reason,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        options: &ChatOptions,
    ) -> Result<ChatStream, ProviderError> {
        let body = self.chat_body(messages, options, true);
        let response = self
            .client
            .post(self.chat_url())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, text));
        }

        let byte_stream = response.bytes_stream();
        let stream = futures_util::stream::try_unfold(
            (byte_stream, Vec::<u8>::new()),
            |(mut stream, mut buffer)| async move {
                loop {
                    if let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = buffer.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&line).trim().to_string();
                        match parse_sse_line(&line) {
                            SseLine::Data(chunk) => {
                                return Ok(Some((chunk, (stream, buffer))));
                            }
                            SseLine::Done => return Ok(None),
                            SseLine::Other => continue,
                        }
                    }
                    match stream.next().await {
                        Some(Ok(bytes)) => buffer.extend_from_slice(&bytes),
                        Some(Err(e)) => {
                            return Err(ProviderError::Request(e.to_string()));
                        }
                        None => return Ok(None),
                    }
                }
            },
        );

        Ok(Box::pin(stream))
    }

    async fn embed(
        &self,
        input: &str,
        options: &EmbeddingOptions,
    ) -> Result<Vec<f32>, ProviderError> {
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| self.embedding_model.clone());
        let body = json!({ "model": model, "input": input });
        let result = self.post_json(&self.embeddings_url(), &body).await?;

        parse_embedding(&result).ok_or_else(|| {
            ProviderError::InvalidResponse("missing embedding in provider response".into())
        })
    }

    async fn embed_many(
        &self,
        inputs: &[String],
        options: &EmbeddingOptions,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| self.embedding_model.clone());
        let body = json!({ "model": model, "input": inputs });
        let result = self.post_json(&self.embeddings_url(), &body).await?;

        let embeddings: Vec<Vec<f32>> = result["data"]
            .as_array()
            .map(|data| {
                data.iter()
                    .filter_map(|entry| {
                        entry["embedding"].as_array().map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_f64)
                                .map(|v| v as f32)
                                .collect()
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        if embeddings.is_empty() {
            return Err(ProviderError::InvalidResponse(
                "missing embeddings in provider response".into(),
            ));
        }
        Ok(embeddings)
    }
}

fn parse_embedding(result: &Value) -> Option<Vec<f32>> {
    result["data"][0]["embedding"].as_array().map(|values| {
        values
            .iter()
            .filter_map(Value::as_f64)
            .map(|v| v as f32)
            .collect()
    })
}

fn parse_tool_calls(result: &Value) -> Vec<super::messages::ToolCall> {
    result["choices"][0]["message"]["tool_calls"]
        .as_array()
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| {
                    Some(super::messages::ToolCall {
                        id: call["id"].as_str()?.to_string(),
                        name: call["function"]["name"].as_str()?.to_string(),
                        arguments: match &call["function"]["arguments"] {
                            Value::String(raw) => serde_json::from_str(raw).unwrap_or(Value::Null),
                            other => other.clone(),
                        },
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

enum SseLine {
    /// A content chunk extracted from an SSE `data:` line.
    Data(String),
    /// The SSE `[DONE]` marker — end of stream.
    Done,
    /// Any other line (blank, comment, non-data, or a chunk without text).
    Other,
}

fn parse_sse_line(line: &str) -> SseLine {
    match line.trim().strip_prefix("data:") {
        Some(rest) => {
            let rest = rest.trim();
            if rest == "[DONE]" {
                return SseLine::Done;
            }
            match serde_json::from_str::<Value>(rest) {
                Ok(payload) => {
                    let content = payload["choices"][0]["delta"]["content"]
                        .as_str()
                        .unwrap_or_default();
                    if content.is_empty() {
                        SseLine::Other
                    } else {
                        SseLine::Data(content.to_string())
                    }
                }
                Err(_) => SseLine::Other,
            }
        }
        None => SseLine::Other,
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Spin up a tiny HTTP server that serves canned responses and records
    /// the last request body, so provider behaviour can be asserted offline.
    async fn mock_server(
        response: String,
        content_type: &'static str,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Option<String>>>) {
        mock_server_with_status(response, content_type, 200).await
    }

    async fn mock_server_with_status(
        response: String,
        content_type: &'static str,
        status: u16,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Option<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let captured_clone = captured.clone();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&request).to_string();
            if let Some(body_start) = request.find("\r\n\r\n") {
                let body = request[body_start + 4..].trim().to_string();
                *captured_clone.lock().unwrap() = Some(body);
            }
            let reason = if status == 200 { "OK" } else { "ERROR" };
            let headers = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status,
                reason,
                content_type,
                response.len()
            );
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        (format!("http://{addr}/v1"), captured)
    }

    fn provider(base: &str) -> OpenAICompatibleProvider {
        OpenAICompatibleProvider::new(base, "test-key", "gpt-4o-mini", "text-embedding-3-small")
    }

    #[tokio::test]
    async fn test_chat_completions() {
        let (base, captured) = mock_server(
            r#"{
                "choices": [
                    { "message": { "role": "assistant", "content": "Hello from the mock!" },
                      "finish_reason": "stop" }
                ],
                "usage": { "prompt_tokens": 9, "completion_tokens": 5, "total_tokens": 14 }
            }"#
            .into(),
            "application/json",
        )
        .await;

        let response = provider(&base)
            .chat(
                &[Message::system("You are terse."), Message::user("Hi")],
                &ChatOptions {
                    temperature: Some(0.5),
                    max_tokens: Some(64),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(response.text, "Hello from the mock!");
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert_eq!(response.usage.unwrap().total_tokens, Some(14));

        let body = captured.lock().unwrap().clone().unwrap();
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "Hi");
        assert_eq!(body["temperature"], 0.5);
        assert_eq!(body["max_tokens"], 64);
        assert!(body.get("stream").is_none());
    }

    #[tokio::test]
    async fn test_chat_option_model_override() {
        let (base, captured) = mock_server(
            r#"{"choices":[{"message":{"content":"ok"}}]}"#.into(),
            "application/json",
        )
        .await;

        provider(&base)
            .chat(
                &[Message::user("Hi")],
                &ChatOptions {
                    model: Some("gpt-4.1".into()),
                    response_format: Some(ResponseFormat::JsonObject),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let body: Value = serde_json::from_str(&captured.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(body["model"], "gpt-4.1");
        assert_eq!(body["response_format"]["type"], "json_object");
    }

    #[tokio::test]
    async fn test_chat_streaming_sse() {
        let sse = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let (base, _) = mock_server(sse.into(), "text/event-stream").await;

        let stream = provider(&base)
            .chat_stream(&[Message::user("Hi")], &ChatOptions::default())
            .await
            .unwrap();

        let chunks: Vec<String> = stream.map(|c| c.unwrap()).collect().await;
        assert_eq!(chunks, vec!["Hello".to_string(), " world".to_string()]);
    }

    #[tokio::test]
    async fn test_chat_streaming_fragmented_chunks() {
        // Split the SSE payload across TCP writes mid-line to exercise the
        // incremental line buffering.
        let (base, _) = mock_server(
            concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"He\"}}]}\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"llo\"}}]}\n\n",
                "data: [DONE]\n\n",
            )
            .into(),
            "text/event-stream",
        )
        .await;

        let stream = provider(&base)
            .chat_stream(&[Message::user("Hi")], &ChatOptions::default())
            .await
            .unwrap();

        let chunks: Vec<String> = stream.map(|c| c.unwrap()).collect().await;
        assert_eq!(chunks, vec!["He".to_string(), "llo".to_string()]);
    }

    #[tokio::test]
    async fn test_embed() {
        let (base, captured) = mock_server(
            r#"{"data": [{"embedding": [0.1, 0.2, 0.3]}]}"#.into(),
            "application/json",
        )
        .await;

        let embedding = provider(&base)
            .embed("hello world", &EmbeddingOptions::default())
            .await
            .unwrap();
        assert_eq!(embedding, vec![0.1, 0.2, 0.3]);

        let body: Value = serde_json::from_str(&captured.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(body["model"], "text-embedding-3-small");
        assert_eq!(body["input"], "hello world");
    }

    #[tokio::test]
    async fn test_embed_many() {
        let (base, captured) = mock_server(
            r#"{"data": [{"embedding": [1.0]}, {"embedding": [2.0]}]}"#.into(),
            "application/json",
        )
        .await;

        let embeddings = provider(&base)
            .embed_many(&["a".into(), "b".into()], &EmbeddingOptions::default())
            .await
            .unwrap();
        assert_eq!(embeddings, vec![vec![1.0], vec![2.0]]);

        let body: Value = serde_json::from_str(&captured.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(body["input"], json!(["a", "b"]));
    }

    #[tokio::test]
    async fn test_http_error_status() {
        let (base, _) = mock_server_with_status(
            r#"{"error":{"message":"rate limited"}}"#.into(),
            "application/json",
            429,
        )
        .await;

        let err = provider(&base)
            .chat(&[Message::user("Hi")], &ChatOptions::default())
            .await
            .unwrap_err();
        match err {
            ProviderError::Status(429, message) => {
                assert!(message.contains("rate limited"));
            }
            other => panic!("expected ProviderError::Status, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_sse_line() {
        assert!(matches!(
            parse_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}"),
            SseLine::Data(content) if content == "Hi"
        ));
        assert!(matches!(parse_sse_line("data: [DONE]"), SseLine::Done));
        assert!(matches!(parse_sse_line(""), SseLine::Other));
        assert!(matches!(
            parse_sse_line("data: {\"choices\":[{\"delta\":{}}]}"),
            SseLine::Other
        ));
    }
}
