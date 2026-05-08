# Contextro Best-in-Class Research Plan

**Date:** 2026-05-07  
**Author:** Research synthesis from primary sources  
**Status:** Final deliverable  

---

## Executive Summary

After surveying primary sources from Cursor, Windsurf/Devin, Anthropic, OpenAI, Google/DeepMind, NVIDIA, Mistral, and peer-reviewed academic work, three patterns emerge as the highest-ROI improvements for Contextro:

1. **Query-aware retrieval training** — Cursor's custom embedding model trained on agent session traces delivers 12.5% higher accuracy than generic embeddings. Contextro can approximate this by mining its own search logs to fine-tune potion-code-16m or train a distilled reranker.

2. **Structured compaction with retrievable history** — Every leading system (Cursor, Anthropic, OpenAI, Devin) now treats compaction as a first-class feature. Contextro's `session_snapshot` is a good start but lacks the ability to recover specific details post-compaction. Adding a searchable compaction archive (like Cursor's "chat history as files") would close this gap.

3. **Progressive disclosure via lightweight index maps** — Both OpenAI ("give Codex a map, not a manual") and Cursor (dynamic context discovery with 46.9% token reduction) converge on the same principle: return minimal metadata first, let the agent pull details on demand. Contextro's sandbox/retrieve pattern already does this; the improvement is making it the default for all tools, not just oversized search results.

These three changes are implementable within Contextro's current architecture (single MCP server, <350MB RAM, disk-backed) and target the metrics that matter most: retrieval precision, token efficiency, and developer trust.

---

## 1. Company-by-Company Source Matrix

