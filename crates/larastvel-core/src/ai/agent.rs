//! AI agents with tool calling — the Rust equivalent of Laravel 13's
//! `Agent` / `AgentTask` from `laravel/ai`.
//!
//! An agent wraps a provider with a persona prompt, an optional model, and a
//! set of tools. [`Agent::ask`] runs the tool-calling loop: the model may
//! request tool calls, the framework executes them, feeds the results back
//! into the conversation, and repeats until the model produces a final
//! completion.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::Value;

use super::messages::{ChatOptions, Message, ToolDefinition};
use super::provider::{AiProvider, ProviderError};

/// How many model turns an agent runs before giving up, guarding against a
/// model that never stops requesting tool calls.
pub const DEFAULT_AGENT_MAX_TURNS: usize = 10;

/// An error raised while executing a tool.
#[derive(Debug, thiserror::Error)]
#[error("agent tool '{name}' failed: {message}")]
pub struct ToolError {
    /// The tool that failed.
    pub name: String,
    /// The failure message, fed back to the model so it can recover.
    pub message: String,
}

impl ToolError {
    /// Create a tool error.
    pub fn new(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
        }
    }
}

/// A tool an agent can call — a name, description, JSON schema, and the
/// handler executed when the model invokes it, mirroring Laravel 13's
/// `AgentTool::from(...)->using(...)`.
pub struct AgentTool {
    name: String,
    description: String,
    parameters: Value,
    handler: Arc<dyn Fn(Value) -> Result<Value, ToolError> + Send + Sync>,
}

impl fmt::Debug for AgentTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("parameters", &self.parameters)
            .finish_non_exhaustive()
    }
}

impl AgentTool {
    /// Start defining a tool with its name and description — Laravel's
    /// `AgentTool::from()`. Attach behaviour with [`AgentTool::using`].
    pub fn from(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: Value::Object(Default::default()),
            handler: Arc::new(|_| Ok(Value::Null)),
        }
    }

    /// Attach the handler executed when the model calls the tool. The
    /// handler receives the parsed JSON arguments and returns a JSON value
    /// (or an error, which is fed back into the conversation so the model
    /// can recover).
    pub fn using<F>(mut self, handler: F) -> Self
    where
        F: Fn(Value) -> Result<Value, ToolError> + Send + Sync + 'static,
    {
        self.handler = Arc::new(handler);
        self
    }

    /// Declare the tool's argument JSON schema.
    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.parameters = parameters;
        self
    }

    /// The tool's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The definition advertised to the model.
    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            self.name.clone(),
            self.description.clone(),
            self.parameters.clone(),
        )
    }
}

/// The lifecycle status of an [`AgentTask`], mirroring Laravel's
/// `AgentTaskStatus` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTaskStatus {
    /// The task is still running.
    Running,
    /// The task completed with a final completion.
    Completed,
    /// The task failed.
    Failed,
}

/// The final result of an [`AgentTask`], mirroring Laravel's `AgentResult`.
#[derive(Debug, Clone)]
pub struct AgentResult {
    completion: String,
}

impl AgentResult {
    /// The agent's final completion text.
    pub fn completion(&self) -> &str {
        &self.completion
    }
}

/// The record of one agent run, mirroring Laravel's `AgentTask`.
#[derive(Debug, Clone)]
pub struct AgentTask {
    id: String,
    status: AgentTaskStatus,
    messages: Vec<Message>,
    result: AgentResult,
}

impl AgentTask {
    /// A unique id for the run.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The task's status.
    pub fn status(&self) -> AgentTaskStatus {
        self.status
    }

    /// The full conversation, including tool calls and their results.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// The task's final result.
    pub fn result(&self) -> &AgentResult {
        &self.result
    }

    /// The agent's final completion text — `result().completion()`.
    pub fn completion(&self) -> &str {
        self.result.completion()
    }
}

/// An AI agent: a persona, optional model, and tools, executed against a
/// provider — the Rust equivalent of Laravel 13's `Ai::agent(...)`.
pub struct Agent {
    name: String,
    provider: Arc<dyn AiProvider>,
    prompt: Option<String>,
    tools: Vec<AgentTool>,
    model: Option<String>,
    max_turns: usize,
}

impl fmt::Debug for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Agent")
            .field("name", &self.name)
            .field("prompt", &self.prompt)
            .field("tools", &self.tools)
            .field("model", &self.model)
            .field("max_turns", &self.max_turns)
            .finish_non_exhaustive()
    }
}

impl Agent {
    /// Create an agent around the given provider.
    pub fn new(name: impl Into<String>, provider: Arc<dyn AiProvider>) -> Self {
        Self {
            name: name.into(),
            provider,
            prompt: None,
            tools: Vec::new(),
            model: None,
            max_turns: DEFAULT_AGENT_MAX_TURNS,
        }
    }

    /// The agent's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set the agent's persona prompt — Laravel's `->prompt(...)`.
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Give the agent tools it can call — Laravel's `->usingTools(...)`.
    pub fn using_tools(mut self, tools: Vec<AgentTool>) -> Self {
        self.tools = tools;
        self
    }

