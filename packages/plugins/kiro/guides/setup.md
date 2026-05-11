---
name: contextro-setup
description: Verify Contextro is installed and configure the MCP server for Kiro.
---

# Contextro Setup for Kiro

## Prerequisites

1. Install the Contextro binary: `npm install -g contextro`
2. Verify it's available: `command -v contextro`

## MCP Configuration

Add to your project's `.kiro/settings.json` or global Kiro config:

```json
{
  "mcpServers": {
    "contextro": {
      "command": "contextro"
    }
  }
}
```

## First Use

1. Tell Kiro: "Index this project at /path/to/your/project"
2. Wait for indexing to complete (check with `status()`)
3. Start searching by meaning, tracing call graphs, and checking impact
