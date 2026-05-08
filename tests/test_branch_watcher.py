"""Tests for branch-aware real-time index management."""

import subprocess
import time
from unittest.mock import MagicMock

import pytest
from contextro_mcp.git.branch_watcher import (
    BranchState,
    RealtimeIndexManager,
)


@pytest.fixture
def git_repo(tmp_path):
    """Create a temporary git repository."""
    repo = tmp_path / "test_repo"
    repo.mkdir()
    subprocess.run(
        ["git", "init"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.email", "test@test.com"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Test User"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    (repo / "main.py").write_text("print('hello')\n")
    subprocess.run(
        ["git", "add", "."],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Initial commit"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    return repo


class TestBranchState:
    """Tests for BranchState tracking."""

    def test_initial_state(self, git_repo):
        state = BranchState(str(git_repo))
        assert state.branch in ("main", "master")
        assert len(state.head) == 40
        assert state.repo_path == str(git_repo)

    def test_no_change_detected(self, git_repo):
        state = BranchState(str(git_repo))
        assert state.has_changed() is False

    def test_change_detected_on_new_commit(self, git_repo):
        state = BranchState(str(git_repo))

        # Make a new commit
        (git_repo / "new.py").write_text("x = 1\n")
        subprocess.run(
            ["git", "add", "."],
            cwd=str(git_repo),
            capture_output=True,
            check=True,
        )
        subprocess.run(
            ["git", "commit", "-m", "New commit"],
            cwd=str(git_repo),
            capture_output=True,
            check=True,
        )

        assert state.has_changed() is True
        # Second call should return False (state updated)
        assert state.has_changed() is False

    def test_branch_switch_detected(self, git_repo):
        state = BranchState(str(git_repo))
        old_branch = state.branch

        # Create and switch to new branch
        subprocess.run(
            ["git", "checkout", "-b", "feature-branch"],
            cwd=str(git_repo),
            capture_output=True,
            check=True,
        )

        assert state.has_changed() is True
        assert state.branch == "feature-branch"

        # Switch back
        subprocess.run(
            ["git", "checkout", old_branch],
            cwd=str(git_repo),
            capture_output=True,
            check=True,
        )
        assert state.has_changed() is True
        assert state.branch == old_branch


class TestRealtimeIndexManager:
    """Tests for the real-time index manager."""

    def test_add_repo(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(reindex_callback=callback)
        assert manager.add_repo(str(git_repo)) is True
        assert str(git_repo) in manager.watched_repos

    def test_add_nonrepo(self, tmp_path):
        callback = MagicMock()
        manager = RealtimeIndexManager(reindex_callback=callback)
        assert manager.add_repo(str(tmp_path)) is False

    def test_remove_repo(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(reindex_callback=callback)
        manager.add_repo(str(git_repo))
        manager.remove_repo(str(git_repo))
        assert str(git_repo) not in manager.watched_repos

    def test_start_stop(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(
            reindex_callback=callback,
            poll_interval=0.1,
        )
        manager.add_repo(str(git_repo))
        manager.start()
        assert manager.is_running is True
        time.sleep(0.3)
        manager.stop()
        assert manager.is_running is False

    def test_get_status(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(reindex_callback=callback)
        manager.add_repo(str(git_repo))
        status = manager.get_status()
        assert "running" in status
        assert "repos" in status
        assert str(git_repo) in status["repos"]

    def test_get_branch(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(reindex_callback=callback)
        manager.add_repo(str(git_repo))
        branch = manager.get_branch(str(git_repo))
        assert branch in ("main", "master")

    def test_get_branch_unknown_repo(self):
        callback = MagicMock()
        manager = RealtimeIndexManager(reindex_callback=callback)
        assert manager.get_branch("/nonexistent") is None

    def test_notify_file_change_triggers_reindex(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(
            reindex_callback=callback,
            debounce_seconds=0.0,  # No debounce for testing
        )
        manager.add_repo(str(git_repo))
        manager.notify_file_change(str(git_repo))
        callback.assert_called_once_with(str(git_repo), False)

    def test_debounce_prevents_rapid_reindex(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(
            reindex_callback=callback,
            debounce_seconds=10.0,  # Long debounce
        )
        manager.add_repo(str(git_repo))
        manager.notify_file_change(str(git_repo))
        manager.notify_file_change(str(git_repo))
        manager.notify_file_change(str(git_repo))
        # Only first call should go through
        assert callback.call_count == 1

    def test_detects_new_commit_and_reindexes(self, git_repo):
        callback = MagicMock()
        manager = RealtimeIndexManager(
            reindex_callback=callback,
            poll_interval=0.1,
            debounce_seconds=0.0,
        )
        manager.add_repo(str(git_repo))
        manager.start()

        # Make a new commit
        (git_repo / "new.py").write_text("x = 1\n")
        subprocess.run(
            ["git", "add", "."],
            cwd=str(git_repo),
            capture_output=True,
            check=True,
        )
        subprocess.run(
            ["git", "commit", "-m", "Trigger reindex"],
            cwd=str(git_repo),
            capture_output=True,
            check=True,
        )

        # Wait for poll to detect
        time.sleep(0.5)
        manager.stop()

        # Should have triggered at least one reindex
        assert callback.call_count >= 1
