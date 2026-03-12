-- Stores the full agentic ReAct trace (tool calls, iterations, reflection)
-- as a JSONB blob so the EvalWorker judge can evaluate multi-step reasoning.
ALTER TABLE eval_events
    ADD COLUMN IF NOT EXISTS agentic_trace JSONB DEFAULT NULL;
