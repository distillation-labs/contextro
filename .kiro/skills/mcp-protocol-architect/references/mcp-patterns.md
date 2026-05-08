# MCP Protocol Patterns

Grounded in the official MCP architecture and server-concepts docs.

## Key Rules

- Tools are model-controlled executable functions.
- Resources are application-controlled passive context sources.
- Prompts are reusable templates for user-invoked workflows.
- Capability negotiation is part of the protocol, not optional ceremony.
- Notifications matter when the server surface changes dynamically.
- Transport is an architectural decision: stdio for local process communication, HTTP for remote/shared service.

## Good MCP Design

- keeps primitives narrow and typed
- makes discovery explicit through list operations
- uses resources for browseable or fetchable context
- uses tools for actions or derived computation
- uses prompts to package repeated workflows, not to hide APIs

## Bad MCP Design

- exposing static documents as tools
- collapsing a browseable context surface into giant tool results
- using prompts instead of fixing the API surface
- ignoring notifications when lists can change at runtime
