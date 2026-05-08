"""Rust-accelerated operations for Contextro.

Provides a Python fallback layer that uses the `ctx_fast` Rust extension
when available, falling back to pure Python implementations otherwise.
This ensures the package works without the Rust extension installed.
"""

import logging
import os
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

logger = logging.getLogger(__name__)
FileStateValue = str | float

# Try to import the Rust extension
try:
    from contextro_mcp import ctx_fast

    RUST_AVAILABLE = True
    logger.debug("ctx_fast Rust extension loaded")
except ImportError:
    try:
        import ctx_fast  # type: ignore[no-redef]

        RUST_AVAILABLE = True
        logger.debug("ctx_fast Rust extension loaded")
    except ImportError:
        RUST_AVAILABLE = False
        logger.debug("ctx_fast not available, using Python fallback")


def discover_files_fast(
    root: str,
    extensions: Optional[Set[str]] = None,
    max_file_size_bytes: int = 10 * 1024 * 1024,
    skip_dirs: Optional[Set[str]] = None,
) -> List[str]:
    """Discover source files using Rust parallel walker or Python fallback.

    When ctx_fast is available, uses the `ignore` crate (same as ripgrep)
    for .gitignore-aware parallel directory walking. Falls back to Python
    os.walk + pathspec otherwise.

    Args:
        root: Root directory to scan.
        extensions: Set of file extensions to include (e.g. {'.py', '.js'}).
        max_file_size_bytes: Skip files larger than this.
        skip_dirs: Additional directory names to skip.

    Returns:
        Sorted list of absolute file paths.
    """
    if RUST_AVAILABLE:
        ext_list = None
        if extensions:
            ext_list = [e.lstrip(".") for e in extensions]
        skip_list = list(skip_dirs) if skip_dirs else None

        try:
            return ctx_fast.discover_files(
                root,
                extensions=ext_list,
                max_file_size_bytes=max_file_size_bytes,
                skip_dirs=skip_list,
            )
        except Exception as e:
            logger.warning("Rust discover_files failed, falling back to Python: %s", e)

    # Python fallback
    return _discover_files_python(root, extensions, max_file_size_bytes, skip_dirs)


def scan_mtimes_fast(paths: List[str]) -> Dict[str, float]:
    """Scan file modification times in parallel.

    Uses Rust rayon parallelism when available, sequential Python otherwise.

    Args:
        paths: List of file paths to stat.

    Returns:
        Dict mapping filepath to mtime (seconds since epoch).
    """
    if RUST_AVAILABLE:
        try:
            return ctx_fast.scan_mtimes(paths)
        except Exception as e:
            logger.warning("Rust scan_mtimes failed, falling back to Python: %s", e)

    # Python fallback
    result = {}
    for path in paths:
        try:
            result[path] = os.stat(path).st_mtime
        except OSError:
            continue
    return result


def _stat_signature(mtime_seconds: float, size_bytes: int, ctime_ns: int) -> str:
    """Build a stable signature from cheap file metadata."""
    mtime_ns = int(round(mtime_seconds * 1_000_000_000))
    return f"{mtime_ns}:{size_bytes}:{ctime_ns}"


def scan_file_stats_fast(paths: List[str]) -> Dict[str, str]:
    """Scan cheap file-stat signatures in parallel.

    Uses the Rust `stat_files()` helper when available for mtime/size, then
    augments with ctime in Python so preserved-mtime edits still invalidate the
    signature. Falls back to pure Python os.stat when the extension is absent.
    """
    if RUST_AVAILABLE and hasattr(ctx_fast, "stat_files"):
        try:
            rust_stats = {
                path: (mtime, size) for path, mtime, size in ctx_fast.stat_files(paths)
            }
            result: Dict[str, str] = {}
            for path, (mtime, size) in rust_stats.items():
                try:
                    stat = os.stat(path)
                except OSError:
                    continue
                result[path] = _stat_signature(mtime, size, stat.st_ctime_ns)
            return result
        except Exception as e:
            logger.warning("Rust stat_files failed, falling back to Python: %s", e)

    result: Dict[str, str] = {}
    for path in paths:
        try:
            stat = os.stat(path)
        except OSError:
            continue
        result[path] = _stat_signature(stat.st_mtime, stat.st_size, stat.st_ctime_ns)
    return result


def _is_numeric_state_map(file_state: Dict[str, FileStateValue]) -> bool:
    """Return True when every value is a real numeric mtime."""
    return all(
        isinstance(value, (int, float)) and not isinstance(value, bool)
        for value in file_state.values()
    )


