//! # Message Types
//!
//! Standard message format used across all providers.

use serde::{Deserialize, Serialize};

/// Role in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    /// Tool call ID (for tool responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Name (for tool calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a tool response message
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_message() {
        let msg = Message::system("You are a helpful assistant");
        assert_eq!(msg.role, MessageRole::System);
        assert_eq!(msg.content, "You are a helpful assistant");
        assert!(msg.tool_call_id.is_none());
        assert!(msg.name.is_none());
    }

    #[test]
    fn test_user_message() {
        let msg = Message::user("Hello!");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello!");
    }

    #[test]
    fn test_assistant_message() {
        let msg = Message::assistant("I can help with that.");
        assert_eq!(msg.role, MessageRole::Assistant);
    }

    #[test]
    fn test_tool_message() {
        let msg = Message::tool("result data", "call_123");
        assert_eq!(msg.role, MessageRole::Tool);
        assert_eq!(msg.content, "result data");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"test\""));
        // tool_call_id should be skipped when None
        assert!(!json.contains("tool_call_id"));
    }

    #[test]
    fn test_message_deserialization() {
        let json = r#"{"role":"assistant","content":"Hello there"}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(msg.content, "Hello there");
    }

    #[test]
    fn test_role_ordering() {
        // Just ensure all roles can be created
        let roles = vec![
            MessageRole::System,
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::Tool,
        ];
        assert_eq!(roles.len(), 4);
    }

    #[test]
    fn test_message_with_string_types() {
        // Test that Into<String> works with different types
        let msg1 = Message::system("literal str");
        let msg2 = Message::user(String::from("String"));
        assert_eq!(msg1.content, "literal str");
        assert_eq!(msg2.content, "String");
    }

    #[test]
    fn test_conversation_flow() {
        let messages = vec![
            Message::system("You are a coding assistant"),
            Message::user("Write a hello world in Python"),
            Message::assistant("print('Hello, World!')"),
            Message::user("Now in Rust"),
            Message::assistant("fn main() { println!(\"Hello, World!\"); }"),
        ];
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[4].role, MessageRole::Assistant);
    }
}
