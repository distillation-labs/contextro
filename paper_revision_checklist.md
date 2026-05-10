# Contextro Paper – Revision Checklist

## 1. Missing Methodological Details

- [ ] **Agent specification** – State which LLM agent was used (e.g., Claude 3.5 Sonnet, GPT-4o, custom ReAct agent). Include version, temperature, max tokens, and system prompt.
- [ ] **Exact task prompts** – Provide the natural language query for each of the 60 tasks (in an appendix or public gist). Without these, the "discovery workflow" cannot be replicated.
- [ ] **Control arm workflow** – Explain exactly how the control agent decided which files to grep and which to read. Was it a hard‑coded script, an LLM‑driven planner, or something else? Show the decision logic or pseudocode.
- [ ] **Token estimator** – Replace `len(output)//4` with a real tokenizer. Specify which model's tokenizer is used (e.g., `tiktoken` for GPT-4, Anthropic's tokenizer for Claude). Report token counts for both arms using the same tokenizer.
- [ ] **Latency measurement** – Specify how wall‑clock time is measured (e.g., `time.perf_counter()`, excluding indexing time). Report percentiles (p50, p90, p99) not just medians.
- [ ] **Determinism** – Describe how you ensured the same ordering of tasks and the same initial state across arms. Provide seed values and any randomisation control.

## 2. Weak Baseline (Control Arm)

- [ ] **Add stronger baseline(s)** – The current control arm (grep + up to 5 file reads) is unrealistically weak. Include at least one of:
  - `git grep` + `ast-grep` + `rg` (pattern matching)
  - A simple RAG agent using chunk‑based retrieval with the same embedding model
  - LSP (Language Server Protocol) symbol lookup (free, local)
- [ ] **Justification** – If you keep the weak baseline, explicitly state that it represents a *naive* agent without any code intelligence, not a practical baseline. Then discuss how the gap would shrink with a smarter baseline.

## 3. Missing Evaluation Dimensions

- [ ] **End‑to‑end correctness** – Add a small study: after discovery, let the agent perform a code change (e.g., fix a bug, add a feature). Measure whether the final change is correct and how many attempts it takes. Even 5‑10 tasks would strengthen the claim.
- [ ] **Human comparison (optional)** – Time a human engineer to perform the same discovery tasks using IDE tools (e.g., "Find All References", "Go to Definition"). Show that Contextro is competitive with or faster than a human.
- [ ] **Cost in dollars** – Convert token savings to approximate API cost (e.g., $0.01 per 1K tokens). This makes the result tangible for practitioners.

## 4. Statistical Rigour

- [ ] **Confidence intervals** – For the 39‑task subset, report mean and 95% CI for token reduction (bootstrapped). You already have bootstrap subsets – just compute the CIs.
- [ ] **Effect size** – Report Cohen's d or a similar standardised effect size for latency and token reduction.
- [ ] **Multiple comparison correction** – Table 5 tests 8 categories. State whether you applied any correction (e.g., Bonferroni) or argue why it is not needed.

## 5. Presentation & Formatting

- [ ] **Fix broken pages** – Pages 12, 19, 20 contain repeated numbers ("1 1 1", "0.0.0.0"). Correct the rendering.
- [ ] **Replace figure placeholders** – `<center>Figure X...</center>` must be replaced with actual diagrams (system overview, study design, results graphs). Use vector graphics (PDF/SVG).
- [ ] **Fix table references** – Ensure every table is referenced in the text. Table 7 is referenced on page 6 but appears on page 15; check all cross‑references.
- [ ] **Correct typos** –
  - "calllee" → "callee" (Table 6)
  - "Contextos" → "Contextro" (page 17, line "Contextos shows")
  - "mcp native" → "MCP‑native" (consistent hyphenation)
- [ ] **Clarify outlier latency** – In Table 6, the "code tool (comparable slice)" row shows median 1.73 ms but max 10,936 ms. Clarify whether that max is an outlier and if it should be excluded.

## 6. Reproducibility (Even with Private Repo)

- [ ] **Create a public proxy repo** – Build a small, public version of the monorepo (e.g., a stripped‑down Next.js + React Native + Convex example with similar structure). Run the 39‑task subset on that public repo and report results. This allows third‑party verification.
- [ ] **Provide a containerised environment** – Include a `Dockerfile` or `devcontainer.json` that sets up the exact Python version (3.12.13), dependencies, and benchmark scripts. Include a `README.md` with step‑by‑step commands.
- [ ] **Publish task list as JSON** – Make the exact 60‑task descriptions available in a machine‑readable format (e.g., `tasks.json`) so others can adapt them to their own codebases.
- [ ] **Set up a CI/CD benchmark** – Create a GitHub Action that runs the open‑source subset on every commit and posts the results. Mention this in the paper as a living reproducibility badge.

## 8. Missing Discussion Points

*(Section numbers continue from original list; #7 is excluded per your request.)*

- [ ] **Why not use an existing code graph** – Discuss why you built your own graph (rustworkx) instead of using `ast‑grep`'s query engine or `tree‑sitter`'s built‑in traversal.
- [ ] **Scalability limits** – The paper tests 8,496 files. What happens at 100k files? 1M files? Provide rough estimates or a note that indexing time scales linearly but graph memory may become an issue.
- [ ] **Security / privacy** – Since Contextro is local‑first, explicitly mention that no code leaves the machine. This is a selling point over cloud‑based tools.

## 9. Missing Conclusion & Future Work

- [ ] **Actionable future work** – Instead of generic "improve disambiguation", list specific next steps:
  - Support for more languages (Python, Go, Rust)
  - Integration with CI/CD for "impact analysis" on pull requests
  - Learning from agent feedback to re‑rank search results
- [ ] **Broader impact statement** – Add one sentence about how reducing token waste helps with energy consumption, latency, or developer frustration.

---

### Summary Quick‑Check

| Section | Status |
|---------|--------|
| 1. Methodological details | ☐ Complete |
| 2. Weak baseline | ☐ Complete |
| 3. Evaluation dimensions | ☐ Complete |
| 4. Statistical rigour | ☐ Complete |
| 5. Presentation & formatting | ☐ Complete |
| 6. Reproducibility | ☐ Complete |
| 8. Discussion points | ☐ Complete |
| 9. Conclusion & future work | ☐ Complete |

Use this checklist to redo the paper. When all boxes are ticked, the manuscript will be substantially stronger. Let me know if you want me to draft any specific section (e.g., tokenisation fix, stronger baseline description, or a public proxy repo structure).