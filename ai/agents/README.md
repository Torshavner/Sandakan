# Agent Configuration System (Sandakan Project)

Dynamic VSCode environment optimization via role-based configuration injection.

## Architecture

Each agent module contains:

* `CLAUDE.md`: Role definition and operational constraints.
* `vs.config.json`: VSCode UI/Theme overrides (Status bar, Title bar).

## Agent Inventory

| Agent | Icon | Role | Model | Color | Focus |
| --- | --- | --- | --- | --- | --- |
| **Lead** | 🎯 | PM/Architect | `claude-opus-4-6` | Cyan (#00CED1) | Planning, Trello/GitHub |
| **Developer** | 👨‍💻 | Implementation | `claude-sonnet-4-5-20250929` | Green (#28A745) | Coding, Testing, Fixes |
| **Reviewer** | 🔍 | Code Quality | `claude-opus-4-6` | Yellow (#FFC107) | PR Review, Security |

---

## Operations

### Deployment

Execute via Skill tool trigger or manual bash command:
`bash ai/agents/apply-agent-config.sh <agent-name>`

### Activation Workflow

1. Run: `bash ai/agents/apply-agent-config.sh <agent-name>`
2. Reload VSCode: `CMD+Shift+P` → `Developer: Reload Window`
3. Restart Claude Code session (for MCP configuration changes)

The script automatically:
- Updates VSCode theme (status bar, title bar)
- Copies agent-specific `.mcp.json` to project root
- Copies agent-specific `CLAUDE.md` to project root
- Saves agent state to `.claude-agent` file

### Status Line Metadata

Format: `Icon Role | Model | Context % | Session Cost`
Example: `🎯 Lead | Opus 4.6 | 15% | $0.042`

---

## System Integration

### File Mapping

* **Logic**: `ai/agents/apply-agent-config.sh`
* **Storage**: `ai/agents/{role}/`
* **Target**: `.vscode/settings.json` (Project Root)

### Technical Dependencies

* **JSON Processing**: `jq` (Mandatory: `brew install jq`)
* **Environment**: Unix-compatible shell (Bash).

### Troubleshooting Protocol

1. **Theme Fail**: Verify `✅ Applied` message -> Force Reload Window.
2. **Execution Fail**: Check `jq` installation (`which jq`) and script permissions.
3. **Status Fail**: Validate `~/.claude/statusline.sh` execution and JSON schema.