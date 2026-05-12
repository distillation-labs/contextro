# Security Policy

## Supported Versions

Security fixes are applied to the latest release only.

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report security issues privately via [GitHub's private vulnerability reporting](https://github.com/distillation-labs/contextro/security/advisories/new).

Include:

- A description of the vulnerability and its potential impact
- Steps to reproduce or a proof-of-concept
- Contextro version or commit SHA
- Any suggested mitigations

We will acknowledge your report within 5 business days and aim to resolve confirmed vulnerabilities within 30 days.

## Scope

Contextro is a local MCP server that indexes and searches code on your machine. It does not transmit your code externally. Relevant security concerns include:

- Path traversal or sandbox escapes in the indexing pipeline
- Denial-of-service via malicious input to the MCP tools
- Unsafe deserialization of indexed data
- Issues in the HTTP transport when exposed on a network interface
