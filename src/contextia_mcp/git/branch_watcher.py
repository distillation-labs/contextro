"""Real-time branch-aware index management for Contextia.

Monitors git HEAD for branch switches and file changes, automatically
triggering incremental reindexing to keep the search index fresh.
This is the "branch-aware, real-time indexing" feature.
"""

import logging
import threading
import time
from typing import Any, Callable, Dict, List, Optional, Set

from contextia_mcp.git.commit_indexer import get_current_branch, get_current_head, is_git_repo

logger = logging.getLogger(__name__)


def _fast_branch(repo_path: str) -> str:
    """Get branch using Rust accelerator if available, else subprocess."""
    try:
        from contextia_mcp.accelerator import RUST_AVAILABLE, git_current_branch_fast
        if RUST_AVAILABLE:
            return git_current_branch_fast(repo_path) or "unknown"
    except ImportError:
        pass
    return get_current_branch(repo_path)


def _fast_head(repo_path: str) -> str:
    """Get HEAD hash using Rust accelerator if available, else subprocess."""
    try:
        from contextia_mcp.accelerator import RUST_AVAILABLE, git_head_hash_fast
        if RUST_AVAILABLE:
            return git_head_hash_fast(repo_path) or ""
    except ImportError:
        pass
    return get_current_head(repo_path)


def _fast_is_repo(path: str) -> bool:
    """Check git repo using Rust accelerator if available."""
    try:
        from contextia_mcp.accelerator import RUST_AVAILABLE, git_is_repo_fast
        if RUST_AVAILABLE:
            return git_is_repo_fast(path)
    except ImportError:
        pass
    return is_git_repo(path)

# Poll interval for HEAD changes (branch switches)
HEAD_POLL_INTERVAL = 2.0
# Debounce delay for file-change-triggered reindex
REINDEX_DEBOUNCE_SECONDS = 3.0


class BranchState:
    """Tracks the current git branch and HEAD state for a repository."""

    def __init__(self, repo_path: str):
        self.repo_path = repo_path
        self.branch = _fast_branch(repo_path)
        self.head = _fast_head(repo_path)
        self.last_index_time: float = 0.0
        self.last_commit_index_time: float = 0.0

    def has_changed(self) -> bool:
        """Check if branch or HEAD has changed since last check."""
        new_branch = _fast_branch(self.repo_path)
        new_head = _fast_head(self.repo_path)

        changed = (new_branch != self.branch) or (new_head != self.head)
        if changed:
            logger.info(
                "Git state changed: %s/%s -> %s/%s",
                self.branch, self.head[:8] if self.head else "?",
                new_branch, new_head[:8] if new_head else "?",
            )
            self.branch = new_branch
            self.head = new_head
        return changed

    def is_branch_switch(self, new_branch: str) -> bool:
        """Check if this represents a branch switch (not just a new commit)."""
        return new_branch != self.branch


