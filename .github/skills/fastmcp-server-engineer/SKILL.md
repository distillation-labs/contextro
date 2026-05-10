---
name: fastmcp-server-engineer
description: >
  Use for building or refactoring FastMCP servers, tools, resources, prompts, middleware,
  validation, annotations, ToolResult outputs, and server behavior. Trigger when the user asks
  how to implement an MCP feature in FastMCP, how to structure tool signatures, when to use
  strict validation, how to return structured output, how to add middleware, visibility, timeouts,
  or context access, or how to make a FastMCP server production-grade. Do not use for pure MCP
  protocol design with no FastMCP implementation intent.
when_to_use: >
  Especially useful for FastMCP tool design, annotations, output schemas, server configuration,
  middleware, context usage, HTTP or stdio deployment choices, and list-changed behavior.
metadata:
  version: "1.0.0"
  category: mcp-development
  tags: [fastmcp, server, tools, validation, annotations, toolresult, middleware]
license: Proprietary
---

# FastMCP Server Engineer

Implement FastMCP servers in the framework's grain, not against it.

## Core Rules

- Keep the server entrypoint small; move logic into normal modules.
- Use typed Python function signatures so FastMCP can generate correct schemas.
- Use the right component: tool, resource, or prompt.
- Prefer explicit annotations like `readOnlyHint` when they are true.
- Return structured outputs intentionally with `ToolResult` when you need control.

## FastMCP Guidance

### Tool Design

- Use ordinary typed function parameters; avoid `*args` and `**kwargs`.
- Write strong docstrings because they become tool descriptions.
- Use `Annotated` and `Field` metadata for parameter descriptions and validation.
- Keep tool schemas simple for LLM clients.
- If a tool is read-only, mark `annotations={"readOnlyHint": True}`.

### Validation

- Prefer flexible validation unless strict schema conformance is a real requirement.
- Use strict validation only when coercion would be risky or ambiguous.
- Avoid leaking complexity into the tool signature when a simpler schema will do.

### Outputs

- Return plain values for simple cases.
- Use `ToolResult` when you need explicit `content`, `structured_content`, or `meta`.
- Shape outputs for both humans and clients.
- Do not return giant raw payloads when compact summaries plus retrieval patterns are better.

### Context And Middleware

- Use `Context` for logging, progress, resource access, and client-side sampling.
- Put cross-cutting concerns in middleware or shared helpers, not repeated inside every tool.
- Use server configuration for visibility, masking, pagination, and tasks instead of ad hoc logic.

### Deployment And Behavior

- Use stdio by default for local MCP integrations.
- Add HTTP only when remote serving is a real requirement.
- Use list-changed notifications or provider patterns when the surface is dynamic.
- Prefer server-level structure over magic in one large `server.py` file.

## Output Format

Return:

1. FastMCP component choice
2. function signature guidance
3. validation choice
4. output strategy
5. middleware/context needs
6. deployment/runtime considerations

## Anti-Patterns

- giant untyped tool signatures
- missing annotations on read-only tools
- framework-agnostic advice when the codebase is already on FastMCP
- putting every policy directly in tool bodies
- using `ToolResult` everywhere when simple returns are clearer

## References

- `references/fastmcp-patterns.md`
- `references/eval-rubric.md`
