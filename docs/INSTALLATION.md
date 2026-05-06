# Contextia Installation Guide

[![jassskalkat/Contextia MCP server](https://glama.ai/mcp/servers/jassskalkat/Contextia/badges/card.svg)](https://glama.ai/mcp/servers/jassskalkat/Contextia)
[![jassskalkat/Contextia MCP server](https://glama.ai/mcp/servers/jassskalkat/Contextia/badges/score.svg)](https://glama.ai/mcp/servers/jassskalkat/Contextia)

## Prerequisites

- **Python 3.10, 3.11, or 3.12** (Python 3.13+ is not yet supported by tree-sitter-languages)
- **pip** (comes with Python)
- **ripgrep (rg)** (recommended for 100% search coverage fallback)

Check your Python version:

```bash
python3 --version
```

If you have multiple Python versions, ensure you use 3.10 or later.

---

## Install from PyPI (recommended)

The simplest way to install Contextia:

```bash
pip install contextia
```

With optional extras:

```bash
# With GPU (CUDA) support
pip install contextia[gpu]

# With FlashRank reranker for better search quality
pip install contextia[reranker]

# Both GPU and reranker
pip install contextia[gpu,reranker]
```

After installing, the `contextia` command is available globally:

```bash
contextia
```

> **Virtual environment recommended:** While `pip install contextia` works globally, using a virtual environment avoids dependency conflicts:
> ```bash
> python3 -m venv ~/.contextia-venv
> source ~/.contextia-venv/bin/activate
> pip install contextia
> ```

---

## Install from Source (for development)

### Quick Start (Setup Script)

```bash
# Clone the repository
git clone https://github.com/jassskalkat/Contextia.git
cd Contextia

# Run setup script (creates venv, installs deps, verifies)
./setup.sh
```

**Setup script options:**

| Flag | Description |
|------|-------------|
| `--clean` | Remove existing venv before creating new |
| `--prod` | Install production dependencies only (no dev) |
| `--reranker` | Include optional FlashRank reranker |
| `--no-verify` | Skip verification step |
| `--help` | Show help message |

Examples:

```bash
./setup.sh                       # Dev install (pytest, ruff, mypy)
./setup.sh --clean               # Remove old venv, fresh install
./setup.sh --prod                # Production-only (no dev tools)
./setup.sh --reranker            # Dev install + FlashRank reranker
./setup.sh --clean --prod        # Clean production install
```

After setup, activate the environment:

```bash
source .venv/bin/activate
```

### Manual Install from Source

```bash
git clone https://github.com/jassskalkat/Contextia.git
cd Contextia

# Option 1: Production only
pip install -e .

# Option 2: With dev dependencies
pip install -e ".[dev]"

# Option 3: With dev + reranker
pip install -e ".[dev,reranker]"

# Option 4: With GPU (CUDA) support
pip install -e ".[gpu]"
```

---

## Verify Installation

```bash
# Check the module imports correctly
python3 -c "import contextia_mcp; print('OK')"

# Check the CLI is available
contextia --help

# Run the self-test demo (exercises all 15 tools)
python self_test/demo_mcp.py
```

---

## AI Tool Integrations

### Claude Code (CLI)

**If installed via pip (recommended):**

```bash
# If contextia is on your PATH (pip install contextia)
claude mcp add contextia -- contextia

# With environment variables
claude mcp add contextia -e CTX_EMBEDDING_MODEL=bge-small-en -- contextia
```

**If installed in a virtual environment:**

```bash
# Use the full venv path so the MCP client finds the right Python
claude mcp add contextia -- /path/to/Contextia/.venv/bin/contextia

# If updating an existing registration, remove first
claude mcp remove contextia
claude mcp add contextia -- /path/to/Contextia/.venv/bin/contextia
```

> **Why use the full venv path?** MCP clients spawn the server as a subprocess. If you just use `contextia`, it resolves to whatever Python is on your system PATH — which may not have the required dependencies installed. Using the venv path ensures the server runs with the correct, isolated environment.

**Manual config** — add to your settings (`~/.claude/settings.json`):

```json
{
  "mcpServers": {
    "contextia": {
      "command": "contextia"
    }
  }
}
```

**After setup**, reload your VS Code window (Cmd+Shift+P → "Reload Window") or restart Claude Code for the MCP server to start.

**Usage in Claude Code:**
```
> index my codebase at ./my-project
> search for authentication logic
> find_symbol User
> explain Config
```

---

### Claude Desktop

Add to your Claude Desktop configuration:

| Platform | Config File Location |
|----------|---------------------|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

```json
{
  "mcpServers": {
    "contextia": {
      "command": "contextia",
      "args": []
    }
  }
}
```

Restart Claude Desktop after saving.

---

### Cursor

Cursor supports MCP servers through its extension system:

1. **Open Settings** → Extensions → MCP
2. **Add Server Configuration**:

```json
{
  "contextia": {
    "command": "contextia",
    "transport": "stdio"
  }
}
```

Or add to `.cursor/mcp.json` in your project:

```json
{
  "servers": {
    "contextia": {
      "command": "contextia"
    }
  }
}
```

---

### Windsurf (Codeium)

Windsurf supports MCP through Cascade:

1. Open **Cascade Settings**
2. Navigate to **MCP Servers**
3. Add configuration:

```json
{
  "contextia": {
    "command": "contextia",
    "transport": "stdio"
  }
}
```

---

### Cline (VS Code)

Add to Cline's MCP settings in VS Code:

1. Open Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`)
2. Search "Cline: Open MCP Settings"
3. Add:

```json
{
  "mcpServers": {
    "contextia": {
      "command": "contextia"
    }
  }
}
```

---

### Zed Editor

Zed supports MCP through its assistant panel. Add to settings:

```json
{
  "assistant": {
    "mcp_servers": {
      "contextia": {
        "command": "contextia"
      }
    }
  }
}
```

---

### Continue (VS Code / JetBrains)

Add to your Continue configuration (`~/.continue/config.json`):

```json
{
  "mcpServers": [
    {
      "name": "contextia",
      "command": "contextia"
    }
  ]
}
```

---

### Generic MCP Client

For any MCP-compatible client, use stdio transport:

```bash
# Command to run
contextia

# Transport
stdio (stdin/stdout)

# Protocol
Model Context Protocol (MCP)
```

---

## Configuration

All settings use the `CTX_` environment variable prefix:

| Variable | Default | Description |
|----------|---------|-------------|
| `CTX_STORAGE_DIR` | `.contextia` | Storage directory for indexes and graph DB |
| `CTX_EMBEDDING_MODEL` | `jina-code` | Embedding model: `jina-code`, `bge-small-en` |
| `CTX_EMBEDDING_DEVICE` | `auto` | Device: `auto` (CUDA > MPS > CPU), `cuda`, `mps`, `cpu` |
| `CTX_MAX_FILE_SIZE_MB` | `10` | Skip files larger than this |
| `CTX_CHUNK_MAX_CHARS` | `4000` | Max characters per code chunk |
| `CTX_MAX_MEMORY_MB` | `350` | Memory budget in MB |
| `CTX_SEARCH_MODE` | `hybrid` | Search mode: `hybrid`, `vector`, or `bm25` |
| `CTX_LOG_LEVEL` | `INFO` | Logging level |
| `CTX_LOG_FORMAT` | `text` | Log format: `text` or `json` |
| `CTX_PERMISSION_LEVEL` | `full` | Permission level: `full` or `read` |
| `CTX_AUDIT_ENABLED` | `true` | Enable audit logging |
| `CTX_RATE_LIMIT_ENABLED` | `false` | Enable per-tool rate limiting |
| `CTX_TRUST_REMOTE_CODE` | `false` | Allow trust_remote_code in models |

Example:

```bash
CTX_LOG_LEVEL=DEBUG CTX_SEARCH_MODE=vector contextia
```

---

## Running Tests

```bash
# All tests (441)
pytest -v

# Skip slow performance benchmarks
pytest -m "not slow"

# Lint
ruff check .
```

---

## Troubleshooting

### ONNX Runtime / Optimum errors during indexing

If you see `Using the ONNX backend requires installing Optimum and ONNX Runtime`, install the required packages:

```bash
pip install "sentence-transformers[onnx]" "optimum[onnxruntime]>=1.19.0,<2.0"
```

**Version compatibility:** Ensure `optimum` and `transformers` versions are compatible. If you see `cannot import name 'FLAX_WEIGHTS_NAME'`, pin compatible versions:

```bash
pip install "optimum[onnxruntime]>=1.19.0,<2.0" "transformers>=4.46,<5.0"
```

**MCP server not picking up new packages:** If you installed packages but the MCP server still errors, the server process needs a restart. Reload your VS Code window, restart Claude Code, or restart Claude Desktop.

**Alternative: use a model that doesn't need ONNX:**

```bash
CTX_EMBEDDING_MODEL=bge-small-en contextia
```

### `ModuleNotFoundError: No module named 'contextia_mcp'`

Ensure you installed with `pip install -e .` from the project root and are using the correct Python version (3.10+). If using a venv, make sure it's activated: `source .venv/bin/activate`.

### `tree-sitter` FutureWarning

The warning `Language(path, name) is deprecated` is harmless and comes from the tree-sitter-languages compatibility layer. It does not affect functionality.

### High memory usage during indexing

The embedding model is loaded during indexing and unloaded after. Peak RSS may exceed the 350MB target briefly. Set `CTX_MAX_MEMORY_MB` to adjust the budget.

### `pip` resolves dependency conflicts

If you see dependency conflict warnings from other installed packages, these are unrelated to Contextia and can be safely ignored as long as `import contextia_mcp` succeeds.

### Demo fails at indexing step

Ensure `tree-sitter==0.21.3` and `tree-sitter-languages>=1.10.0` are installed. These are pinned for compatibility.

### Server not found after install

If `contextia` command is not found, ensure the install location is on your PATH. With a venv, activate it first. Without a venv, you may need `python3 -m contextia_mcp.server` as a fallback.

### MCP client can't connect

- Ensure `contextia` is on the PATH that the MCP client uses
- If installed in a venv, use the full path: `/path/to/Contextia/.venv/bin/contextia`
- Check the client's logs for connection errors
