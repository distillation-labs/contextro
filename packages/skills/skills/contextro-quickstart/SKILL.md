---
name: contextro-quickstart
description: >
  Minimal Contextro setup. Teaches your agent to index a codebase and use
  search, find_symbol, explain, and impact before reading files directly.
when_to_use: >
  Use when the agent needs to discover code in an unfamiliar or large codebase.
  Prefer Contextro search over reading multiple files.
metadata:
  version: "0.1.0"
  mcp-server: contextro
  category: mcp-enhancement
  tags: [contextro, mcp, quickstart]
license: MIT
---

# Contextro Quickstart

Use Contextro MCP for code discovery instead of reading files one by one.

## Setup

```text
1. index("/path/to/project")
2. status()  — wait for indexed=true
```

## Core Tools

| Need | Tool | Example |
|------|------|---------|
| Find code by meaning | `search("query")` | `search("authentication flow")` |
| Find a symbol | `find_symbol("Name")` | `find_symbol("UserService")` |
| Understand a symbol | `explain("Name")` | `explain("TokenBudget")` |
| Check what breaks | `impact("Name")` | `impact("authenticate")` |
| Find callers | `find_callers("Name")` | `find_callers("validate")` |
| Search git history | `commit_search("query")` | `commit_search("auth fix")` |

## Rules

- Always `impact()` before renaming or deleting a function.
- Prefer `search()` over reading 5+ files to find something.
- Use `find_symbol()` when you know the exact name.
- Use `explain()` before editing unfamiliar code.
