#!/bin/bash
set -e

AGENT_NAME="${1:-lead}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AGENT_DIR="$SCRIPT_DIR/$AGENT_NAME"
AGENT_CONFIG="$AGENT_DIR/vs.config.json"
AGENT_MCP="$AGENT_DIR/.mcp.json"
AGENT_CLAUDE_MD="$AGENT_DIR/CLAUDE.md"
VSCODE_SETTINGS="$PROJECT_ROOT/.vscode/settings.json"
PROJECT_MCP="$PROJECT_ROOT/.mcp.json"
PROJECT_CLAUDE_MD="$PROJECT_ROOT/CLAUDE.md"
CLAUDE_AGENT_STATE="$PROJECT_ROOT/.claude-agent"

if [ ! -f "$AGENT_CONFIG" ]; then
    echo "Error: Agent config not found: $AGENT_CONFIG"
    echo "Available agents: lead, developer, reviewer"
    exit 1
fi

if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed"
    echo "Install with: brew install jq"
    exit 1
fi

mkdir -p "$PROJECT_ROOT/.vscode"

AGENT_ICON=$(jq -r '.agentIcon' "$AGENT_CONFIG")
VS_SETTINGS=$(jq '.vscodeSettings' "$AGENT_CONFIG")

if [ -f "$VSCODE_SETTINGS" ]; then
    cp "$VSCODE_SETTINGS" "$VSCODE_SETTINGS.backup"
    jq -s '.[0] * .[1]' "$VSCODE_SETTINGS" <(echo "$VS_SETTINGS") > "$VSCODE_SETTINGS.tmp"
    mv "$VSCODE_SETTINGS.tmp" "$VSCODE_SETTINGS"
else
    echo "$VS_SETTINGS" > "$VSCODE_SETTINGS"
fi

if [ -f "$AGENT_MCP" ]; then
    cp "$AGENT_MCP" "$PROJECT_MCP"
    echo "✓ Copied .mcp.json"
fi

if [ -f "$AGENT_CLAUDE_MD" ]; then
    cp "$AGENT_CLAUDE_MD" "$PROJECT_CLAUDE_MD"
    echo "✓ Copied CLAUDE.md"
fi

echo "$AGENT_NAME" > "$CLAUDE_AGENT_STATE"

echo ""
echo "========================================="
echo "$AGENT_ICON Applied $AGENT_NAME agent"
echo "========================================="
echo ""
echo "Next: Reload VSCode + Restart Claude Code"
echo ""
