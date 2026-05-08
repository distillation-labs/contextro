"""Tests for the Rust-accelerated operations module."""

import subprocess
from pathlib import Path

import pytest

from contextro_mcp.accelerator import (
    RUST_AVAILABLE,
    diff_mtimes_fast,
    discover_files_fast,
    git_current_branch_fast,
    git_head_hash_fast,
    git_is_repo_fast,
    hash_files_fast,
    scan_mtimes_fast,
)


@pytest.fixture
def git_repo(tmp_path):
    """Create a temporary git repository."""
    repo = tmp_path / "repo"
    repo.mkdir()
    subprocess.run(["git", "init"], cwd=str(repo), capture_output=True, check=True)
    subprocess.run(
        ["git", "config", "user.email", "t@t.com"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "T"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    (repo / "main.py").write_text("x = 1\n")
    (repo / "utils.py").write_text("y = 2\n")
    subprocess.run(["git", "add", "."], cwd=str(repo), capture_output=True, check=True)
    subprocess.run(
        ["git", "commit", "-m", "init"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    return repo


class TestDiscoverFiles:
    """Tests for discover_files_fast."""

    def test_discovers_python_files(self, tmp_path):
        (tmp_path / "a.py").write_text("x=1")
        (tmp_path / "b.py").write_text("y=2")
        (tmp_path / "c.txt").write_text("text")

        files = discover_files_fast(str(tmp_path), extensions={".py"})
        names = [Path(f).name for f in files]
        assert "a.py" in names
        assert "b.py" in names
        assert "c.txt" not in names

    def test_respects_max_size(self, tmp_path):
        (tmp_path / "small.py").write_text("x=1")
        (tmp_path / "big.py").write_text("x" * 1000)

        files = discover_files_fast(str(tmp_path), extensions={".py"}, max_file_size_bytes=500)
        names = [Path(f).name for f in files]
        assert "small.py" in names
        assert "big.py" not in names

    def test_skips_directories(self, tmp_path):
        (tmp_path / "good.py").write_text("x=1")
        node_modules = tmp_path / "node_modules"
        node_modules.mkdir()
        (node_modules / "bad.py").write_text("y=2")

        files = discover_files_fast(
            str(tmp_path),
            extensions={".py"},
            skip_dirs={"node_modules"},
        )
        names = [Path(f).name for f in files]
        assert "good.py" in names
        assert "bad.py" not in names

    def test_returns_sorted(self, tmp_path):
        (tmp_path / "z.py").write_text("z")
        (tmp_path / "a.py").write_text("a")
        (tmp_path / "m.py").write_text("m")

        files = discover_files_fast(str(tmp_path), extensions={".py"})
        names = [Path(f).name for f in files]
        assert names == sorted(names)


class TestScanMtimes:
    """Tests for scan_mtimes_fast."""

    def test_scans_existing_files(self, tmp_path):
        (tmp_path / "a.py").write_text("x")
        (tmp_path / "b.py").write_text("y")
        paths = [str(tmp_path / "a.py"), str(tmp_path / "b.py")]

        mtimes = scan_mtimes_fast(paths)
        assert len(mtimes) == 2
        for path in paths:
            assert path in mtimes
            assert mtimes[path] > 0

    def test_skips_missing_files(self, tmp_path):
        (tmp_path / "exists.py").write_text("x")
        paths = [str(tmp_path / "exists.py"), str(tmp_path / "missing.py")]

        mtimes = scan_mtimes_fast(paths)
        assert str(tmp_path / "exists.py") in mtimes
        assert str(tmp_path / "missing.py") not in mtimes


class TestDiffMtimes:
    """Tests for diff_mtimes_fast."""

    def test_detects_added(self):
        current = {"a.py": 1.0, "b.py": 2.0}
        stored = {"a.py": 1.0}
        added, modified, deleted = diff_mtimes_fast(current, stored)
        assert "b.py" in added
        assert modified == []
        assert deleted == []

    def test_detects_modified(self):
        current = {"a.py": 2.0}
        stored = {"a.py": 1.0}
        added, modified, deleted = diff_mtimes_fast(current, stored)
        assert added == []
        assert "a.py" in modified
        assert deleted == []

    def test_detects_deleted(self):
        current = {}
        stored = {"a.py": 1.0}
        added, modified, deleted = diff_mtimes_fast(current, stored)
        assert added == []
        assert modified == []
        assert "a.py" in deleted

    def test_combined_changes(self):
        current = {"a.py": 1.0, "b.py": 3.0, "new.py": 1.0}
        stored = {"a.py": 1.0, "b.py": 2.0, "old.py": 1.0}
        added, modified, deleted = diff_mtimes_fast(current, stored)
        assert "new.py" in added
        assert "b.py" in modified
        assert "old.py" in deleted


class TestHashFiles:
    """Tests for hash_files_fast."""

    def test_hashes_files(self, tmp_path):
        (tmp_path / "a.py").write_text("content_a")
        (tmp_path / "b.py").write_text("content_b")
        paths = [str(tmp_path / "a.py"), str(tmp_path / "b.py")]

        hashes = hash_files_fast(paths)
        assert len(hashes) == 2
        assert hashes[paths[0]] != hashes[paths[1]]

    def test_same_content_same_hash(self, tmp_path):
        (tmp_path / "a.py").write_text("same")
        (tmp_path / "b.py").write_text("same")
        paths = [str(tmp_path / "a.py"), str(tmp_path / "b.py")]

        hashes = hash_files_fast(paths)
        assert hashes[paths[0]] == hashes[paths[1]]

    def test_skips_missing(self, tmp_path):
        (tmp_path / "exists.py").write_text("x")
        paths = [str(tmp_path / "exists.py"), "/nonexistent/file.py"]

        hashes = hash_files_fast(paths)
        assert str(tmp_path / "exists.py") in hashes
        assert "/nonexistent/file.py" not in hashes


class TestGitOps:
    """Tests for git operation wrappers."""

    def test_is_repo(self, git_repo):
        assert git_is_repo_fast(str(git_repo)) is True

    def test_is_not_repo(self, tmp_path):
        plain = tmp_path / "plain"
        plain.mkdir()
        assert git_is_repo_fast(str(plain)) is False

    def test_current_branch(self, git_repo):
        branch = git_current_branch_fast(str(git_repo))
        assert branch in ("main", "master")

    def test_head_hash(self, git_repo):
        head = git_head_hash_fast(str(git_repo))
        assert head is not None
        assert len(head) == 40


class TestRustAvailability:
    """Test that the module reports Rust availability correctly."""

    def test_rust_available_flag(self):
        # This test just verifies the flag is a boolean
        assert isinstance(RUST_AVAILABLE, bool)
        # If we got here, the module loaded successfully
        print(f"RUST_AVAILABLE = {RUST_AVAILABLE}")
