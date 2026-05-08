#!/usr/bin/env bash
# Usage: ./scripts/test_tool.sh <tool_name> [json_args]
# Example: ./scripts/test_tool.sh health '{}'
set -euo pipefail

TOOL="${1:-health}"
ARGS="${2:-{}}"
HOST="${MCP_HOST:-http://localhost:8000}"

SESSION=$(curl -s -X POST "$HOST/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' \
  | grep '^data:' | head -1 | sed 's/^data: //' \
  | python3 -c "import sys,json; print(json.load(sys.stdin))" 2>/dev/null || true)

SESSION_ID=$(curl -s -X POST "$HOST/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' \
  -D /tmp/mcp_headers.txt > /dev/null && grep -i 'mcp-session-id' /tmp/mcp_headers.txt | awk '{print $2}' | tr -d '\r')

echo "Session: $SESSION_ID"

curl -s -X POST "$HOST/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "mcp-session-id: $SESSION_ID" \
  -d "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"$TOOL\",\"arguments\":$ARGS}}" \
  | grep '^data:' | sed 's/^data: //' | python3 -m json.tool
