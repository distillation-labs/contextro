"""Research entry registry used by docs and benchmark workflows."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True, slots=True)
class ResearchEntry:
    """A single research insight linked to a concrete implementation area."""

    topic: str
    source: str
    publisher: str
    year: int
    claim: str
    implementation_targets: tuple[str, ...]


@dataclass(slots=True)
class ResearchCatalog:
    """In-memory catalog of research entries."""

    entries: list[ResearchEntry] = field(default_factory=list)

    def add(self, entry: ResearchEntry) -> None:
        self.entries.append(entry)

    def by_target(self, target: str) -> list[ResearchEntry]:
        """Return entries that map to a given implementation target."""
        needle = target.lower()
        return [
            entry
            for entry in self.entries
            if any(needle in mapped.lower() for mapped in entry.implementation_targets)
        ]

    def to_markdown(self) -> str:
        """Render catalog as compact markdown bullets."""
        lines = ["# Research Catalog", ""]
        for entry in self.entries:
            targets = ", ".join(entry.implementation_targets)
            lines.append(
                f"- **{entry.topic}** ({entry.publisher}, {entry.year}) — {entry.claim} "
                f"Source: {entry.source}. Targets: {targets}."
            )
        return "\n".join(lines)


def build_default_catalog() -> ResearchCatalog:
    """Build the default research catalog used by Contextia docs.

    Living product documentation is versioned inconsistently, so entries for
    Windsurf, Devin, and Gemini CLI use the access year when an immutable
    publication date is not exposed by the source.
    """
    catalog = ResearchCatalog()

    catalog.add(
        ResearchEntry(
            topic="Contextual retrieval and contextual BM25",
            source="https://www.anthropic.com/engineering/contextual-retrieval",
            publisher="Anthropic",
            year=2024,
            claim=(
                "Contextual chunk augmentation plus hybrid retrieval reduces "
                "top-k retrieval failures in RAG workflows"
            ),
            implementation_targets=(
                "indexing/chunker.py",
                "indexing/smart_chunker.py",
                "execution/search.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Long-context degradation in middle tokens",
            source="https://arxiv.org/abs/2307.03172",
            publisher="arXiv",
            year=2023,
            claim=(
                "Long-context models degrade when critical evidence sits in the middle, "
                "supporting focused previews, progressive retrieval, and bookended "
                "result ordering"
            ),
            implementation_targets=(
                "execution/compaction.py",
                "execution/response_policy.py",
                "execution/search.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="RAG systems as end-to-end pipelines",
            source=(
                "https://developer.nvidia.com/blog/"
                "rag-101-retrieval-augmented-generation-questions-answered/"
            ),
            publisher="NVIDIA",
            year=2023,
            claim=(
                "Retrieval quality and latency improve when chunking, indexing, "
                "retrieval, reranking, and evaluation are tuned as one system"
            ),
            implementation_targets=(
                "scripts/benchmark_token_efficiency.py",
                "scripts/benchmark_retrieval_quality.py",
                "docs/RESEARCH.md",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Prompt prefix caching for repeated agent context",
            source="https://openai.com/index/api-prompt-caching/",
            publisher="OpenAI",
            year=2024,
            claim=(
                "Reusing stable prompt prefixes reduces latency and input-token cost, "
                "supporting compact previews, sandbox references, and session reuse "
                "instead of resending large repeated context"
            ),
            implementation_targets=(
                "execution/response_policy.py",
                "engines/output_sandbox.py",
                "memory/session_tracker.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Chunking strategy evaluation across content types",
            source=(
                "https://developer.nvidia.com/blog/"
                "finding-the-best-chunking-strategy-for-accurate-ai-responses/"
            ),
            publisher="NVIDIA",
            year=2025,
            claim=(
                "Chunking strategy should be benchmarked per corpus/query shape rather than fixed "
                "globally, reinforcing Contextia's need for chunk-profile evaluation instead of a "
                "single hard-coded indexing profile"
            ),
            implementation_targets=(
                "indexing/chunk_context.py",
                "indexing/chunker.py",
                "scripts/benchmark_chunk_profiles.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Scoped rules and AGENTS files for durable instructions",
            source="https://docs.windsurf.com/windsurf/cascade/memories",
            publisher="Windsurf",
            year=2026,
            claim=(
                "Rules and AGENTS.md files can stay scoped by repo or directory, "
                "keeping durable instructions targeted instead of always-on"
            ),
            implementation_targets=(
                "server.py",
                "security/permissions.py",
                "middleware/audit.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Hook-based governance and blocking preflight checks",
            source="https://docs.windsurf.com/windsurf/cascade/hooks",
            publisher="Windsurf",
            year=2026,
            claim=(
                "Pre-hooks receive structured JSON and can block reads, writes, "
                "commands, or MCP calls for policy enforcement and audit"
            ),
            implementation_targets=(
                "middleware/audit.py",
                "security/permissions.py",
                "server.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Per-task worktrees and bootstrap hooks",
            source="https://docs.windsurf.com/windsurf/cascade/worktrees",
            publisher="Windsurf",
            year=2026,
            claim=(
                "Per-task worktrees isolate parallel edits, while setup hooks "
                "restore local state that is not committed to the repo"
            ),
            implementation_targets=(
                "git/cross_repo.py",
                "git/branch_watcher.py",
                "server.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Declarative environment blueprints and boot knowledge",
            source="https://docs.devin.ai/onboard-devin/environment/blueprints",
            publisher="Devin",
            year=2026,
            claim=(
                "Environment blueprints build reusable snapshots, while lightweight "
                "knowledge entries capture lint, test, and build commands for session boot"
            ),
            implementation_targets=(
                "server.py",
                "memory/memory_store.py",
                "config.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Knowledge retrieval works best with specific triggers",
            source="https://docs.devin.ai/onboard-devin/knowledge-onboarding",
            publisher="Devin",
            year=2026,
            claim=(
                "Knowledge is more reusable when triggers are specific and specialized "
                "repo files such as AGENTS.md or rule files are ingested automatically"
            ),
            implementation_targets=(
                "server.py",
                "memory/memory_store.py",
                "git/cross_repo.py",
            ),
        )
    )
    catalog.add(
        ResearchEntry(
            topic="Checkpointing, token caching, and context files for terminal agents",
            source=(
                "https://raw.githubusercontent.com/google-gemini/"
                "gemini-cli/main/README.md"
            ),
            publisher="Google",
            year=2026,
            claim=(
                "Checkpointing, token caching, and repo-scoped context files make "
                "terminal agent sessions more resumable without replaying full history"
            ),
            implementation_targets=(
                "memory/session_tracker.py",
                "execution/runtime.py",
                "server.py",
            ),
        )
    )

    return catalog
