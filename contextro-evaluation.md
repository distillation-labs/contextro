# Contextro evaluation

**Installed:** 1.1.3

## Verdict

Yes — this version is better.

The big improvement is that a fresh `repo_add` session now comes up indexed and searchable right away. The core discovery tools are still strong, and the operational flow is less confusing than before.

## What worked

| Area | Result |
|---|---|
| Bootstrap | `initialize` and `tools/list` worked |
| Indexing | `index(/Users/japneetkalkat/platform)` built the graph successfully |
| Fresh-session repo load | `repo_add` now returned `indexed: true` and search worked immediately |
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
- `index` reports a persistent index path under `~/.contextro/projects/...`
- `audit`: quality score 75, with 698 highly connected symbols and 15 large files called out
- `docs_bundle`: generated `architecture.md` and `overview.md`
- `sidecar_export`: succeeded, but still produced 0 sidecars for this repo
- `introspect("search code by meaning")`: now returns useful tool suggestions

## What still doesn’t work or is still rough

| Item | Result |
|---|---|
| `sidecar_export` | No sidecars were produced |
| `repo_add` mental model | It now works better, but the behavior is still easy to misunderstand |

## Bottom line

Better than the previous build. The main win is that repo registration now behaves like a real on-ramp instead of a dead end, so the tool is less awkward in daily use.
