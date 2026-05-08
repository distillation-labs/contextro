"""Tests for universal progressive disclosure via ToolResponsePolicy."""

import json

from contextro_mcp.engines.output_sandbox import OutputSandbox
from contextro_mcp.execution.response_policy import ToolResponsePolicy
from contextro_mcp.memory.compaction_archive import CompactionArchive


def _make_policy(threshold: int = 100) -> ToolResponsePolicy:
    sandbox = OutputSandbox(max_entries=50, ttl=300.0)
    return ToolResponsePolicy(output_sandbox=sandbox, threshold_tokens=threshold)


def test_small_response_passes_through():
    """Responses under threshold are returned unchanged."""
    policy = _make_policy(threshold=500)
    data = {"symbol": "foo", "total": 2, "callers": ["a", "b"]}
    result = policy.apply(data, tool_name="find_callers")
    assert result == data
    assert "sandbox_ref" not in result


def test_large_response_gets_sandboxed():
    """Responses over threshold get sandboxed with preview."""
    policy = _make_policy(threshold=50)  # Very low threshold
    data = {
        "symbol": "BigClass",
        "total": 25,
        "callers": [f"caller_{i} (src/mod{i}.py:{i * 10})" for i in range(25)],
    }
    result = policy.apply(data, tool_name="find_callers", preview_keys=["symbol", "total"])
    assert "sandbox_ref" in result
    assert result["sandboxed"] is True
    assert result["sandbox_ref"].startswith("sx_")
    assert result["symbol"] == "BigClass"
    assert result["total"] == 25
    assert "hint" in result
    # Preview should have truncated callers list
    assert len(result["callers"]) <= 5
    assert "callers_total" in result
    assert result["callers_total"] == 25


def test_sandbox_ref_is_retrievable():
    """Content stored via disclosure can be retrieved."""
    sandbox = OutputSandbox(max_entries=50, ttl=300.0)
    policy = ToolResponsePolicy(output_sandbox=sandbox, threshold_tokens=50)
    data = {
        "symbol": "test",
        "impacted_symbols": [f"sym_{i}" for i in range(30)],
        "impacted_files": {f"file_{i}.py": [f"fn_{i}"] for i in range(10)},
    }
    result = policy.apply(data, tool_name="impact")
    assert "sandbox_ref" in result

    # Retrieve full content
    content = sandbox.retrieve(result["sandbox_ref"])
    assert content is not None
    full = json.loads(content)
    assert len(full["impacted_symbols"]) == 30


def test_preview_keys_always_included():
    """Specified preview_keys are always in the preview."""
    policy = _make_policy(threshold=30)
    data = {
        "symbol": "X",
        "max_depth": 10,
        "total_impacted": 50,
        "impacted_symbols": [f"s{i}" for i in range(50)],
    }
    result = policy.apply(
        data, tool_name="impact", preview_keys=["symbol", "max_depth", "total_impacted"]
    )
    assert result["symbol"] == "X"
    assert result["max_depth"] == 10
    assert result["total_impacted"] == 50


def test_dict_fields_summarized_when_large():
    """Large dict fields get summarized in preview."""
    policy = _make_policy(threshold=30)
    data = {
        "quality": {"score": 85},
        "complexity": {f"func_{i}": {"cc": i} for i in range(100)},
    }
    result = policy.apply(data, tool_name="analyze", preview_keys=["quality"])
    assert result["quality"] == {"score": 85}
    # Large dict should be summarized
    assert "complexity_summary" in result or "complexity" in result


def test_large_string_fields_are_truncated_in_preview():
    """Large string fields keep a short inline preview plus original length."""
    policy = _make_policy(threshold=30)
    data = {"content": "x" * 1000}

    result = policy.apply(data, tool_name="skill_prompt")

    assert result["sandboxed"] is True
    assert result["content"].endswith("…")
    assert len(result["content"]) < len(data["content"])
    assert result["content_chars"] == 1000


# --- Compaction Archive Tests ---


def test_archive_stores_and_retrieves():
    """Archived content can be retrieved by ref_id."""
    archive = CompactionArchive(max_entries=10, ttl=3600.0)
    content = "User asked about auth flow.\nAgent searched for authentication.\nFound 5 results."
    ref_id = archive.archive(content)
    assert ref_id.startswith("ca_")
    assert archive.retrieve(ref_id) == content


def test_archive_search_finds_matches():
    """Search returns excerpts matching the query."""
    archive = CompactionArchive(max_entries=10, ttl=3600.0)
    archive.archive(
        "Line 1: setup\nLine 2: auth middleware\nLine 3: JWT validation\n"
        "Line 4: token refresh\nLine 5: cleanup"
    )
    results = archive.search("JWT")
    assert len(results) == 1
    assert any("JWT" in exc for exc in results[0]["excerpts"])


def test_archive_search_no_match():
    """Search returns empty when nothing matches."""
    archive = CompactionArchive(max_entries=10, ttl=3600.0)
    archive.archive("some unrelated content about databases")
    results = archive.search("authentication")
    assert results == []


def test_archive_lru_eviction():
    """Old entries are evicted when max_entries is reached."""
    archive = CompactionArchive(max_entries=2, ttl=3600.0)
    ref1 = archive.archive("first entry")
    archive.archive("second entry")
    archive.archive("third entry")
    assert archive.size == 2
    assert archive.retrieve(ref1) is None  # Evicted


def test_archive_persists_entries_to_disk(tmp_path):
    """Archive entries survive process restarts when storage_path is configured."""
    storage_path = tmp_path / "compaction-archive.json"

    archive = CompactionArchive(max_entries=10, ttl=3600.0, storage_path=storage_path)
    ref_id = archive.archive("Persisted context about JWT refresh tokens", metadata={"events": 4})

    restored = CompactionArchive(max_entries=10, ttl=3600.0, storage_path=storage_path)

    assert restored.retrieve(ref_id) == "Persisted context about JWT refresh tokens"
    results = restored.search("refresh tokens")
    assert len(results) == 1
    assert results[0]["metadata"] == {"events": 4}
