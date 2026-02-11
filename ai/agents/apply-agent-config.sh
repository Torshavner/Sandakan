#!/bin/bash
# Apply agent-specific VSCode configuration
# Usage: ./apply-agent-config.sh <agent-name>

set -e

AGENT_NAME="${1:-lead}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AGENT_CONFIG="$SCRIPT_DIR/$AGENT_NAME/vs.config.json"
VSCODE_SETTINGS="$PROJECT_ROOT/.vscode/settings.json"

if [ ! -f "$AGENT_CONFIG" ]; then
    echo "Error: Agent config not found: $AGENT_CONFIG"
    echo "Available agents: lead, developer, reviewer"
    exit 1
fi

mkdir -p "$PROJECT_ROOT/.vscode"

if command -v jq &> /dev/null; then
    AGENT_ICON=$(jq -r '.agentIcon' "$AGENT_CONFIG")
    VS_SETTINGS=$(jq '.vscodeSettings' "$AGENT_CONFIG")

    if [ -f "$VSCODE_SETTINGS" ]; then
        cp "$VSCODE_SETTINGS" "$VSCODE_SETTINGS.backup"

        jq -s '.[0] * .[1]' "$VSCODE_SETTINGS" <(echo "$VS_SETTINGS") > "$VSCODE_SETTINGS.tmp"
        mv "$VSCODE_SETTINGS.tmp" "$VSCODE_SETTINGS"
    else
        echo "$VS_SETTINGS" > "$VSCODE_SETTINGS"
    fi

    echo "Applied $AGENT_ICON $AGENT_NAME agent configuration"
else
    echo "Error: jq is required but not installed"
    echo "Install with: brew install jq"
    exit 1
fi