| Company | Topic | Mechanism | Source | Date | Confidence |
|---------|-------|-----------|--------|------|------------|
| **Cursor** | Semantic search | Custom embedding trained on agent traces; LLM ranks retrospective helpfulness | cursor.com/blog/semsearch | Nov 2025 | High |
| **Cursor** | Token efficiency | Dynamic context discovery: files as primitive, MCP tools synced to folders. 46.9% token reduction in A/B test | cursor.com/blog/dynamic-context-discovery | Jan 2026 | High |
| **Cursor** | Text indexing | Sparse N-gram indexes, frequency-weighted selection, git-commit-based state, mmap lookup table + disk postings | cursor.com/blog/fast-regex-search | Mar 2026 | High |
| **Cursor** | Compaction | Chat history written to files; agent can search history post-summarization to recover lost details | cursor.com/blog/dynamic-context-discovery | Jan 2026 | High |
| **Cursor** | Security | Merkle trees for semantic index without storing source on servers | zenml.io (secondary) | 2025 | Medium |
| **Windsurf** | Fast retrieval | SWE-grep: RL-trained subagent for parallel tool calling (8 parallel calls/turn, 4 turns max). 20x faster than agentic search | docs.codeium.com/context-awareness/fast-context | 2025 | High |
| **Windsurf** | Search quality | M-Query: LLM-based search outperforms embedding-based search for code | docs.codeium.com | 2025 | Medium |
| **Windsurf** | Context control | Context pinning for explicit user control over what's in context | docs.codeium.com/context-awareness/overview | 2025 | High |
| **Devin** | Memory | Model writes notes to filesystem as external memory; custom compaction outperforms model self-summarization | cognition.ai/blog/devin-sonnet-4-5-lessons-and-challenges | Sep 2025 | High |
| **Devin** | Context anxiety | Models take shortcuts near context limits. Fix: report 1M window but cap at 200k | cognition.ai/blog | Sep 2025 | High |
| **Devin** | Architecture | Compound AI system: diverse model inferences to plan, act, evaluate, use tools | cognition.ai/blog/evaluating-coding-agents | 2024 | High |
| **Anthropic** | Context engineering | Context as finite resource with diminishing returns. Principle: smallest set of high-signal tokens | anthropic.com/engineering/effective-context-engineering | Sep 2025 | High |
| **Anthropic** | Compaction | Summarize message history, preserve architectural decisions + unresolved bugs. Tool result clearing as lightweight compaction | anthropic.com/engineering/effective-context-engineering | Sep 2025 | High |
| **Anthropic** | Long-running agents | Initializer agent + coding agent. Progress files + git history for cross-session continuity. Feature list as JSON prevents premature completion | anthropic.com/engineering/effective-harnesses-for-long-running-agents | Nov 2025 | High |
| **Anthropic** | Sub-agents | Specialized sub-agents explore extensively (10k+ tokens) but return 1-2k token summaries | anthropic.com/engineering/effective-context-engineering | Sep 2025 | High |
| **Anthropic** | Memory | File-based memory tool released with Sonnet 4.5. Store/recall across sessions | anthropic.com/news/context-management | 2025 | High |
| **OpenAI** | Agent legibility | AGENTS.md as table of contents (~100 lines), not encyclopedia. Progressive disclosure. Structured docs/ directory | openai.com/index/harness-engineering | Feb 2026 | High |
| **OpenAI** | Context management | Compaction, token counting, prompt caching as API features. GPT-5.2-Codex optimized for long-horizon via compaction | openai.com/index/introducing-gpt-5-2-codex | 2026 | High |
| **OpenAI** | Architecture | Repository knowledge as system of record. Linters enforce architecture. 6+ hour single Codex runs | openai.com/index/harness-engineering | Feb 2026 | High |
| **Google** | Long context | Gemini 1.5 Pro: 1M tokens (expanded to 2M). 99%+ retrieval accuracy up to 10M tokens | arxiv.org/html/2403.05530v2 | Feb 2024 | High |
| **Google** | Infini-attention | Compressive memory in vanilla attention. Bounded memory for unbounded context. Both local + long-term linear attention | arxiv.org/html/2404.07143v1 | Apr 2024 | High |
| **Google** | Retrieval at limit | Gemini 2.5 Flash: near-perfect needle-in-haystack even at context limit | arxiv.org/html/2511.05850v1 | 2025 | High |
| **NVIDIA** | RAG architecture | Enterprise RAG Blueprint: modular reference. ReAct agent decides retrieve vs respond. NeMo Retriever for embedding + reranking | developer.nvidia.com/blog | 2025 | High |
| **NVIDIA** | Agentic RAG | Autonomous retrieval activation only when needed. Multi-agent self-corrective systems | developer.nvidia.com/blog | 2025 | Medium |
| **DeepSeek** | Hybrid attention | CSA (Compressed Sparse Attention) + HCA (Heavily Compressed Attention). Compresses KV cache to 1/m then applies top-k sparse selection. 10% KV cache vs V3.2 at 1M tokens | huggingface.co/deepseek-ai/DeepSeek-V4 (paper) | 2026 | High |
| **DeepSeek** | On-disk KV cache | Heterogeneous KV cache with on-disk storage. Periodic checkpointing for SWA entries. Three strategies: full caching, periodic checkpoint, zero-cache with recomputation | DeepSeek-V4 paper §3.5.2 | 2026 | High |
| **DeepSeek** | Interleaved thinking | Tool-calling scenarios preserve full reasoning history across all rounds. General conversations discard previous reasoning on new user turn | DeepSeek-V4 paper §5.1.1 | 2026 | High |
| **DeepSeek** | Quick Instruction | Special tokens for auxiliary tasks (search trigger, title gen, query gen) that reuse existing KV cache. Avoids redundant prefilling | DeepSeek-V4 paper §5.1.1 | 2026 | High |
| **DeepSeek** | Sandbox infrastructure | DSec: production-grade sandbox with trajectory logging and preemption-safe resumption. Deterministic replay from trajectory | DeepSeek-V4 paper §5.2.5 | 2026 | Medium |
| **DeepSeek** | Context scaling | RL framework scaled to 1M-token context. Preemptible rollout with token-granular WAL (Write-Ahead Log) for fault tolerance | DeepSeek-V4 paper §5.2.4 | 2026 | Medium |
| **Mistral** | Efficient attention | Sliding window attention in Mistral 7B for efficient long-context | huggingface.co/docs/transformers/model_doc/mistral | 2023 | High |
| **Mistral** | Agents | Agents API with context management and tool coordination. Devstral 2 (123B/24B) for coding | mistral.ai/news/devstral-2-vibe-cli | 2025 | Medium |
| **Academic** | Prompt caching | 45-80% cost reduction, 13-31% TTFT improvement. System prompt only caching most consistent. Full context can regress latency | arxiv 2601.06007 | Jan 2026 | High |
| **Academic** | KV cache compression | SideQuest: model-driven compression, 65% peak token reduction on agentic tasks, minimal accuracy loss | arxiv 2602.22603 | Feb 2026 | High |
| **Academic** | KV eviction | KV Policy: RL-trained per-head agents for token eviction decisions | arxiv 2602.10238 | Feb 2026 | Medium |
| **Academic** | Memory hierarchy | MemGPT: OS-inspired virtual context. Working memory + long-term with store/retrieve/summarize/update primitives | research.memgpt.ai | Oct 2023 | High |
| **Academic** | Hierarchical memory | HiMem: hierarchical long-term memory for long-horizon agents. Construction, retrieval, dynamic updating | arxiv 2601.06377 | Jan 2026 | Medium |
| **Academic** | Code compression | Code-specific prompt compression for RAG in coding tasks. Compresses retrieved code examples | arxiv 2502.14925 | Feb 2025 | Medium |
| **Academic** | Sentence compression | Context-aware sentence encoding: relevance scoring per sentence for compression | arxiv 2409.01227 | Sep 2024 | Medium |

