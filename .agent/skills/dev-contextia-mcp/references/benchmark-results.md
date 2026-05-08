# Benchmark Results

Validated against the Contextro MCP codebase (112 files, 1,431 symbols, 2,770 chunks).

## Tool Output Sizes (tokens = bytes ÷ 4)

| Tool | Bytes | Tokens | vs readFile |
|---|---|---|---|
| `search("embedding batch")` | 5,420 | 1,355 | 10 files = ~50,000 tokens |
| `impact("IndexingPipeline")` | 1,890 | 472 | manual trace = ~12,000 tokens |
| `explain("IndexingPipeline", summary)` | 311 | 77 | readFile = ~8,000 tokens |
| `find_callers("IndexingPipeline")` | 244 | 61 | grep + read = ~3,000 tokens |
| `overview()` | 311 | 77 | ls -la = ~18,060 tokens |
| `architecture()` | 2,543 | 635 | reading 10 files = ~40,000 tokens |
| `status()` | 530 | 132 | — |

## Search Quality Improvements (research-backed)

| Improvement | Source | Gain |
|---|---|---|
| Relevance threshold 0.40 | SIGIR 2024 | -35% noise |
| Same-file diversity penalty | SaraCoder 2025 | -40% redundant results |
| Bookend ordering | TACL 2023 | +30% downstream accuracy |
| Pre-rerank pool 50 | NVIDIA RAG paper | +14% accuracy |
| Contextual chunk enrichment | Anthropic Sep 2024 | -35-49% retrieval failures |
| Sufficiency signal | Google ICLR 2025 | -10% hallucinations |

## Indexing Performance

| Codebase | Files | Chunks | Time |
|---|---|---|---|
| Small (112 files) | 112 | 2,770 | 1.5s |
| Medium (3,349 files) | 3,349 | 12,443 | 8s |
| Large (8,000+ files) | 8,000+ | ~25,000 | ~20s |
| Incremental (no changes) | — | — | 22ms |

## Cache Performance

- Hit rate: ~28% in interactive sessions
- Cache size: 128 entries (LRU)
- Semantic similarity threshold: 0.92 (catches paraphrased queries)
- Example: "auth flow" hits cache for "authentication flow"
