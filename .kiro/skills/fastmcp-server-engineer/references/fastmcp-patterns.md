# FastMCP Patterns

Grounded in the official FastMCP server and tools docs.

## Strong FastMCP Practice

- typed function signatures drive schema generation
- docstrings and parameter metadata improve tool usability
- annotations communicate safety without spending prompt tokens
- `ToolResult` is for explicit output control, not mandatory everywhere
- `Context` is the proper path for logging, progress, resource access, and sampling
- middleware handles cross-cutting concerns cleanly
- server config should express validation, visibility, pagination, and masking decisions

## Weak FastMCP Practice

- untyped or overly dynamic signatures
- giant tool bodies duplicating logging, audit, or shaping logic
- no read-only annotations on safe tools
- strict validation by default without a reason
- forcing remote HTTP when local stdio is enough