    /// Override the model used for this agent — Laravel's
    /// `->usingModel(...)`.
    pub fn using_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Cap the number of model turns before the run errors (default 10).
    pub fn with_max_turns(mut self, turns: usize) -> Self {
        self.max_turns = turns.max(1);
        self
    }

    /// Run the agent against a query, returning an [`AgentTask`] — Laravel
    /// 13's `$agent->ask(...)`.
    pub async fn ask(&self, query: &str) -> Result<AgentTask, ProviderError> {
        let mut messages = Vec::new();
        if let Some(prompt) = &self.prompt {
            messages.push(Message::system(prompt));
        }
        messages.push(Message::user(query));

        let tools: Vec<ToolDefinition> = self.tools.iter().map(AgentTool::definition).collect();

        for _ in 0..self.max_turns {
            let options = ChatOptions {
                model: self.model.clone(),
                tools: tools.clone(),
                ..Default::default()
            };
            let response = self.provider.chat(&messages, &options).await?;

            if response.tool_calls.is_empty() {
                let completion = response.text;
                messages.push(Message::assistant(&completion));
                let task = AgentTask {
                    id: agent_task_id(),
                    status: AgentTaskStatus::Completed,
                    messages,
                    result: AgentResult { completion },
                };
                return Ok(task);
            }

            let content = if response.text.is_empty() {
                None
            } else {
                Some(response.text)
            };
            messages.push(Message::assistant_with_tool_calls(
                content,
                response.tool_calls.clone(),
            ));

            for call in &response.tool_calls {
                let result = match self.tools.iter().find(|tool| tool.name == call.name) {
                    Some(tool) => match (tool.handler)(call.arguments.clone()) {
                        Ok(value) => value.to_string(),
                        Err(error) => error.to_string(),
                    },
                    None => format!("Unknown tool: {}", call.name),
                };
                messages.push(Message::tool(&call.id, result));
            }
        }

        Err(ProviderError::Request(format!(
            "agent '{}' exceeded its {} turn limit without producing a final response",
            self.name, self.max_turns
        )))
    }

    /// Run the agent — Laravel 12's `$agent->run(...)`, kept as an alias of
    /// [`Agent::ask`].
    pub async fn run(&self, query: &str) -> Result<AgentTask, ProviderError> {
        self.ask(query).await
    }
}

static AGENT_TASK_COUNTER: AtomicU64 = AtomicU64::new(0);

