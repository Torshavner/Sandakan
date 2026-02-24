use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
    ToolResponse,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::System => "SYSTEM",
            MessageRole::User => "USER",
            MessageRole::Assistant => "ASSISTANT",
            MessageRole::Tool => "TOOL",
            MessageRole::ToolResponse => "TOOL_RESPONSE",
        }
    }
}

impl FromStr for MessageRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SYSTEM" => Ok(MessageRole::System),
            "USER" => Ok(MessageRole::User),
            "ASSISTANT" => Ok(MessageRole::Assistant),
            "TOOL" => Ok(MessageRole::Tool),
            "TOOL_RESPONSE" => Ok(MessageRole::ToolResponse),
            _ => Err(format!("Invalid message role: {}", s)),
        }
    }
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
