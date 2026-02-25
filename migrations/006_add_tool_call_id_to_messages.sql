-- Add nullable tool_call_id column to messages table.
-- Used by ToolResponse messages to link back to the originating tool call.
-- NULL for all existing User/Assistant/System messages (backward compatible).
ALTER TABLE messages ADD COLUMN IF NOT EXISTS tool_call_id TEXT;
