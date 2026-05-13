# Contextro evaluation

**Installed:** 1.0.2  
**npm latest:** 1.0.2

## Verdict

Yes, it is useful. The core graph/search/history tools work well and are genuinely helpful for codebase discovery.

## What worked

| Area | Result |
|---|---|
| Bootstrap | `initialize` and `tools/list` worked |
| Indexing | `index(/Users/japneetkalkat/platform)` built the graph successfully |
| Discovery | `search`, `find_symbol`, `explain`, `find_callers`, `find_callees`, `impact` worked |
| Code graph | `code` with `get_document_symbols`, `search_symbols`, `lookup_symbols`, `list_symbols`, `pattern_search` worked |
| Repo/history | `commit_history`, `commit_search`, `dead_code`, `circular_dependencies` worked |
| Analysis | `analyze`, `audit`, `overview`, `architecture`, `focus` worked |
| Docs/export | `docs_bundle` and `sidecar_export` worked |
| Memory | `remember`, `recall`, `forget`, `compact`, `retrieve` worked |
| Knowledge | `knowledge` add/search/show/remove worked |
| Utilities | `status`, `repo_status`, `restore`, `session_snapshot`, `health`, `tags`, `skill_prompt` worked |

## What it returned

- `index`: 19,942 nodes, 21,080 relationships, 8,498 files, 19,942 symbols
- `health` after indexing: healthy, indexed true
- `audit`: quality score 75, with 698 highly connected symbols and 15 large files called out
- `docs_bundle`: generated `architecture.md` and `overview.md`
- `sidecar_export`: exported successfully, but produced 0 sidecars for this repo

## What did not work or needs care

| Item | Result |
|---|---|
| `repo_add` | Registers the repo, but does not load the graph |
| Fresh-session search after `repo_add` | Fails until `index()` runs in that session |
| `introspect("search code by meaning")` | Returned no tool suggestions for a vague query |
| npm from repo root | `npm view contextro version` hit an `EINVALIDTAGNAME` quirk in this workspace; running it from `/tmp` worked |

## Bottom line

Good MCP. Not perfect operationally, but the useful tools are real and reliable, and the indexing/search flow is strong once you know to run `index()` in the active session.
