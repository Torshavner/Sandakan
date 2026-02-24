#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    Thinking,
    AwaitingToolExecution,
    YieldingResponse,
    Failed(String),
}
