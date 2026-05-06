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
    """Build the default research catalog used by Contextia docs."""
    catalog = ResearchCatalog()

    catalog.add(
        ResearchEntry(
            topic="Contextual retrieval and contextual BM25",
            source="https://www.anthropic.com/engineering/contextual-retrieval",
            publisher="Anthropic",
            year=2024,
            claim=(
                "Contextual chunk augmentation + BM25/embedding hybrid retrieval "
                "reduces top-k retrieval failure rates in RAG pipelines"
            ),
            implementation_targets=(
                "indexing/chunker.py",
                "indexing/smart_chunker.py",
                "engines/fusion.py",
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
                "supporting focused chunk previews and progressive retrieval"
            ),
            implementation_targets=(
                "execution/compaction.py",
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
                "Accuracy and latency gains depend on whole-pipeline optimization across "
                "chunking, indexing, retrieval, reranking, and evaluation"
            ),
            implementation_targets=(
                "scripts/benchmark_token_efficiency.py",
                "scripts/benchmark_retrieval_quality.py",
                "docs/RESEARCH.md",
            ),
        )
    )

    return catalog
