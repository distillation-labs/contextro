# Tool Regression Matrix

Exercise every public tool before a release claim.

## Core lifecycle

| Tool | Minimum assertion |
|---|---|
| `status` | Works before indexing and reports indexed state truthfully |
| `health` | Works before indexing and reports healthy status |
| `index` | Indexes a real repo, errors on invalid paths, and can be rerun without corrupting state |

## Search and code intelligence

| Tool | Minimum assertion |
|---|---|
| `search` | Returns relevant ranked results without flattened or misleading scores |
| `find_symbol` | Finds exact and fuzzy symbols and gives useful misses |
| `find_callers` | Returns real caller sets for a known function |
| `find_callees` | Returns real callee sets for a known function |
| `explain` | Includes honest summary plus full caller/callee counts |
| `impact` | Shows transitive impact for a shared symbol |
| `refactor_check` | Combines risk, caller/callee, and impact output coherently |
| `code` | Cover `get_document_symbols`, `list_symbols`, `lookup_symbols`, `search_codebase_map`, `pattern_search`, `pattern_rewrite` dry run, and `edit_plan` |

## Analysis and reporting

| Tool | Minimum assertion |
|---|---|
| `overview` | Returns a real project summary, not bare counters |
| `architecture` | Produces usable module or hub overview on a real repo |
| `analyze` | Errors cleanly on bad paths and reports meaningful metrics on valid ones |
| `focus` | Errors cleanly on bad paths and returns useful low-token context on valid ones |
| `dead_code` | Avoids obvious fixture or test-framework false positives |
| `circular_dependencies` | Returns sane results for a repo that actually has import structure |
| `test_coverage_map` | Is clearly labeled as static heuristic coverage |
| `audit` | Produces a bundled report without crashing |
| `docs_bundle` | Generates docs output for a real repo |
| `sidecar_export` | Writes sidecar artifacts without path confusion |

## Git and repo registry

| Tool | Minimum assertion |
|---|---|
| `commit_search` | Finds a known recent change in git history |
| `commit_history` | Returns recent commits without path confusion |
| `repo_add` | Adds and indexes another repo successfully |
| `repo_remove` | Removes a repo cleanly |
| `repo_status` | Reflects the registered repos and survives restart |

## Memory, knowledge, and session recovery

| Tool | Minimum assertion |
|---|---|
| `remember` | Stores a note that can be retrieved later |
| `recall` | Finds the stored note with a related query |
| `forget` | Deletes the stored note or matching tag set |
| `tags` | Lists current memory tags |
| `knowledge` | Cover add and search on a small docs input |
| `compact` | Creates retrievable archive content |
| `retrieve` | Retrieves archived or sandboxed content after restart |
| `session_snapshot` | Returns recent events with timestamps and tool arguments |
| `restore` | Produces a project re-entry summary |

## Agent support and introspection

| Tool | Minimum assertion |
|---|---|
| `skill_prompt` | Reflects the current preferred parameters and workflows |
| `introspect` | Finds the right tool for a natural-language query |

## Parameter compatibility checks

Recheck these before release:

- symbol-taking tools accept `symbol_name`
- legacy aliases such as `name` or `symbol` do not regress where supported
- file-taking tools accept `path`
- legacy `file_path` callers do not regress where supported

## Minimum release claim

Do not say "all tools work" unless every row above has been exercised on at least one
real repo or the relevant synthetic input.