def diff_mtimes_fast(
    current: Dict[str, FileStateValue],
    stored: Dict[str, FileStateValue],
) -> Tuple[List[str], List[str], List[str]]:
    """Diff two file-state maps to find added, modified, and deleted files.

    Uses Rust when available for large numeric mtime maps. Content-hash maps
    stay on the Python path because the Rust diff helper currently accepts
    floats only.

    Returns:
        Tuple of (added, modified, deleted) file path lists.
    """
    if (
        RUST_AVAILABLE
        and len(current) + len(stored) > 100
        and _is_numeric_state_map(current)
        and _is_numeric_state_map(stored)
    ):
        try:
            return ctx_fast.diff_mtimes(current, stored)
        except Exception as e:
            logger.warning("Rust diff_mtimes failed, falling back to Python: %s", e)

    # Python fallback
    current_set = set(current.keys())
    stored_set = set(stored.keys())

    added = list(current_set - stored_set)
    deleted = list(stored_set - current_set)

    # Handle both string hashes and float mtimes in comparison
    modified = []
    for f in current_set & stored_set:
        curr_val = current[f]
        stored_val = stored.get(f)
        if stored_val is None:
            continue
        # If both are strings (hashes), compare directly
        if isinstance(curr_val, str) and isinstance(stored_val, str):
            if curr_val != stored_val:
                modified.append(f)
        # If both are numbers (mtimes), compare with epsilon
        elif isinstance(curr_val, (int, float)) and isinstance(stored_val, (int, float)):
            if abs(curr_val - stored_val) > 0.001:
                modified.append(f)
        # Mixed types - always consider modified
        else:
            modified.append(f)

    return added, modified, deleted


def hash_files_fast(paths: List[str]) -> Dict[str, str]:
    """Hash file contents in parallel using xxHash3.

    xxHash3 is ~10x faster than SHA-256 and suitable for change detection.

    Args:
        paths: List of file paths to hash.

    Returns:
        Dict mapping filepath to hex hash string.
    """
    if RUST_AVAILABLE:
        try:
            return ctx_fast.hash_files(paths)
        except Exception as e:
            logger.warning("Rust hash_files failed, falling back to Python: %s", e)

    # Python fallback using hashlib
    import hashlib

    result = {}
    for path in paths:
        try:
            with open(path, "rb") as f:
                content = f.read()
            result[path] = hashlib.sha256(content).hexdigest()[:16]
        except OSError:
            continue
    return result


def git_current_branch_fast(repo_path: str) -> Optional[str]:
    """Get current git branch using Rust or subprocess fallback."""
    if RUST_AVAILABLE:
        try:
            return ctx_fast.git_current_branch(repo_path)
        except Exception:
            pass

    from contextro_mcp.git.commit_indexer import get_current_branch

    return get_current_branch(repo_path)


def git_head_hash_fast(repo_path: str) -> Optional[str]:
    """Get current HEAD hash using Rust or subprocess fallback."""
    if RUST_AVAILABLE:
        try:
            return ctx_fast.git_head_hash(repo_path)
        except Exception:
            pass

    from contextro_mcp.git.commit_indexer import get_current_head

    return get_current_head(repo_path) or None


def git_is_repo_fast(path: str) -> bool:
    """Check if path is a git repo using Rust or subprocess fallback."""
    if RUST_AVAILABLE:
        try:
            return ctx_fast.git_is_repo(path)
        except Exception:
            pass

    from contextro_mcp.git.commit_indexer import is_git_repo

    return is_git_repo(path)


def git_changed_files_fast(
    repo_path: str,
    from_commit: Optional[str] = None,
    to_commit: Optional[str] = None,
) -> List[str]:
    """Get files changed between commits using Rust or subprocess fallback."""
    if RUST_AVAILABLE:
        try:
            return ctx_fast.git_changed_files(
                repo_path,
                from_commit=from_commit,
                to_commit=to_commit,
            )
        except Exception:
            pass
    return []


def git_status_fast(repo_path: str) -> List[str]:
    """Get uncommitted file changes using Rust or subprocess fallback."""
    if RUST_AVAILABLE:
        try:
            return ctx_fast.git_status(repo_path)
        except Exception:
            pass
    return []


# --- Python fallback for discover_files ---

_DEFAULT_SKIP_DIRS = {
    ".git",
    "node_modules",
    "__pycache__",
    ".contextro",
    "venv",
    ".venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    "dist",
    "build",
    ".eggs",
    ".ruff_cache",
    ".hg",
    ".svn",
    "target",
    ".cargo",
}


def _discover_files_python(
    root: str,
    extensions: Optional[Set[str]],
    max_file_size_bytes: int,
    skip_dirs: Optional[Set[str]],
) -> List[str]:
    """Pure Python file discovery with .gitignore support."""
    root_path = Path(root).resolve()
    all_skip = _DEFAULT_SKIP_DIRS | (skip_dirs or set())

    # Load .gitignore
    gitignore_spec = None
    gitignore_path = root_path / ".gitignore"
    if gitignore_path.exists():
        try:
            import pathspec

            patterns = gitignore_path.read_text().splitlines()
            gitignore_spec = pathspec.PathSpec.from_lines("gitignore", patterns)
        except (ImportError, Exception):
            pass

    files = []
    for dirpath, dirnames, filenames in os.walk(root_path):
        dirnames[:] = [d for d in dirnames if d not in all_skip and not d.startswith(".")]
        for filename in filenames:
            filepath = Path(dirpath) / filename
            if extensions:
                if filepath.suffix.lower() not in extensions:
                    continue
            if gitignore_spec:
                try:
                    rel = filepath.relative_to(root_path)
                    if gitignore_spec.match_file(str(rel)):
                        continue
                except ValueError:
                    pass
            try:
                if filepath.stat().st_size > max_file_size_bytes:
                    continue
            except OSError:
                continue
            files.append(str(filepath))

    return sorted(files)