class RealtimeIndexManager:
    """Manages real-time, branch-aware indexing for one or more repositories.

    Features:
    - Polls git HEAD to detect branch switches and new commits
    - Triggers incremental reindex on file changes (via file watcher)
    - Triggers full reindex on branch switches
    - Debounces rapid changes to avoid thrashing
    - Thread-safe for concurrent access from MCP tools
    """

    def __init__(
        self,
        reindex_callback: Callable[[str, bool], Any],
        commit_index_callback: Optional[Callable[[str], Any]] = None,
        poll_interval: float = HEAD_POLL_INTERVAL,
        debounce_seconds: float = REINDEX_DEBOUNCE_SECONDS,
    ):
        """
        Args:
            reindex_callback: Called with (repo_path, is_full_reindex) when reindex needed.
            commit_index_callback: Called with (repo_path) to reindex commits after branch switch.
            poll_interval: Seconds between HEAD polls.
            debounce_seconds: Minimum seconds between reindex triggers.
        """
        self._reindex_callback = reindex_callback
        self._commit_index_callback = commit_index_callback
        self._poll_interval = poll_interval
        self._debounce_seconds = debounce_seconds

        self._repos: Dict[str, BranchState] = {}
        self._lock = threading.Lock()
        self._running = False
        self._poll_thread: Optional[threading.Thread] = None
        self._pending_reindex: Set[str] = set()
        self._last_reindex_time: Dict[str, float] = {}

    def add_repo(self, repo_path: str) -> bool:
        """Register a repository for monitoring.

        Returns True if the repo was added, False if not a git repo.
        """
        if not _fast_is_repo(repo_path):
            logger.debug("Not a git repo, skipping watch: %s", repo_path)
            return False

        with self._lock:
            if repo_path not in self._repos:
                self._repos[repo_path] = BranchState(repo_path)
                logger.info(
                    "Watching repo: %s (branch: %s)",
                    repo_path, self._repos[repo_path].branch,
                )
            return True

    def remove_repo(self, repo_path: str) -> None:
        """Stop monitoring a repository."""
        with self._lock:
            self._repos.pop(repo_path, None)

    def start(self) -> None:
        """Start the background HEAD polling thread."""
        if self._running:
            return

        self._running = True
        self._poll_thread = threading.Thread(
            target=self._poll_loop,
            daemon=True,
            name="contextia-branch-watcher",
        )
        self._poll_thread.start()
        logger.info("Branch watcher started (poll interval: %.1fs)", self._poll_interval)

    def stop(self) -> None:
        """Stop the background polling thread."""
        self._running = False
        if self._poll_thread:
            self._poll_thread.join(timeout=5.0)
            self._poll_thread = None
        logger.info("Branch watcher stopped")

    def _poll_loop(self) -> None:
        """Background loop that checks for git state changes."""
        while self._running:
            try:
                self._check_all_repos()
            except Exception as e:
                logger.error("Branch watcher poll error: %s", e)
            time.sleep(self._poll_interval)

    def _check_all_repos(self) -> None:
        """Check all registered repos for changes."""
        with self._lock:
            repos = list(self._repos.items())

        for repo_path, state in repos:
            old_branch = state.branch
            if state.has_changed():
                is_branch_switch = (old_branch != state.branch)
                self._trigger_reindex(repo_path, full=is_branch_switch)

                # Reindex commits on branch switch
                if is_branch_switch and self._commit_index_callback:
                    try:
                        self._commit_index_callback(repo_path)
                    except Exception as e:
                        logger.warning("Commit reindex failed for %s: %s", repo_path, e)

    def _trigger_reindex(self, repo_path: str, full: bool = False) -> None:
        """Trigger a reindex with debouncing."""
        now = time.time()
        last = self._last_reindex_time.get(repo_path, 0)

        if now - last < self._debounce_seconds:
            logger.debug("Debouncing reindex for %s", repo_path)
            return

        self._last_reindex_time[repo_path] = now

        try:
            logger.info(
                "Triggering %s reindex for %s",
                "full" if full else "incremental",
                repo_path,
            )
            self._reindex_callback(repo_path, full)
        except Exception as e:
            logger.error("Reindex callback failed for %s: %s", repo_path, e)

    def notify_file_change(self, repo_path: str) -> None:
        """Called by file watcher when files change. Triggers incremental reindex."""
        self._trigger_reindex(repo_path, full=False)

    def get_status(self) -> Dict[str, Any]:
        """Get the current status of all watched repositories."""
        with self._lock:
            repos_status = {}
            for path, state in self._repos.items():
                repos_status[path] = {
                    "branch": state.branch,
                    "head": state.head[:12] if state.head else None,
                    "last_index_time": state.last_index_time,
                }

        return {
            "running": self._running,
            "repos": repos_status,
            "poll_interval": self._poll_interval,
        }

    @property
    def is_running(self) -> bool:
        return self._running

    @property
    def watched_repos(self) -> List[str]:
        with self._lock:
            return list(self._repos.keys())

    def get_branch(self, repo_path: str) -> Optional[str]:
        """Get the current branch for a watched repo."""
        with self._lock:
            state = self._repos.get(repo_path)
            return state.branch if state else None
