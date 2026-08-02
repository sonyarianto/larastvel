use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// The role of a message in a chat conversation, mirroring Laravel's AI SDK
/// and the OpenAI chat completions format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A function call requested by the model, mirroring the OpenAI chat
/// completions `tool_calls` format.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// The provider-assigned call id, echoed back on the `tool` result
    /// message.
    pub id: String,
    /// The name of the tool to invoke.
    pub name: String,
    /// The parsed JSON arguments the model wants to pass to the tool.
    pub arguments: Value,
}

impl ToolCall {
    /// Create a tool call.
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

impl Serialize for ToolCall {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ToolCall", 3)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("type", "function")?;
        let function = serde_json::json!({
            "name": self.name,
            "arguments": self.arguments.to_string(),
        });
        state.serialize_field("function", &function)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ToolCall {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let id = value["id"]
            .as_str()
            .ok_or_else(|| serde::de::Error::custom("tool call missing id"))?;
        let name = value["function"]["name"]
            .as_str()
            .ok_or_else(|| serde::de::Error::custom("tool call missing function name"))?;
        let arguments = match &value["function"]["arguments"] {
            Value::String(raw) => serde_json::from_str(raw).unwrap_or(Value::Null),
            other => other.clone(),
        };
        Ok(ToolCall::new(id, name, arguments))
    }
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    /// The message text. `None` on assistant messages that carry tool calls.
    #[serde(default)]
    pub content: Option<String>,
    /// Function calls requested by the model (assistant messages only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// The id of the tool call this message answers (tool messages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// A system message — the model's instructions / persona.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// A user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// An assistant message (e.g. prior turns of a conversation).
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// An assistant message requesting tool calls, with an optional lead-in
    /// text (serialized as `content: null` when absent, per the OpenAI
    /// format).
    pub fn assistant_with_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_calls,
            tool_call_id: None,
        }
    }

    /// The result of a tool call, addressed by its call id.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// The message text, when present.
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }
}

/// A tool definition advertised to the model, mirroring the OpenAI chat
/// completions `tools` parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON schema for the tool's arguments.
    pub parameters: Value,
}

impl ToolDefinition {
    /// Create a tool definition from a JSON schema.
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

impl Serialize for ToolDefinition {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ToolDefinition", 2)?;
        state.serialize_field("type", "function")?;
        let function = serde_json::json!({
            "name": self.name,
            "description": self.description,
            "parameters": self.parameters,
        });
        state.serialize_field("function", &function)?;
        state.end()
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
    /// Tool definitions the model may call during the conversation.
    pub tools: Vec<ToolDefinition>,
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
    /// Function calls requested by the model, when any.
    pub tool_calls: Vec<ToolCall>,
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
        assert_eq!(system.content(), Some("You are helpful."));

        let user = Message::user("Hi");
        assert_eq!(user.role, Role::User);
        assert_eq!(user.content(), Some("Hi"));

        let assistant = Message::assistant("Hello!");
        assert_eq!(assistant.role, Role::Assistant);
        assert_eq!(assistant.content(), Some("Hello!"));

        let tool = Message::tool("call_1", r#"{"temp": 31}"#);
        assert_eq!(tool.role, Role::Tool);
        assert_eq!(tool.tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn test_message_serializes_to_openai_format() {
        let message = Message::user("What is the weather?");
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["role"], serde_json::json!("user"));
        assert_eq!(json["content"], serde_json::json!("What is the weather?"));
        assert!(json.get("tool_calls").is_none());
        assert!(json.get("tool_call_id").is_none());
    }

    #[test]
    fn test_tool_call_serializes_to_openai_format() {
        let message = Message::assistant_with_tool_calls(
            None,
            vec![ToolCall::new(
                "call_1",
                "get_weather",
                serde_json::json!({"city": "Jakarta"}),
            )],
        );
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["content"], serde_json::json!(null));
        assert_eq!(json["tool_calls"][0]["id"], "call_1");
        assert_eq!(json["tool_calls"][0]["type"], "function");
        assert_eq!(json["tool_calls"][0]["function"]["name"], "get_weather");
        assert_eq!(
            json["tool_calls"][0]["function"]["arguments"],
            r#"{"city":"Jakarta"}"#
        );
    }

    #[test]
    fn test_tool_call_round_trip_deserialization() {
        let raw = serde_json::json!({
            "id": "call_1",
            "type": "function",
            "function": {
                "name": "get_weather",
                "arguments": "{\"city\":\"Jakarta\"}",
            },
        });
        let call: ToolCall = serde_json::from_value(raw).unwrap();
        assert_eq!(call.id, "call_1");
        assert_eq!(call.name, "get_weather");
        assert_eq!(call.arguments, serde_json::json!({"city": "Jakarta"}));
    }

    #[test]
    fn test_tool_definition_serializes_nested() {
        let definition = ToolDefinition::new(
            "get_weather",
            "Get the weather for a city.",
            serde_json::json!({"type": "object", "properties": {"city": {"type": "string"}}}),
        );
        let json = serde_json::to_value(definition).unwrap();
        assert_eq!(json["type"], "function");
        assert_eq!(json["function"]["name"], "get_weather");
        assert_eq!(
            json["function"]["parameters"]["properties"]["city"]["type"],
            "string"
        );
    }

    #[test]
    fn test_tool_message_serializes_role_and_call_id() {
        let message = Message::tool("call_1", r#"{"temp": 31}"#);
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["role"], serde_json::json!("tool"));
        assert_eq!(json["tool_call_id"], serde_json::json!("call_1"));
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
            tool_calls: Vec::new(),
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
