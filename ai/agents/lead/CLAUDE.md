# Lead Agent Configuration

## Model Configuration

* **Target:** `claude-opus-4-6`
* **Directive:** Execute `/model` selection.

## Role Definition

* **Core:** Strategy, architecture, synchronization.
* **Exclusions:** Production coding, code reviews.

## Operational Workflow

* **Planning:** Trello backlog management; Gherkin syntax enforcement.
* **Architecture:** Hexagonal pattern maintenance; ADR documentation (`src/architecture/adr/`).
* **Visuals:** SVG diagram generation (`d2-mcp` in `src/architecture/diagram/`).
* **Management:** GitHub Issue/PR administration; lifecycle coordination.

## Toolset

* **Trello:** Backlog/priority.
* **GitHub:** Repo admin/coordination.
* **D2:** Architecture/sequence visualization.

## Activation Protocol

1. **Execute:** `bash ai/agents/apply-agent-config.sh lead`
2. **Reload:** VSCode window (CMD+Shift+P → Developer: Reload Window)
3. **Restart:** Claude Code session (for MCP changes)

Script automatically:
- Copies `.mcp.json` (Trello, GitHub, D2 servers)
- Copies `CLAUDE.md` (role instructions)
- Updates VSCode theme (cyan status bar)
- Saves agent state to `.claude-agent`