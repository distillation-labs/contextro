# Contextro Paper – Honest Evaluation for Coding Agent (Closed‑Source Version)

## Summary

The paper presents **Contextro**, a local MCP server that replaces file‑centric discovery with structured code intelligence (semantic search, call graphs, AST queries, git history, persistent memory). A paired study on an 8,496‑file private TypeScript monorepo (60 tasks, 39 directly comparable) shows dramatic token reduction (95.6% full set, 98.0% comparable), latency improvement (median 448 ms → 5.2 ms), and zero file reads in the treatment arm. Bootstrap subsets confirm robustness.

The system is **not open source**. The paper must therefore remove the MIT license claim and instead state clearly what is and is not shared. The private repository cannot be redistributed, so reproducibility and credibility rely on public proxy benchmarks and transparent methodology.

## What is already good

- Tokenizer fixed (tiktoken, cl100k_base).  
- Task catalogs as JSON (60 tasks, 39 comparable).  
- Outlier latency reported (7 s, 11 s).  
- Bootstrap robustness (97.24‑98.48%).  
- Clean separation of full vs. comparable vs. MCP‑native tasks.  
- Public benchmarks on a small `src/` tree (MRR 1.0).  

## What must be fixed (actionable checklist)

### 🔴 Critical (fix immediately)

#### 1. PDF formatting is broken
- **Pages 12, 20, 21** are garbage (repeated numbers, line noise).  
- **Raw results table (Table 9)** is unreadable.  
- **Figure placeholders** (`<center>Figure X...</center>`) – no actual diagrams.  
- **Table 6** appears twice.  

**Action:**  
- Fix PDF export. Remove garbage pages.  
- Ensure Table 9 is readable (e.g., as a proper Markdown/LaTeX table).  
- Replace `<center>Figure X>` with real diagrams (even hand‑drawn is fine).  
- Ensure each table appears only once.

#### 2. License is wrong – remove MIT
- The paper says “Contextro software is released under the MIT License.” This is **false** for a closed‑source project.  
- MIT is an open‑source license. Claiming it without releasing the code is misleading.  

**Action:**  
- Delete the MIT license statement.  
- Replace with a clear proprietary notice:  
  > “Contextro is proprietary software. The benchmark harness, task catalogs, and analysis scripts are provided for reproducibility under [choose: MIT / Apache 2.0 / CC‑BY‑NC]. The Contextro server itself is not open source.”  
- Be explicit about what is shared (scripts, JSON task files) and what is not (the MCP server binary/source).

### 🟠 High priority (essential for credibility)

#### 3. Control arm is a straw man (weak baseline)
- Control = `grep` + reading up to 5 files. That is **not how any competent agent works**.  
- The paper acknowledges this in Limitations but the abstract and headline numbers don’t warn the reader.  

**Action:**  
- **Add a stronger baseline** using only free, local tools (e.g., `git grep` + `ast-grep` + `rg` + LSP `find-references` if scriptable). Report its token/latency numbers alongside the grep‑only baseline.  
- **Or**, if you keep the weak baseline, add a **bold caveat** in the abstract and results:  
  > “Compared against a deliberately naive grep‑based baseline; practical agents using modern code search tools would perform better, so the real‑world improvement from Contextro will be smaller than reported.”

#### 4. No end‑to‑end correctness
- The paper measures only **discovery cost** (tokens, latency, file reads).  
- It does not measure whether an agent can use that discovery to make correct code changes.  

**Action:**  
- Add a small end‑to‑end correctness study:  
  - Choose 5‑10 tasks where the agent must actually edit code (e.g., fix a bug, add a parameter, update a function).  
  - Provide the discovery output (from control and treatment) to an LLM agent (e.g., Claude 3.5 Sonnet) and ask it to output the final code change.  
  - Measure success rate (human‑judged correctness), number of iterations (if multi‑turn allowed), and total time.  
  - Report results. This does not require open‑sourcing the server.

#### 5. “Agent” framing is misleading
- Both arms are **scripted pipelines**, not autonomous agents.  
- No LLM decides what to grep or which tool to call.  

**Action:**  
- Rename “control arm” to “grep‑based pipeline” and “treatment arm” to “Contextro pipeline”.  
- Remove “AI agent” from places where no agent loop exists.  
- Be honest: this is a **tool‑assisted retrieval comparison**, not an agent benchmark.

#### 6. Missing MCP tool API schemas
- The paper does not document the input/output JSON schemas for any MCP tool (`find_symbol`, `search`, `impact`, `commit_search`, etc.).  

**Action:**  
- Add an appendix with the exact JSON schema for each tool:  
  - Input parameters (names, types, examples)  
  - Output structure (fields, types, examples)  
- This documents the public API without open‑sourcing the server.

### 🟡 Medium priority (improves trust)

#### 7. Reproducibility without the private repo – create a public proxy
- The headline 98% reduction comes from a private monorepo. No one can verify it.  
- Public `src/` reruns are on a tiny tree.  

**Action:**  
- Create a **public, synthetic but realistic proxy repo** (e.g., a stripped‑down Next.js + React Native + Convex monorepo) of at least 1,000 files.  
- Run the same 39‑task subset on that proxy repo.  
- Report the results in the paper (e.g., as a supplementary table). This allows third parties to verify the effect on a nontrivial codebase.

### ⚪ Low priority (cleanup)

#### 8. Minor issues (typos, inconsistencies)
- Name inconsistency: “Contextro” vs “Contexto” (page 7 first line).  
- Table 1: duplicate “find_callers, find_callers” → should be “find_callers, find_callees”.  
- Table 7: “TOON reduction” → “token reduction”.  
- Table 6: “calllee” → “callee”.  
- Missing figures from “List of Figures” (page 5).  
- Page 17 heading: “1This revision ships…” – missing space after “1”.  
- Page 18 line 1: “1. The fresh reruns…” – extra “1.”.

**Action:**  
- Correct all typos. Use “Contextro” consistently throughout.  
- Remove or fill figure references if figures are not ready.

## Summary Checklist for Coding Agent

| # | Issue | Priority |
|---|-------|----------|
| 1 | Fix PDF formatting (garbage pages, missing Table 9, figure placeholders) | **Critical** |
| 2 | Remove MIT license; add correct proprietary + shared‑scripts license statement | **Critical** |
| 3 | Add stronger baseline (`git grep` + `ast-grep` + `rg`) or add bold caveat in abstract | **High** |
| 4 | Add small end‑to‑end correctness study (5‑10 real code changes) | **High** |
| 5 | Rename arms to “grep‑based pipeline” and “Contextro pipeline” – remove fake “agent” framing | **High** |
| 6 | Document MCP tool JSON schemas (inputs/outputs) in an appendix | **High** |
| 7 | Create a public proxy repo (≥1,000 files) and run the 39 tasks there; report results | Medium |
| 8 | Fix typos and name inconsistencies | Low |

## Verdict (after fixes)

If you fix the **critical** and **high** items, the paper becomes a solid **benchmarking + systems paper** for a conference or workshop that accepts closed‑source evaluations (e.g., industry track, systems demo). Without those fixes, reviewers will reject it due to broken formatting, misleading baseline, lack of end‑to‑end validation, and incorrect licensing.

Hand this checklist to your coding agent. Focus on critical and high items first.