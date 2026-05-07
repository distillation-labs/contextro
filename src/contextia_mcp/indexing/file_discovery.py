"""File discovery helpers for indexing flows."""

from __future__ import annotations

import logging
import os
from pathlib import Path

from contextia_mcp.config import Settings
from contextia_mcp.parsing.language_registry import get_supported_extensions

logger = logging.getLogger(__name__)

SKIP_DIRS: set[str] = {
    ".git",
    "node_modules",
    "__pycache__",
    ".contextia",
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
    "storage",
}


def discover_files(root: Path, settings: Settings) -> list[Path]:
    """Discover source files under root, respecting .gitignore and size limits."""
    from contextia_mcp.accelerator import RUST_AVAILABLE, discover_files_fast

    supported_extensions = get_supported_extensions()
    max_size = settings.max_file_size_mb * 1024 * 1024
    root = root.resolve()

    if RUST_AVAILABLE:
        # The Rust walker handles .gitignore correctly inside git repos. For plain
        # directories with only a loose .gitignore file, keep the Python fallback.
        gitignore_exists = (root / ".gitignore").exists()
        is_git = (root / ".git").is_dir()

        if not gitignore_exists or is_git:
            file_strs = discover_files_fast(
                str(root),
                extensions=supported_extensions,
                max_file_size_bytes=max_size,
                skip_dirs=SKIP_DIRS,
            )
            return [Path(path) for path in file_strs]

    gitignore_spec = None
    gitignore_path = root / ".gitignore"
    if gitignore_path.exists():
        try:
            import pathspec

            patterns = gitignore_path.read_text().splitlines()
            gitignore_spec = pathspec.PathSpec.from_lines("gitignore", patterns)
        except ImportError:
            logger.debug("pathspec not installed, skipping .gitignore support")
        except Exception as exc:
            logger.warning("Failed to parse .gitignore: %s", exc)

    files: list[Path] = []

    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            name
            for name in dirnames
            if name not in SKIP_DIRS and not name.startswith(".")
        ]

        for filename in filenames:
            filepath = Path(dirpath) / filename
            if filepath.suffix.lower() not in supported_extensions:
                continue

            if gitignore_spec:
                rel = filepath.relative_to(root)
                if gitignore_spec.match_file(str(rel)):
                    continue

            try:
                if filepath.stat().st_size > max_size:
                    continue
            except OSError:
                continue

            files.append(filepath)

    return sorted(files)