fn agent_task_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let counter = AGENT_TASK_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{counter:x}")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::FakeAi;
    use crate::ai::Role;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A mock server that serves one scripted response per connection,
    /// recording each request body.
    async fn mock_server_multi(
        responses: Vec<String>,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let bodies = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let bodies_clone = bodies.clone();

        tokio::spawn(async move {
            for response in responses {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
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
                    bodies_clone
                        .lock()
                        .unwrap()
                        .push(request[body_start + 4..].trim().to_string());
                }
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.len()
                );
                let _ = socket.write_all(headers.as_bytes()).await;
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        (format!("http://{addr}/v1"), bodies)
    }

    fn tool_response(call_id: &str, name: &str, arguments: &str) -> String {
        format!(
            r#"{{
                "choices": [{{
                    "message": {{
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{{
                            "id": "{call_id}",
                            "type": "function",
                            "function": {{ "name": "{name}", "arguments": "{arguments}" }}
                        }}]
                    }},
                    "finish_reason": "tool_calls"
                }}]
            }}"#
        )
    }

    fn text_response(text: &str) -> String {
        format!(
            r#"{{"choices": [{{"message": {{"role": "assistant", "content": "{text}"}}, "finish_reason": "stop"}}]}}"#
        )
    }

    #[tokio::test]
    async fn test_agent_tool_round_trip() {
        let (base, bodies) = mock_server_multi(vec![
            tool_response("call_1", "get_weather", r#"{\"city\":\"Jakarta\"}"#),
            text_response("Sunny, 31C."),
        ])
        .await;
        let provider: Arc<dyn AiProvider> = Arc::new(crate::ai::OpenAICompatibleProvider::new(
            &base,
            "test-key",
            "gpt-4o-mini",
            "text-embedding-3-small",
        ));

        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let calls_clone = calls.clone();
        let tool = AgentTool::from("get_weather", "Get the weather for a city.")
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": { "city": { "type": "string" } },
                "required": ["city"],
            }))
            .using(move |arguments| {
                calls_clone.lock().unwrap().push(arguments.clone());
                Ok(serde_json::json!({ "temp": 31 }))
            });

        let agent = Agent::new("weather", provider)
            .prompt("You are a weather assistant.")
            .using_tools(vec![tool]);

        let task = agent.ask("What is the weather in Jakarta?").await.unwrap();

        assert_eq!(task.status(), AgentTaskStatus::Completed);
        assert_eq!(task.completion(), "Sunny, 31C.");
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [serde_json::json!({"city": "Jakarta"})]
        );

        let bodies = bodies.lock().unwrap();
        let first: Value = serde_json::from_str(&bodies[0]).unwrap();
        assert_eq!(first["tools"][0]["type"], "function");
        assert_eq!(first["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(first["tool_choice"], "auto");
        assert_eq!(first["messages"][0]["role"], "system");
        assert_eq!(
            first["messages"][1]["content"],
            "What is the weather in Jakarta?"
        );

        let second: Value = serde_json::from_str(&bodies[1]).unwrap();
        let messages = second["messages"].as_array().unwrap();
        let tool_message = messages
            .iter()
            .find(|m| m["role"] == "tool")
            .expect("second request must carry the tool result");
        assert_eq!(tool_message["tool_call_id"], "call_1");
        assert_eq!(tool_message["content"], r#"{"temp":31}"#);
        let assistant = messages
            .iter()
            .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some())
            .expect("second request must carry the assistant tool call");
        assert_eq!(
            assistant["tool_calls"][0]["function"]["name"],
            "get_weather"
        );
    }

    #[tokio::test]
    async fn test_agent_without_tools() {
        let (base, bodies) = mock_server_multi(vec![text_response("Hello!")]).await;
        let provider: Arc<dyn AiProvider> = Arc::new(crate::ai::OpenAICompatibleProvider::new(
            &base,
            "test-key",
            "gpt-4o-mini",
            "text-embedding-3-small",
        ));

        let task = Agent::new("chatty", provider).ask("Hi").await.unwrap();

        assert_eq!(task.completion(), "Hello!");
        assert_eq!(bodies.lock().unwrap().len(), 1);
        let body: Value = serde_json::from_str(&bodies.lock().unwrap()[0]).unwrap();
        assert!(body.get("tools").is_none());
    }

    #[tokio::test]
    async fn test_agent_tool_error_feeds_back_to_model() {
        let (base, _) = mock_server_multi(vec![
            tool_response("call_1", "boom", "{}"),
            text_response("I could not do that."),
        ])
        .await;
        let provider: Arc<dyn AiProvider> = Arc::new(crate::ai::OpenAICompatibleProvider::new(
            &base,
            "test-key",
            "gpt-4o-mini",
            "text-embedding-3-small",
        ));

        let tool = AgentTool::from("boom", "Always fails.")
            .using(|_| Err(ToolError::new("boom", "permission denied")));

        let agent = Agent::new("failing", provider).using_tools(vec![tool]);

        let task = agent.ask("do it").await.unwrap();
        assert_eq!(task.completion(), "I could not do that.");
        let tool_messages: Vec<&Message> = task
            .messages()
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(tool_messages.len(), 1);
        assert!(tool_messages[0]
            .content()
            .unwrap()
            .contains("permission denied"));
    }

    #[tokio::test]
    async fn test_agent_unknown_tool_continues() {
        let (base, _) = mock_server_multi(vec![
            tool_response("call_1", "missing_tool", "{}"),
            text_response("ok"),
        ])
        .await;
        let provider: Arc<dyn AiProvider> = Arc::new(crate::ai::OpenAICompatibleProvider::new(
            &base,
            "test-key",
            "gpt-4o-mini",
            "text-embedding-3-small",
        ));

        let agent = Agent::new("solo", provider);
        let task = agent.ask("hi").await.unwrap();
        assert_eq!(task.completion(), "ok");
        assert!(task.messages().iter().any(|m| m
            .content()
            .is_some_and(|c| c.contains("Unknown tool: missing_tool"))));
    }

    #[tokio::test]
    async fn test_agent_max_turns_guard() {
        let (base, _) = mock_server_multi(vec![
            tool_response("call_1", "loop", "{}"),
            tool_response("call_2", "loop", "{}"),
        ])
        .await;
        let provider: Arc<dyn AiProvider> = Arc::new(crate::ai::OpenAICompatibleProvider::new(
            &base,
            "test-key",
            "gpt-4o-mini",
            "text-embedding-3-small",
        ));

        let tool = AgentTool::from("loop", "Never finishes.").using(|_| Ok(Value::Null));
        let agent = Agent::new("looper", provider)
            .using_tools(vec![tool])
            .with_max_turns(2);

        let err = agent.ask("go").await.unwrap_err();
        assert!(err.to_string().contains("turn limit"), "got: {err}");
    }

    #[tokio::test]
    async fn test_agent_with_fake_provider() {
        let fake = Arc::new(FakeAi::new());
        fake.add_response("Fake completion");
        let agent = Agent::new("fake-agent", fake);
        let task = agent.ask("Hi").await.unwrap();
        assert_eq!(task.completion(), "Fake completion");
        assert_eq!(task.status(), AgentTaskStatus::Completed);
        assert!(!task.id().is_empty());
        assert_eq!(task.result().completion(), "Fake completion");
    }

    #[test]
    fn test_agent_task_ids_unique() {
        let a = agent_task_id();
        let b = agent_task_id();
        assert_ne!(a, b);
    }
}