---

## 2. Adopt / Adapt / Avoid Table

| Strategy | Verdict | Rationale | Effort | Risk |
|----------|---------|-----------|--------|------|
| **Dynamic context discovery (files as primitive)** | ADAPT | Contextro already uses sandbox/retrieve. Extend to all tool outputs above a threshold. Don't need filesystem—use in-memory sandbox with ref IDs | Low | Low |
| **Custom embedding from agent traces** | ADAPT | Can't train a full model, but can mine search logs to build hard-negative training pairs for fine-tuning potion-code-16m or training a reranker | Medium | Medium |
| **Sparse N-gram text index** | AVOID (for now) | Contextro already has BM25 via LanceDB FTS. Sparse N-grams shine for monorepos (>50k files). Not worth the complexity for typical Contextro users | High | Low |
| **SWE-grep style subagent** | AVOID | Requires training a specialized model. Contextro is model-agnostic MCP server—this is the client's job | High | High |
| **M-Query (LLM-based search)** | ADAPT | Contextro could offer an optional "LLM rerank" mode using the calling model itself to re-score top-K results. Lightweight to implement | Low | Low |
| **Context pinning** | ADOPT | Add explicit "pin" capability to remember/recall. Let users mark certain search results or symbols as always-include | Low | Low |
| **Compaction with searchable history** | ADOPT | Extend session_snapshot to write full compaction archive. Make it searchable via recall/search | Medium | Low |
| **Tool result clearing** | ADOPT | Already partially done via sandbox. Formalize: after N turns, offer to clear old tool results from context | Low | Low |
| **Progressive disclosure (map not manual)** | ADOPT | Make overview/architecture return compact maps by default. Detailed content on demand via retrieve | Low | Low |
| **Prompt caching optimization** | ADAPT | Structure Contextro's tool responses to be cache-friendly: stable metadata prefix + dynamic content suffix | Low | Low |
| **Infini-attention / compressive memory** | AVOID | Architectural research, not applicable to an MCP server. This is the model provider's job | N/A | N/A |
| **MemGPT-style virtual context** | ADAPT | Contextro's remember/recall already implements the core primitives. Add automatic summarization of old memories and hierarchical organization | Medium | Low |
| **Hierarchical memory (HiMem)** | ADAPT | Add memory categories (decisions, facts, preferences) with automatic relevance decay. Already have tags—add TTL-based pruning | Low | Low |
| **Code-specific prompt compression** | ADAPT | Contextro already does snippet truncation. Add AST-aware compression: keep signatures, collapse bodies, preserve imports | Medium | Medium |
| **Relevance-scored sentence compression** | ADAPT | Apply to search results: score each line's relevance to query, drop low-scoring lines from snippets | Medium | Medium |
| **ReAct-style conditional retrieval** | ADOPT | Already implemented via search confidence levels. Formalize: if confidence=high, return fewer results | Low | Low |
| **Git-commit-based index state** | ADOPT | Already using git for branch detection. Extend: store index generation keyed to commit hash for instant validation | Low | Low |
| **Initializer + progress file pattern** | ADAPT | Contextro can generate a "project context file" on first index that serves as the agent's map. Auto-update on reindex | Medium | Low |
| **Interleaved thinking (DeepSeek-V4)** | ADAPT | Contextro's compaction archive already preserves tool-call context. Could add a mode where tool results are preserved across compactions while general chat is discarded | Low | Low |
| **On-disk KV cache with periodic checkpointing (DeepSeek-V4)** | ADAPT | Contextro already uses disk-backed LanceDB. The periodic checkpoint pattern maps to compaction archive: checkpoint every N tool calls, not just at context limit | Low | Low |
| **Quick Instruction / KV reuse (DeepSeek-V4)** | ADAPT | Structure tool responses with stable prefix (metadata) + dynamic suffix (content) to maximize prompt cache hits. Already implemented in cache-friendly response structure | Low | Low |
| **Trajectory logging (DeepSeek-V4 DSec)** | ADAPT | The compaction archive is a form of trajectory logging. Could extend to log every tool call + result for deterministic replay | Medium | Low |

