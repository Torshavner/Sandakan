-- Add nullable tool_name column to messages table.
-- Used by Tool (assistant tool-call intent) and ToolResponse messages to carry
-- the name of the tool being called or responded to.
-- NULL for all existing User/Assistant/System messages (backward compatible).
ALTER TABLE messages ADD COLUMN IF NOT EXISTS tool_name TEXT;
