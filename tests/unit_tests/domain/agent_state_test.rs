use sandakan::domain::AgentState;

#[test]
fn given_thinking_state_when_comparing_equality_then_matches_same_variant() {
    assert_eq!(AgentState::Thinking, AgentState::Thinking);
    assert_ne!(AgentState::Thinking, AgentState::YieldingResponse);
}

#[test]
fn given_failed_state_when_extracting_reason_then_returns_inner_string() {
    let reason = "max iterations exceeded".to_string();
    let state = AgentState::Failed(reason.clone());

    assert!(matches!(state, AgentState::Failed(ref r) if r == &reason));
}

#[test]
fn given_awaiting_tool_execution_state_when_comparing_then_matches_own_variant() {
    let state = AgentState::AwaitingToolExecution;
    assert_eq!(state, AgentState::AwaitingToolExecution);
}

#[test]
fn given_yielding_response_state_when_cloning_then_produces_equal_value() {
    let state = AgentState::YieldingResponse;
    let cloned = state.clone();
    assert_eq!(state, cloned);
}
