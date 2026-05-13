# Contextro evaluation

**Installed:** 1.1.2

## Verdict

Still good, and a bit better than before. The core discovery tools remain strong, and the latest build now reports a persistent index path.

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

## Notable results

- `index`: 19,942 nodes, 21,080 relationships, 8,498 files, 19,942 symbols
- `index` now reports a persistent index path under `~/.contextro/projects/...`
- `audit`: quality score 75, with 698 highly connected symbols and 15 large files called out
- `docs_bundle`: generated `architecture.md` and `overview.md`
- `sidecar_export`: succeeded, but produced 0 sidecars for this repo

## What still doesn’t work

| Item | Result |
|---|---|
| `repo_add` | Registers the repo, but does not load the graph |
| Fresh-session search after `repo_add` | Fails until `index()` runs in that session |
| `introspect("search code by meaning")` | Returned no tool suggestions for a vague query |

## Bottom line

Good MCP. The useful tools are real and reliable, but you still need to know the session rules: `repo_add` is not enough, and `index()` is the step that actually makes search work.
