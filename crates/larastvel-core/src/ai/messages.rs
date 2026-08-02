use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The role of a message in a chat conversation, mirroring Laravel's AI SDK
/// and the OpenAI chat completions format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    /// A system message — the model's instructions / persona.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// A user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// An assistant message (e.g. prior turns of a conversation).
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// The requested response format for a chat completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResponseFormat {
    /// Plain text (default).
    #[serde(rename = "text")]
    Text,
    /// A JSON object — parsed from the response with `ChatResponse::structured`.
    #[serde(rename = "json_object")]
    JsonObject,
    /// A JSON object matching the given JSON schema.
    #[serde(rename = "json_schema")]
    JsonSchema { name: String, schema: Value },
}

/// Options for a chat completion request.
///
/// `None` / empty values defer to the provider's defaults, mirroring the
/// Laravel AI SDK's `withTemperature()`, `withMaxTokens()`, etc.
#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    /// Override the provider's default model.
    pub model: Option<String>,
    /// Sampling temperature (0.0–2.0).
    pub temperature: Option<f32>,
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Nucleus sampling (0.0–1.0).
    pub top_p: Option<f32>,
    /// Sequences where generation should stop.
    pub stop: Vec<String>,
    /// Request structured (JSON) output.
    pub response_format: Option<ResponseFormat>,
}

/// Options for an embedding request.
#[derive(Debug, Clone, Default)]
pub struct EmbeddingOptions {
    /// Override the provider's default embedding model.
    pub model: Option<String>,
}

/// Token usage reported by the provider, mirroring OpenAI's `usage` object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

/// A completed chat response.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// The generated text (or JSON for structured output).
    pub text: String,
    /// Token usage, when reported by the provider.
    pub usage: Option<Usage>,
    /// The provider's finish reason (e.g. `stop`, `length`, `tool_calls`).
    pub finish_reason: Option<String>,
}

impl ChatResponse {
    /// Deserialize a structured (JSON) response into a typed value.
    pub fn structured<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.text)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_constructors() {
        let system = Message::system("You are helpful.");
        assert_eq!(system.role, Role::System);
        assert_eq!(system.content, "You are helpful.");

        let user = Message::user("Hi");
        assert_eq!(user.role, Role::User);
        assert_eq!(user.content, "Hi");

        let assistant = Message::assistant("Hello!");
        assert_eq!(assistant.role, Role::Assistant);
        assert_eq!(assistant.content, "Hello!");
    }

    #[test]
    fn test_message_serializes_to_openai_format() {
        let message = Message::user("What is the weather?");
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["role"], serde_json::json!("user"));
        assert_eq!(json["content"], serde_json::json!("What is the weather?"));
    }

    #[test]
    fn test_response_format_serde_names() {
        assert_eq!(
            serde_json::to_string(&ResponseFormat::Text).unwrap(),
            r#""text""#
        );
        assert_eq!(
            serde_json::to_string(&ResponseFormat::JsonObject).unwrap(),
            r#""json_object""#
        );
    }

    #[test]
    fn test_chat_response_structured() {
        let response = ChatResponse {
            text: r#"{"city": "London", "temp": 12}"#.into(),
            usage: None,
            finish_reason: None,
        };
        #[derive(Debug, PartialEq, Deserialize)]
        struct Weather {
            city: String,
            temp: u64,
        }
        let weather: Weather = response.structured().unwrap();
        assert_eq!(
            weather,
            Weather {
                city: "London".into(),
                temp: 12
            }
        );
    }

    #[test]
    fn test_usage_deserialization() {
        let usage: Usage = serde_json::from_str(
            r#"{"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}"#,
        )
        .unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.total_tokens, Some(15));
    }
}
