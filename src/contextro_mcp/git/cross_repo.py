"""Cross-repository context management for Contextro.

Manages multiple independent repositories with a unified search surface,
cross-repo graph linking, and shared semantic memory. Enables queries
that span multiple codebases — e.g., "how does service A call service B?"
"""

import logging
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

logger = logging.getLogger(__name__)


@dataclass
class RepoContext:
    """Metadata and state for a single registered repository."""

    path: str
    name: str  # Short name (e.g., "backend", "frontend")
    branch: str = "unknown"
    head: str = ""
    indexed: bool = False
    last_indexed_at: float = 0.0
    total_files: int = 0
    total_symbols: int = 0
    total_chunks: int = 0
    total_commits: int = 0
    languages: Dict[str, int] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "path": self.path,
            "name": self.name,
            "branch": self.branch,
            "head": self.head[:12] if self.head else "",
            "indexed": self.indexed,
            "last_indexed_at": self.last_indexed_at,
            "total_files": self.total_files,
            "total_symbols": self.total_symbols,
            "total_chunks": self.total_chunks,
            "total_commits": self.total_commits,
            "languages": self.languages,
        }


class CrossRepoManager:
    """Manages multiple repositories for unified code intelligence.

    Features:
    - Register/unregister repositories by path
    - Track per-repo indexing state (branch, HEAD, stats)
    - Provide unified search across all repos
    - Detect cross-repo dependencies (shared imports, API calls)
    - Maintain per-repo commit history
    """

    def __init__(self):
        self._repos: Dict[str, RepoContext] = {}  # path -> context

    def register_repo(
        self,
        path: str,
        name: Optional[str] = None,
    ) -> RepoContext:
        """Register a repository for cross-repo context.

        Args:
            path: Absolute path to the repository root.
            name: Short name for the repo. Defaults to directory name.

        Returns:
            The RepoContext for the registered repo.
        """
        resolved = str(Path(path).resolve())

        if resolved in self._repos:
            return self._repos[resolved]

        if name is None:
            name = Path(resolved).name

        # Detect git info
        from contextro_mcp.git.commit_indexer import (
            get_current_branch,
            get_current_head,
            is_git_repo,
        )

        branch = "unknown"
        head = ""
        if is_git_repo(resolved):
            branch = get_current_branch(resolved)
            head = get_current_head(resolved)

        ctx = RepoContext(
            path=resolved,
            name=name,
            branch=branch,
            head=head,
        )
        self._repos[resolved] = ctx
        logger.info("Registered repo: %s (%s, branch: %s)", name, resolved, branch)
        return ctx

    def unregister_repo(self, path: str) -> bool:
        """Remove a repository from cross-repo context."""
        resolved = str(Path(path).resolve())
        if resolved in self._repos:
            del self._repos[resolved]
            logger.info("Unregistered repo: %s", resolved)
            return True
        return False

    def get_repo(self, path: str) -> Optional[RepoContext]:
        """Get context for a specific repo."""
        resolved = str(Path(path).resolve())
        return self._repos.get(resolved)

    def get_repo_by_name(self, name: str) -> Optional[RepoContext]:
        """Find a repo by its short name."""
        for ctx in self._repos.values():
            if ctx.name == name:
                return ctx
        return None

    def update_repo_stats(
        self,
        path: str,
        total_files: int = 0,
        total_symbols: int = 0,
        total_chunks: int = 0,
        total_commits: int = 0,
        languages: Optional[Dict[str, int]] = None,
    ) -> None:
        """Update indexing statistics for a repo."""
        resolved = str(Path(path).resolve())
        ctx = self._repos.get(resolved)
        if ctx:
            ctx.indexed = True
            ctx.last_indexed_at = time.time()
            ctx.total_files = total_files
            ctx.total_symbols = total_symbols
            ctx.total_chunks = total_chunks
            if total_commits:
                ctx.total_commits = total_commits
            if languages:
                ctx.languages = languages

    def update_branch(self, path: str, branch: str, head: str) -> None:
        """Update branch/HEAD info for a repo."""
        resolved = str(Path(path).resolve())
        ctx = self._repos.get(resolved)
        if ctx:
            ctx.branch = branch
            ctx.head = head

    @property
    def repos(self) -> List[RepoContext]:
        """Get all registered repositories."""
        return list(self._repos.values())

    @property
    def repo_paths(self) -> List[str]:
        """Get all registered repository paths."""
        return list(self._repos.keys())

    @property
    def repo_count(self) -> int:
        return len(self._repos)

    def get_all_status(self) -> Dict[str, Any]:
        """Get status of all registered repositories."""
        return {
            "total_repos": len(self._repos),
            "repos": [ctx.to_dict() for ctx in self._repos.values()],
            "total_files": sum(ctx.total_files for ctx in self._repos.values()),
            "total_symbols": sum(ctx.total_symbols for ctx in self._repos.values()),
            "total_chunks": sum(ctx.total_chunks for ctx in self._repos.values()),
            "total_commits": sum(ctx.total_commits for ctx in self._repos.values()),
        }

    def find_repo_for_file(self, filepath: str) -> Optional[RepoContext]:
        """Find which repo a file belongs to."""
        resolved = str(Path(filepath).resolve())
        for path, ctx in self._repos.items():
            if resolved.startswith(path):
                return ctx
        return None

    def get_cross_repo_summary(self) -> Dict[str, Any]:
        """Generate a summary of cross-repo relationships.

        Identifies shared languages, potential API boundaries,
        and inter-repo dependency patterns.
        """
        all_languages: Dict[str, int] = {}
        for ctx in self._repos.values():
            for lang, count in ctx.languages.items():
                all_languages[lang] = all_languages.get(lang, 0) + count

        return {
            "total_repos": len(self._repos),
            "repos": {ctx.name: ctx.to_dict() for ctx in self._repos.values()},
            "shared_languages": all_languages,
            "total_files_across_repos": sum(c.total_files for c in self._repos.values()),
            "total_symbols_across_repos": sum(c.total_symbols for c in self._repos.values()),
        }