---

## 3. Ranked Highest-ROI Improvements

### Tier 1: Immediate (1-2 weeks, high impact, low risk)

| # | Improvement | Expected Benefit | Metric | Cost |
|---|-------------|-----------------|--------|------|
| 1 | **Universal progressive disclosure** — All tools return compact summaries by default with sandbox refs for details | 30-50% token reduction per session | Tokens/session, sandbox hit rate | 3-5 days |
| 2 | **Searchable compaction archive** — When session_snapshot fires, write full history to a searchable store. Agent can recall specific details post-compaction | Eliminate post-compaction knowledge loss | Task completion rate across compactions | 3-4 days |
| 3 | **Cache-friendly response structure** — Stable metadata prefix (file, line, symbol type) + dynamic content suffix in all tool responses | Enable 45-80% cost reduction for clients using prompt caching | Cache hit rate (client-side) | 2-3 days |
| 4 | **Confidence-adaptive result count** — When top result scores very high, return fewer results. When scores are spread, return more | 15-25% token reduction on high-confidence queries | Tokens/query at constant recall | 1-2 days |

### Tier 2: Medium-term (2-6 weeks, high impact, medium effort)

| # | Improvement | Expected Benefit | Metric | Cost |
|---|-------------|-----------------|--------|------|
| 5 | **AST-aware snippet compression** — Keep function signatures + docstrings, collapse implementation bodies, preserve imports/types | 40-60% snippet size reduction with minimal information loss | Tokens/result, answer quality | 2-3 weeks |
| 6 | **Query-log-trained reranker** — Mine Contextro search logs to build training pairs. Fine-tune FlashRank or train lightweight reranker | 10-15% retrieval precision improvement | MRR, Recall@5 | 3-4 weeks |
| 7 | **Hierarchical memory with auto-summarization** — Group memories by type (decision/fact/preference). Auto-summarize old memories. Relevance decay | Better recall precision, reduced memory noise | Memory recall precision, storage efficiency | 2-3 weeks |
| 8 | **Project context map** — Auto-generate on first index: key entry points, architecture layers, hot files, dependency graph summary. Serve as first-call response | Faster agent orientation, fewer exploratory searches | Searches-to-first-useful-result | 2 weeks |

### Tier 3: Long-term (1-3 months, transformative, higher risk)

