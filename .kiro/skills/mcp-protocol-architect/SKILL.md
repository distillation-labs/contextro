---
name: mcp-protocol-architect
description: >
  Use for designing or refactoring MCP servers around correct protocol primitives,
  transport choices, lifecycle, capability negotiation, and tool/resource/prompt boundaries.
  Trigger when the user asks how an MCP feature should be modeled, when to use a tool vs
  resource vs prompt, how to structure a server or protocol surface, how to support clients
  cleanly, or how to improve MCP architecture without cargo-culting one framework. Do not use
  for ordinary Python refactors or low-level performance work.
when_to_use: >
  Especially useful for server surface design, protocol correctness, capability modeling,
  transport decisions, notifications, and long-term MCP API shape.
metadata:
  version: "1.0.0"
  category: mcp-development
  tags: [mcp, protocol, tools, resources, prompts, transport, architecture]
license: MIT
---

# MCP Protocol Architect

Design MCP features around protocol semantics first, implementation details second.

## Core Rule

Choose the correct primitive before writing code.

- `Tools`: model-invoked actions with typed inputs and side effects or active computation
- `Resources`: passive context the application reads and decides how to inject
- `Prompts`: reusable interaction templates that users invoke intentionally

If the feature is mis-modeled at the primitive layer, implementation quality downstream will not save it.

## Design Sequence

1. Name the user-facing workflow.
2. Decide which primitive owns each part.
3. Define lifecycle and capability implications.
4. Define transport constraints.
5. Define discovery and change-notification behavior.
6. Only then define server implementation details.

## Protocol Guidance

### Primitive Selection

- Use a tool when the model should decide to invoke a typed operation.
- Use a resource when the application should fetch or browse context.
- Use a prompt when the user needs a reusable structured workflow.
- Do not overload tools to serve static context when resources fit better.
- Do not hide critical workflow behavior in prompts when the real need is a tool or resource.

### Lifecycle And Capabilities

- Respect initialization and capability negotiation.
- Model dynamic behavior explicitly with list-changed notifications where appropriate.
- Prefer stable, discoverable primitives over hidden assumptions in instructions.

### Transport

- Use stdio for local single-client workflows and tight local integrations.
- Use streamable HTTP when remote access, shared serving, or standard auth are required.
- Do not choose a transport for convenience if it conflicts with security or deployment reality.

## Output Expectations

Produce:

1. primitive mapping
2. lifecycle/capability notes
3. transport recommendation
4. discovery and notification plan
5. implementation implications
6. tradeoffs and rejected alternatives

## Anti-Patterns

- Treating everything as a tool
- Using prompts to paper over bad primitive design
- Ignoring capability negotiation
- Designing remote-first when the repo is local-first
- Confusing client-driven resources with model-driven tools

## References

- `references/mcp-patterns.md`
- `references/eval-rubric.md`
