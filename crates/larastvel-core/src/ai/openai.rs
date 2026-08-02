use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};

use super::image::{ImageOptions, ImageResponse, ImageResult};
use super::media::{AudioOptions, Media};
use super::messages::{ChatOptions, ChatResponse, EmbeddingOptions, Message, ResponseFormat};
use super::moderation::{ModerationCategory, ModerationResponse};
use super::provider::{AiProvider, ChatStream, ProviderError};
use super::rerank::{RerankOptions, RerankResponse, RerankResult};

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

    fn images_generations_url(&self) -> String {
        format!("{}/images/generations", self.base_url)
    }

    fn images_edits_url(&self) -> String {
        format!("{}/images/edits", self.base_url)
    }

    fn images_variations_url(&self) -> String {
        format!("{}/images/variations", self.base_url)
    }

    fn audio_speech_url(&self) -> String {
        format!("{}/audio/speech", self.base_url)
    }

    fn audio_transcriptions_url(&self) -> String {
        format!("{}/audio/transcriptions", self.base_url)
    }

    fn moderations_url(&self) -> String {
        format!("{}/moderations", self.base_url)
    }

    fn rerank_url(&self) -> String {
        format!("{}/rerank", self.base_url)
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

const DEFAULT_RERANK_MODEL: &str = "gpt-5-mini";

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

    async fn image_create(
        &self,
        prompt: &str,
        options: &ImageOptions,
    ) -> Result<ImageResponse, ProviderError> {
        let body = self.image_body(prompt, options);
        let result = self
            .post_json(&self.images_generations_url(), &body)
            .await?;
        parse_image_response(&result)
    }

    async fn image_edit(
        &self,
        image: &Media,
        prompt: &str,
        options: &ImageOptions,
    ) -> Result<ImageResponse, ProviderError> {
        let form = reqwest::multipart::Form::new()
            .text("prompt", prompt.to_string())
            .part("image", media_part(image, "image.png")?);
        let form = self.image_form_options(form, options);
        let result = self.post_multipart(&self.images_edits_url(), form).await?;
        parse_image_response(&result)
    }

    async fn image_variation(
        &self,
        image: &Media,
        options: &ImageOptions,
    ) -> Result<ImageResponse, ProviderError> {
        let form = reqwest::multipart::Form::new().part("image", media_part(image, "image.png")?);
        let form = self.image_form_options(form, options);
        let result = self
            .post_multipart(&self.images_variations_url(), form)
            .await?;
        parse_image_response(&result)
    }

    async fn tts(&self, text: &str, options: &AudioOptions) -> Result<Vec<u8>, ProviderError> {
        let mut body = json!({
            "model": "tts-1",
            "input": text,
            "voice": options.voice.clone().unwrap_or_else(|| "alloy".into()),
        });
        if let Some(format) = &options.format {
            body["response_format"] = json!(format);
        }
        if let Some(speed) = options.speed {
            body["speed"] = json!(speed);
        }
        let response = self
            .client
            .post(self.audio_speech_url())
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
        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))
    }

    async fn stt(&self, audio: &Media, options: &AudioOptions) -> Result<String, ProviderError> {
        let mut form = reqwest::multipart::Form::new()
            .text("model", "whisper-1".to_string())
            .part("file", media_part(audio, "audio.bin")?);
        if let Some(language) = &options.language {
            form = form.text("language", language.clone());
        }
        let result = self
            .post_multipart(&self.audio_transcriptions_url(), form)
            .await?;
        result["text"].as_str().map(str::to_string).ok_or_else(|| {
            ProviderError::InvalidResponse("missing transcript in provider response".into())
        })
    }

    async fn moderate(&self, content: &str) -> Result<ModerationResponse, ProviderError> {
        let body = json!({ "model": "omni-moderation-latest", "input": content });
        let result = self.post_json(&self.moderations_url(), &body).await?;
        let entry = result["results"]
            .as_array()
            .and_then(|results| results.first())
            .ok_or_else(|| ProviderError::InvalidResponse("missing moderation results".into()))?;
        let flagged = entry["flagged"].as_bool().unwrap_or(false);
        let categories = entry["categories"]
            .as_object()
            .map(|map| {
                map.iter()
                    .map(|(id, value)| ModerationCategory {
                        id: id.clone(),
                        flagged: value.as_bool().unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(ModerationResponse {
            flagged,
            categories,
        })
    }

    async fn rerank(
        &self,
        query: &str,
        documents: &[String],
        options: &RerankOptions,
    ) -> Result<RerankResponse, ProviderError> {
        let body = json!({
            "model": options.model.clone().unwrap_or_else(|| DEFAULT_RERANK_MODEL.to_string()),
            "query": query,
            "documents": documents,
        });
        let result = self.post_json(&self.rerank_url(), &body).await?;
        let results = result["results"]
            .as_array()
            .map(|results| {
                results
                    .iter()
                    .filter_map(|entry| {
                        Some(RerankResult {
                            index: entry["index"].as_u64()? as usize,
                            relevance_score: entry["relevance_score"].as_f64()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(RerankResponse { results })
    }
}

impl OpenAICompatibleProvider {
    fn image_body(&self, prompt: &str, options: &ImageOptions) -> Value {
        let mut body = json!({ "model": "dall-e-3", "prompt": prompt });
        if let Some(size) = &options.size {
            body["size"] = json!(size);
        }
        if let Some(quality) = &options.quality {
            body["quality"] = json!(quality);
        }
        if let Some(format) = &options.response_format {
            body["response_format"] = json!(format);
        }
        if let Some(n) = options.n {
            body["n"] = json!(n);
        }
        body
    }

    fn image_form_options(
        &self,
        form: reqwest::multipart::Form,
        options: &ImageOptions,
    ) -> reqwest::multipart::Form {
        let mut form = form;
        if let Some(size) = &options.size {
            form = form.text("size", size.clone());
        }
        if let Some(quality) = &options.quality {
            form = form.text("quality", quality.clone());
        }
        if let Some(n) = options.n {
            form = form.text("n", n.to_string());
        }
        form
    }

    async fn post_multipart(
        &self,
        url: &str,
        form: reqwest::multipart::Form,
    ) -> Result<Value, ProviderError> {
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .multipart(form)
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

fn media_part(
    media: &Media,
    fallback_name: &str,
) -> Result<reqwest::multipart::Part, ProviderError> {
    let part = match reqwest::multipart::Part::bytes(media.content().to_vec())
        .mime_str(media.mime_type())
    {
        Ok(part) => part,
        Err(_) => reqwest::multipart::Part::bytes(media.content().to_vec()),
    };
    Ok(part.file_name(fallback_name.to_string()))
}

fn parse_image_response(result: &Value) -> Result<ImageResponse, ProviderError> {
    let data: Vec<ImageResult> = result["data"]
        .as_array()
        .map(|data| {
            data.iter()
                .map(|entry| ImageResult {
                    url: entry["url"].as_str().map(str::to_string),
                    b64_json: entry["b64_json"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();
    if data.is_empty() {
        return Err(ProviderError::InvalidResponse(
            "missing images in provider response".into(),
        ));
    }
    Ok(ImageResponse { data })
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

    #[tokio::test]
    async fn test_image_create() {
        let (base, captured) = mock_server(
            r#"{"data": [{"url": "https://example.test/a.png"}]}"#.into(),
            "application/json",
        )
        .await;

        let response = provider(&base)
            .image_create(
                "a red panda",
                &ImageOptions {
                    size: Some("1024x1024".into()),
                    quality: Some("hd".into()),
                    response_format: Some("url".into()),
                    n: Some(2),
                },
            )
            .await
            .unwrap();

        assert_eq!(
            response.first().unwrap().url.as_deref(),
            Some("https://example.test/a.png")
        );
        let body: Value = serde_json::from_str(&captured.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(body["model"], "dall-e-3");
        assert_eq!(body["prompt"], "a red panda");
        assert_eq!(body["size"], "1024x1024");
        assert_eq!(body["quality"], "hd");
        assert_eq!(body["n"], 2);
    }

    #[tokio::test]
    async fn test_image_edit_is_multipart() {
        let (base, captured) = mock_server(
            r#"{"data": [{"b64_json": "aGVsbG8="}]}"#.into(),
            "application/json",
        )
        .await;
        let image = Media::image(vec![0x89, 0x50], "image/png");

        let response = provider(&base)
            .image_edit(&image, "add sunglasses", &ImageOptions::default())
            .await
            .unwrap();

        let bytes = response.first().unwrap().bytes().unwrap();
        assert_eq!(bytes, b"hello");
        let body = captured.lock().unwrap().clone().unwrap();
        assert!(body.contains(r#"name="prompt""#), "body: {body}");
        assert!(body.contains("add sunglasses"), "body: {body}");
        assert!(body.contains(r#"name="image""#), "body: {body}");
        assert!(body.contains("filename=\"image.png\""), "body: {body}");
        assert!(body.contains("image/png"), "body: {body}");
    }

    #[tokio::test]
    async fn test_tts_returns_audio_bytes() {
        let (base, captured) = mock_server("ID3fakeaudio".into(), "audio/mpeg").await;

        let audio = provider(&base)
            .tts(
                "Hello there",
                &AudioOptions {
                    voice: Some("shimmer".into()),
                    format: Some("mp3".into()),
                    speed: Some(1.25),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(audio, b"ID3fakeaudio");
        let body: Value = serde_json::from_str(&captured.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(body["model"], "tts-1");
        assert_eq!(body["input"], "Hello there");
        assert_eq!(body["voice"], "shimmer");
        assert_eq!(body["response_format"], "mp3");
        assert_eq!(body["speed"], 1.25);
    }

    #[tokio::test]
    async fn test_stt_transcribes() {
        let (base, captured) = mock_server(
            r#"{"text": "Hello world transcript"}"#.into(),
            "application/json",
        )
        .await;
        let audio = Media::audio(vec![0x00, 0x01, 0x02], "audio/mpeg");

        let transcript = provider(&base)
            .stt(
                &audio,
                &AudioOptions {
                    language: Some("en".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(transcript, "Hello world transcript");
        let body = captured.lock().unwrap().clone().unwrap();
        assert!(body.contains("whisper-1"), "body: {body}");
        assert!(body.contains(r#"name="language""#), "body: {body}");
        assert!(body.contains(r#"name="file""#), "body: {body}");
    }

    #[tokio::test]
    async fn test_moderate() {
        let (base, captured) = mock_server(
            r#"{
                "results": [{
                    "flagged": true,
                    "categories": { "violence": true, "harassment": false }
                }]
            }"#
            .into(),
            "application/json",
        )
        .await;

        let result = provider(&base).moderate("kill them all").await.unwrap();

        assert!(result.flagged);
        assert!(result.is_flagged("violence"));
        assert!(!result.is_flagged("harassment"));
        let body: Value = serde_json::from_str(&captured.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(body["input"], "kill them all");
    }

    #[tokio::test]
    async fn test_image_create_missing_data_is_invalid() {
        let (base, _) = mock_server(r#"{"data": []}"#.into(), "application/json").await;
        let error = provider(&base)
            .image_create("x", &ImageOptions::default())
            .await
            .unwrap_err();
        assert!(matches!(error, ProviderError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn test_moderation_missing_results_is_invalid() {
        let (base, _) = mock_server(r#"{"results": []}"#.into(), "application/json").await;
        let error = provider(&base).moderate("x").await.unwrap_err();
        assert!(matches!(error, ProviderError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn test_rerank() {
        let (base, captured) = mock_server(
            r#"{"results": [{"index": 2, "relevance_score": 0.9}, {"index": 0, "relevance_score": 0.3}]}"#.into(),
            "application/json",
        )
        .await;

        let response = provider(&base)
            .rerank(
                "Where is the nearest coffee shop?",
                &["Mall A".into(), "Mall B".into(), "Cafe C".into()],
                &RerankOptions {
                    model: Some("custom-rerank".into()),
                },
            )
            .await
            .unwrap();

        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].index, 2);
        assert_eq!(response.results[0].relevance_score, 0.9);
        assert_eq!(response.best(), Some(2));

        let body: Value = serde_json::from_str(&captured.lock().unwrap().clone().unwrap()).unwrap();
        assert_eq!(body["model"], "custom-rerank");
        assert_eq!(body["query"], "Where is the nearest coffee shop?");
        assert_eq!(body["documents"][1], "Mall B");
    }
}
