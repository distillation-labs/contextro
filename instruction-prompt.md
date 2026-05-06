# Contextia MCP — Setup Prompt

Copy and paste the following prompt to your coding agent. It will install and configure everything end-to-end.

---

## The Prompt

```
Set up Contextia MCP on this machine end-to-end so it's ready to use. Follow these steps exactly:

---

STEP 1 — DETECT ENVIRONMENT

Check what's available:
- Run: docker --version
- Run: python3 --version (or python --version)
- Run: pip3 --version (or pip --version)
- Check if Claude Code is installed: claude --version
- Check OS: uname -s (or echo %OS% on Windows)

---

STEP 2 — INSTALL CONTEXTIA

Choose the right method based on what's available:

Option A — pip (preferred for local use):
  pip install contextia
  pip install model2vec
  pip install contextia[reranker]

  Verify: contextia --help

Option B — Docker (preferred for team/remote use):
  Pull the right image for this machine:
  - Apple Silicon (M1/M2/M3): docker pull jassskalkat/contextia-mcp:latest-arm64
  - Linux/Intel Mac/Windows:  docker pull jassskalkat/contextia-mcp:latest

  Then run it:
  docker run -d \
    --name contextia \
    -p 8000:8000 \
    -v contextia-data:/data \
    -v "$(pwd):/repos/codebase:ro" \
    -e CTX_STORAGE_DIR=/data/.contextia \
    -e CTX_CODEBASE_HOST_PATH="$(pwd)" \
    -e CTX_CODEBASE_MOUNT_PATH=/repos/codebase \
    -e CTX_PATH_PREFIX_MAP="${CTX_PATH_PREFIX_MAP:-}" \
    -e CTX_TRANSPORT=http \
    -e CTX_HTTP_HOST=0.0.0.0 \
    -e CTX_HTTP_PORT=8000 \
    -e CTX_AUTO_WARM_START=true \
    -e CTX_COMMIT_HISTORY_ENABLED=true \
    jassskalkat/contextia-mcp:latest

  Wait 5 seconds, then verify: curl http://localhost:8000/health

---

STEP 3 — CONNECT TO YOUR MCP CLIENT

Detect which MCP client is being used and configure it:

If Claude Code is installed (claude --version works):
  Run: claude mcp add contextia -- contextia
  (If using Docker instead: claude mcp add contextia -e CTX_TRANSPORT=http -- contextia --host localhost --port 8000)

If Claude Desktop is installed:
  Find the config file:
  - macOS: ~/Library/Application Support/Claude/claude_desktop_config.json
  - Windows: %APPDATA%\Claude\claude_desktop_config.json
  - Linux: ~/.config/Claude/claude_desktop_config.json

  Add this to the mcpServers section (create the file if it doesn't exist):
  {
    "mcpServers": {
      "contextia": {
        "command": "contextia"
      }
    }
  }

  If using Docker, add this instead:
  {
    "mcpServers": {
      "contextia": {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-http-proxy", "http://localhost:8000/mcp"]
      }
    }
  }

If Cursor is installed:
  Open Cursor Settings > MCP > Add Server
  Name: contextia
  Command: contextia
  (or URL: http://localhost:8000/mcp if using Docker)

If Windsurf is installed:
  Edit ~/.codeium/windsurf/mcp_config.json and add:
  {
    "mcpServers": {
      "contextia": {
        "command": "contextia",
        "transport": "stdio"
      }
    }
  }

If Kiro is installed:
  The MCP is already configured if you're running this prompt inside Kiro.
  Verify by running the status tool.

---

STEP 4 — INSTALL THE SKILL

The Contextia skill teaches your agent how to use the MCP tools correctly.

Find the skill directory:
  - Kiro:        ~/.kiro/skills/
  - Claude Code: ~/.claude/skills/  (or check: claude skills --list)
  - Claude.ai:   Upload via Settings > Capabilities > Skills

Install the skill:

  Method A — Clone from the repo:
    git clone https://github.com/jassskalkat/Contextia-MCP.git /tmp/contextia-mcp
    
    For Kiro:
      cp -r /tmp/contextia-mcp/.kiro/skills/dev-contextia-mcp ~/.kiro/skills/
    
    For Claude Code:
      cp -r /tmp/contextia-mcp/.kiro/skills/dev-contextia-mcp ~/.claude/skills/

  Method B — Create the skill manually:
    Create the directory: mkdir -p ~/.kiro/skills/dev-contextia-mcp
    
    Download the skill file:
    curl -o ~/.kiro/skills/dev-contextia-mcp/SKILL.md \
      https://raw.githubusercontent.com/jassskalkat/Contextia-MCP/main/.kiro/skills/dev-contextia-mcp/SKILL.md

  Verify the skill is installed:
    ls ~/.kiro/skills/dev-contextia-mcp/SKILL.md
    (or the equivalent path for your client)

---

STEP 5 — VERIFY EVERYTHING WORKS

Run these checks:

1. Check the server is running:
   - pip install: contextia --help (should show usage)
   - Docker: curl http://localhost:8000/health (should return {"status":"healthy"})

2. Test the MCP connection by calling the status tool:
   Tell the agent: "Call the status tool"
   Expected: {"version": "0.1.1", "indexed": false, "hint": "Run 'index' first."}

3. Index a project:
   Tell the agent: "Index this project at [current directory path]"
   Then: "Call status until indexed is true"

4. Run a test search:
   Tell the agent: "Search for 'main function' using the search tool"
   Expected: results with file paths and code snippets

If any step fails, report the exact error message and which step failed.

---

STEP 6 — DONE

Once status() returns indexed: true, Contextia is ready. Your agent now has:
- Semantic search across your entire codebase
- Call graph traversal (find_callers, find_callees, impact)
- Git history search (commit_search)
- Persistent memory across sessions (remember, recall)
- AST-based code search and rewrite (code tool)
- Session recovery after context compaction (session_snapshot)

The skill is installed and will automatically guide the agent to use the right tool for each task.
```

---

## Quick Reference

| Scenario | Use this |
|---|---|
| Local dev, single machine | pip install |
| Team sharing one index | Docker |
| Apple Silicon Mac | `latest-arm64` Docker tag |
| Intel Mac / Linux / Windows | `latest` Docker tag |

## Troubleshooting

**"contextia: command not found"** — Python's bin directory isn't in PATH. Try: `python3 -m contextia_mcp.server`

**Docker "port already in use"** — Change `-p 8000:8000` to `-p 8001:8000` and update the URL accordingly.

**"No codebase indexed"** — Normal on first run. Tell the agent to call `index("/path/to/your/project")`.

**Skill not triggering** — Make sure the SKILL.md file is in the correct skills directory for your client and the client has been restarted.