| # | Improvement | Expected Benefit | Metric | Cost |
|---|-------------|-----------------|--------|------|
| 9 | **Relevance-scored line compression** — Per-line relevance scoring within search results. Drop low-relevance lines, keep high-signal ones | 50-70% further snippet compression | Tokens/result at constant answer quality | 4-6 weeks |
| 10 | **Agent-trace-trained embeddings** — Collect anonymized search→success traces. Train custom embedding model aligned to actual developer workflows | 12-25% retrieval accuracy improvement (per Cursor's results) | MRR, code retention, user satisfaction | 2-3 months |
| 11 | **Cross-session learning** — Track which search results led to successful outcomes. Boost those chunks in future searches | Personalized retrieval improvement over time | Per-user MRR improvement | 2-3 months |

---

## 4. Architecture Recommendations

### Immediate: Preserve current architecture, optimize outputs

Contextro's architecture (LanceDB + rustworkx + tree-sitter + Model2Vec) is sound and competitive. No structural changes needed. Focus on:

- **Response shaping**: Every tool response should follow the pattern: `{compact_summary, sandbox_ref?, confidence, token_count}`. The agent decides whether to retrieve full content.
- **Compaction support**: Add a `compaction_archive` store (SQLite or LanceDB table) that holds full session histories, searchable via the existing vector engine.
- **Cache-friendliness**: Ensure tool response structure is deterministic and stable for the metadata portion. Dynamic content (code snippets, line numbers) goes last.

### Medium-term: Add intelligence to retrieval

- **Adaptive retrieval**: Use query classification (keyword vs. semantic vs. structural) to route queries to the optimal search mode automatically. Currently the agent must specify `mode=hybrid|vector|bm25`.
- **Reranker training pipeline**: Add a script that mines search logs + follow-up actions to generate training pairs. Use these to fine-tune FlashRank or train a small cross-encoder.
- **AST-aware compression**: Extend the chunking pipeline to produce "compressed" variants of chunks (signatures only) alongside full chunks. Return compressed by default, full on demand.

### Long-term: Learn from usage

- **Feedback loop**: Track which search results the agent actually uses (reads the full content via retrieve). Use this signal to boost/demote chunks.
- **Embedding fine-tuning**: If sufficient trace data accumulates, fine-tune potion-code-16m on hard negatives mined from search sessions where the agent had to search multiple times before finding the right result.
- **Multi-resolution index**: Store chunks at multiple granularities (file-level, function-level, line-level). Route queries to the appropriate resolution based on query type.

---

## 5. Top Experiments with Success Criteria

### Experiment 1: Progressive Disclosure Default

**Hypothesis:** Making all tool responses return compact summaries by default (with sandbox refs) will reduce tokens/session by 30%+ without degrading task success.

**Protocol:**
1. Benchmark current tokens/session on 20 representative tasks
2. Implement universal progressive disclosure
3. Re-run same 20 tasks
4. Measure: tokens/session, task completion rate, number of retrieve calls

**Success criteria:** ≥30% token reduction, ≤5% task completion regression, retrieve calls < 40% of sandbox refs issued

**Risk:** Agents may over-retrieve, negating savings. Mitigation: tune sandbox threshold.

---

### Experiment 2: AST-Aware Snippet Compression

**Hypothesis:** Returning function signatures + docstrings (collapsing bodies) will reduce snippet tokens by 40%+ while maintaining answer quality.

**Protocol:**
1. Generate 20 queries against current index, measure tokens/result and answer quality (human eval)
2. Implement AST-aware compression (keep signature, first docstring line, collapse body to `...`)
3. Re-run same queries
4. Measure: tokens/result, human-rated answer quality (1-5 scale)

**Success criteria:** ≥40% token reduction per result, answer quality ≥4.0/5.0 (vs baseline ≥4.2/5.0)

**Risk:** Some queries need implementation details. Mitigation: full content always available via retrieve.

---

### Experiment 3: Confidence-Adaptive Result Count

**Hypothesis:** Returning fewer results when the top result has very high confidence will save tokens without hurting recall.

**Protocol:**
1. Analyze score distributions across 100 queries
2. Implement adaptive logic: if top_score > 0.85, return max 3 results; if spread, return up to 10
3. Measure: tokens/query, Recall@K, user satisfaction

**Success criteria:** ≥15% token reduction, Recall@5 maintained within 5% of baseline

**Risk:** Aggressive thresholds may miss relevant results. Mitigation: always include diversity penalty to avoid single-file dominance.

---

### Experiment 4: Searchable Compaction Archive

**Hypothesis:** Storing full pre-compaction history in a searchable archive will eliminate post-compaction knowledge loss.

**Protocol:**
1. Design 5 multi-step tasks that require >1 compaction cycle
2. Run with current session_snapshot (baseline)
3. Run with searchable archive (agent can search old context)
4. Measure: task completion rate, errors from forgotten context

**Success criteria:** ≥50% reduction in post-compaction errors, <100ms archive search latency

**Risk:** Archive grows unbounded. Mitigation: TTL-based expiry (24h default).

---

### Experiment 5: Query-Log-Trained Reranker

**Hypothesis:** A reranker trained on Contextro's own search logs will outperform generic FlashRank.

**Protocol:**
1. Collect 1000+ search→retrieve pairs (query, clicked result, skipped results)
2. Train lightweight cross-encoder on these pairs
3. A/B test against FlashRank on retrieval quality benchmark

**Success criteria:** ≥10% MRR improvement over FlashRank, <5ms additional latency

**Risk:** Insufficient training data initially. Mitigation: start with synthetic hard negatives from the existing benchmark.

---

## 6. Risks, Tradeoffs, and Open Questions

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Progressive disclosure causes agents to over-retrieve, increasing total tokens | Medium | Medium | Tune sandbox threshold; monitor retrieve-to-sandbox ratio |
| AST compression loses critical implementation details | Low | High | Always offer full content via retrieve; never compress below signature level |
| Reranker overfits to specific codebase patterns | Medium | Medium | Train on diverse codebases; validate on held-out repos |
| Compaction archive grows unbounded | Low | Low | TTL-based expiry; size cap with LRU eviction |
| Cache-friendly responses constrain future tool design | Low | Low | Only the prefix structure is fixed; content remains flexible |

### Tradeoffs

1. **Token efficiency vs. completeness**: Every compression technique trades information for tokens. The key insight from all sources: make full content *available* but not *default*.

2. **Latency vs. quality**: Reranking adds latency. FlashRank adds ~2ms. A custom cross-encoder might add 5-10ms. Acceptable for search but not for autocomplete-speed tools.

3. **Simplicity vs. intelligence**: Cursor and Windsurf invest heavily in custom models. Contextro's strength is being model-agnostic and local-first. Don't sacrifice this for marginal retrieval gains.

4. **Privacy vs. learning**: Agent-trace-trained embeddings require collecting usage data. Contextro is "no data leaves your machine." Any learning must happen locally.

### Open Questions

1. **How much does embedding quality matter as models get smarter?** Cursor found 12.5% improvement from custom embeddings, but models are getting better at compensating for imperfect retrieval. Will this gap close?

2. **Is LLM-based reranking (M-Query style) worth the latency for a local MCP server?** Windsurf does this server-side. For Contextro, it would require calling the host model, which adds a round-trip.

3. **What's the optimal compaction frequency?** Anthropic and Devin both do it at context limits. Should Contextro proactively suggest compaction earlier to maintain quality?

4. **Can graph-based retrieval (rustworkx) be used for relevance scoring?** Hypothesis: chunks connected to more callers/callees are more architecturally important and should rank higher. Untested.

5. **Should Contextro offer a "project map" tool that's always called first?** OpenAI's harness engineering suggests yes. But this adds a mandatory round-trip. Tradeoff unclear.

---

## 7. Final Recommendation: Three Changes to Implement First

Based on ROI (impact × confidence / effort × risk):

### 1. Universal Progressive Disclosure (Tier 1, #1)

**What:** Every tool that can return >1200 tokens returns a compact summary + sandbox_ref. The agent pulls full content only when needed.

**Why:** 
- Cursor proved 46.9% token reduction with this pattern (A/B tested, statistically significant)
- Anthropic's core principle: "smallest possible set of high-signal tokens"
- OpenAI's principle: "give Codex a map, not a manual"
- Contextro already has the sandbox infrastructure; this is about making it the default

**How:** Extend `response_policy.py` to apply progressive disclosure to `explain`, `find_callers`, `find_callees`, `impact`, `architecture`, and `analyze` (not just `search`).

**Metric:** Tokens/session reduction ≥30% on benchmark tasks.

---

### 2. Searchable Compaction Archive (Tier 1, #2)

**What:** When `session_snapshot` fires (or when the client signals compaction), write the full pre-compaction context to a searchable store. The agent can search this archive to recover specific details.

**Why:**
- Cursor: "chat history as files" for summarization recovery
- Devin: "custom compaction outperforms model self-summarization"
- Anthropic: "the art of compaction lies in selection of what to keep vs discard"
- Current session_snapshot is lossy. This makes it lossless-on-demand.

**How:** Add a `compaction_archive` LanceDB table. On compaction, embed and store the full message history. Expose via `recall(query, source="archive")` or a new `search_archive` tool.

**Metric:** Post-compaction task completion rate improvement ≥50%.

---

### 3. AST-Aware Snippet Compression (Tier 2, #5)

**What:** Search results return compressed snippets by default: function signature + first docstring line + `...` for body. Full content available via retrieve.

**Why:**
- Code-specific prompt compression paper (arxiv 2502.14925) validates this approach for coding RAG
- Contextro's current snippets include full function bodies, which are often 80%+ implementation detail irrelevant to the query
- Combined with progressive disclosure, this could achieve 60-70% total token reduction

**How:** Extend the chunking pipeline to generate a `compressed_snippet` field alongside `code`. Use tree-sitter to identify function boundaries, extract signature + docstring, collapse body.

**Metric:** Tokens/result reduction ≥40% at constant Recall@5.

---

## Appendix: Key Citations

1. Cursor, "Dynamic context discovery," cursor.com/blog, Jan 6, 2026
2. Cursor, "Improving agent with semantic search," cursor.com/blog, Nov 6, 2025
3. Cursor, "Fast regex search: indexing text for agent tools," cursor.com/blog, Mar 23, 2026
4. Windsurf/Codeium, "Fast Context," docs.codeium.com/context-awareness/fast-context, 2025
5. Windsurf/Codeium, "Context Awareness Overview," docs.codeium.com, 2025
6. Cognition, "Rebuilding Devin for Claude Sonnet 4.5," cognition.ai/blog, Sep 29, 2025
7. Anthropic, "Effective context engineering for AI agents," anthropic.com/engineering, Sep 29, 2025
8. Anthropic, "Effective harnesses for long-running agents," anthropic.com/engineering, Nov 26, 2025
9. OpenAI, "Harness engineering: leveraging Codex in an agent-first world," openai.com/index, Feb 11, 2026
10. OpenAI, "Introducing GPT-5.2-Codex," openai.com/index, 2026
11. Google/DeepMind, "Gemini 1.5 Pro technical report," arxiv 2403.05530v2, Feb 2024
12. Google/DeepMind, "Infini-attention," arxiv 2404.07143v1, Apr 2024
13. Lumer et al., "An Evaluation of Prompt Caching for Long-Horizon Agentic Tasks," arxiv 2601.06007, Jan 2026
14. SideQuest, "Model-Driven KV Cache Management," arxiv 2602.22603, Feb 2026
15. MemGPT, "Towards LLMs as Operating Systems," research.memgpt.ai, Oct 2023
16. HiMem, "Hierarchical Long-Term Memory for LLM Long-Horizon Agents," arxiv 2601.06377, Jan 2026
17. "Code-specific Prompt Compression for RAG in Coding Tasks," arxiv 2502.14925, Feb 2025
18. "Prompt Compression with Context-Aware Sentence Encoding," arxiv 2409.01227, Sep 2024
19. NVIDIA, "Build a RAG Agent with NVIDIA Nemotron," developer.nvidia.com/blog, 2025
20. Mistral AI, "Devstral 2 and Mistral Vibe CLI," mistral.ai/news, 2025
21. DeepSeek-AI, "DeepSeek-V4: Towards Highly Efficient Million-Token Context Intelligence," huggingface.co/deepseek-ai/DeepSeek-V4, 2026
