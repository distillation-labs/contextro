"""Tests for research catalog registry."""

from contextro_mcp.research.catalog import build_default_catalog


def test_default_catalog_has_expected_publishers():
    catalog = build_default_catalog()
    publishers = {entry.publisher for entry in catalog.entries}
    assert {"Anthropic", "NVIDIA", "OpenAI", "arXiv"}.issubset(publishers)


def test_catalog_target_filter_matches_execution_compaction():
    catalog = build_default_catalog()
    matches = catalog.by_target("execution/compaction.py")
    assert matches
    assert any("long-context" in entry.topic.lower() for entry in matches)


def test_catalog_target_filter_matches_governance_surfaces():
    catalog = build_default_catalog()
    matches = catalog.by_target("security/permissions.py")
    assert matches
    assert any("rule" in entry.topic.lower() or "hook" in entry.topic.lower() for entry in matches)
