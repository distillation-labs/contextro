"""Git commit history indexer for Contextro.

Extracts commit metadata (hash, author, message, changed files, diffs)
from git repositories and stores them as searchable chunks in LanceDB.
Provides semantic search over commit history — the "Context Lineage" feature.
"""

import hashlib
import logging
import subprocess
import time
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

logger = logging.getLogger(__name__)

# Maximum diff size per commit to keep index compact
MAX_DIFF_CHARS = 4000
# Default number of commits to index
DEFAULT_COMMIT_LIMIT = 500


def _ctx_fast() -> Any | None:
    try:
        from contextro_mcp.accelerator import RUST_AVAILABLE, ctx_fast

        if RUST_AVAILABLE:
            return ctx_fast
    except Exception:
        return None
    return None


@dataclass
class CommitInfo:
    """Parsed git commit metadata."""

    hash: str
    short_hash: str
    author_name: str
    author_email: str
    timestamp: str  # ISO 8601
    message: str
    files_changed: List[str] = field(default_factory=list)
    insertions: int = 0
    deletions: int = 0
    diff_summary: str = ""
    branch: str = ""
    repo_path: str = ""

    def to_dict(self) -> Dict[str, Any]:
        return {
            "hash": self.hash,
            "short_hash": self.short_hash,
            "author_name": self.author_name,
            "author_email": self.author_email,
            "timestamp": self.timestamp,
            "message": self.message,
            "files_changed": self.files_changed,
            "insertions": self.insertions,
            "deletions": self.deletions,
            "diff_summary": self.diff_summary,
            "branch": self.branch,
            "repo_path": self.repo_path,
        }


@dataclass
class CommitChunk:
    """A commit prepared for embedding and LanceDB storage."""

    id: str
    text: str
    commit_hash: str
    short_hash: str
    author: str
    timestamp: str
    message: str
    files_changed: str  # comma-separated
    branch: str
    repo_path: str
    insertions: int
    deletions: int
    vector: List[float] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "text": self.text,
            "vector": self.vector,
            "commit_hash": self.commit_hash,
            "short_hash": self.short_hash,
            "author": self.author,
            "timestamp": self.timestamp,
            "message": self.message,
            "files_changed": self.files_changed,
            "branch": self.branch,
            "repo_path": self.repo_path,
            "insertions": self.insertions,
            "deletions": self.deletions,
        }


def _run_git(args: List[str], cwd: str, timeout: int = 30) -> Optional[str]:
    """Run a git command and return stdout, or None on failure."""
    try:
        result = subprocess.run(
            ["git"] + args,
            cwd=cwd,
            capture_output=True,
            text=False,  # Handle encoding manually to avoid binary data crashes
            timeout=timeout,
        )
        if result.returncode != 0:
            # Signal-killed (e.g. SIGBUS on ARM64 Docker) — return partial output if available
            if result.returncode < 0 and result.stdout:
                logger.debug(
                    "git %s killed by signal %d, using partial output (%d bytes)",
                    " ".join(args),
                    -result.returncode,
                    len(result.stdout),
                )
                return result.stdout.decode("utf-8", errors="replace")
            stderr = result.stderr.decode("utf-8", errors="replace").strip()
            logger.debug("git %s failed: %s", " ".join(args), stderr)
            return None
        return result.stdout.decode("utf-8", errors="replace")
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError) as e:
        logger.debug("git command failed: %s", e)
        return None


def get_current_branch(repo_path: str) -> str:
    """Get the current git branch name."""
    ctx = _ctx_fast()
    if ctx is not None:
        try:
            branch = ctx.git_current_branch(repo_path)
            if branch:
                return branch
        except Exception:
            pass
    output = _run_git(["rev-parse", "--abbrev-ref", "HEAD"], cwd=repo_path)
    if output:
        return output.strip()
    return "unknown"


def get_current_head(repo_path: str) -> str:
    """Get the current HEAD commit hash."""
    ctx = _ctx_fast()
    if ctx is not None:
        try:
            head = ctx.git_head_hash(repo_path)
            if head:
                return head
        except Exception:
            pass
    output = _run_git(["rev-parse", "HEAD"], cwd=repo_path)
    if output:
        return output.strip()
    return ""


def is_git_repo(path: str) -> bool:
    """Check if a path is inside a git repository."""
    ctx = _ctx_fast()
    if ctx is not None:
        try:
            return bool(ctx.git_is_repo(path))
        except Exception:
            pass
    output = _run_git(["rev-parse", "--is-inside-work-tree"], cwd=path)
    return output is not None and output.strip() == "true"


def get_repo_root(path: str) -> Optional[str]:
    """Get the root directory of the git repository containing path."""
    output = _run_git(["rev-parse", "--show-toplevel"], cwd=path)
    if output:
        return output.strip()
    return None


def extract_commits(
    repo_path: str,
    limit: int = DEFAULT_COMMIT_LIMIT,
    since: Optional[str] = None,
    branch: Optional[str] = None,
) -> List[CommitInfo]:
    """Extract commit history from a git repository.

    Args:
        repo_path: Path to the git repository.
        limit: Maximum number of commits to extract.
        since: Only commits after this date (ISO 8601 or relative like '3 months ago').
        branch: Branch to extract from (default: current branch).

    Returns:
        List of CommitInfo objects, newest first.
    """
    if not is_git_repo(repo_path):
        logger.warning("Not a git repository: %s", repo_path)
        return []

    current_branch = branch or get_current_branch(repo_path)

    # Use a record separator that won't appear in commit data
    record_sep = "---CTX_REC---"
    field_sep = "---CTX_FLD---"
    # Format: hash, short_hash, author_name, author_email, timestamp, subject
    format_str = (
        f"{record_sep}%H{field_sep}%h{field_sep}%an{field_sep}%ae{field_sep}%aI{field_sep}%s"
    )

    args = [
        "log",
        f"--format={format_str}",
        f"-n{limit}",
        "--name-only",
    ]
    if since:
        args.append(f"--since={since}")

    output = _run_git(args, cwd=repo_path, timeout=60)
    if not output:
        return []

    commits: List[CommitInfo] = []

    # Split by record separator to get individual commit blocks
    # Each block starts with the header line, followed by file name lines
    blocks = output.split(record_sep)

    for block in blocks:
        block = block.strip()
        if not block:
            continue

        lines = block.split("\n")
        if not lines:
            continue

        # First line contains the formatted commit info
        header = lines[0].strip()
        parts = header.split(field_sep)
        if len(parts) < 6:
            continue

        commit_hash = parts[0]
        short_hash = parts[1]
        author_name = parts[2]
        author_email = parts[3]
        timestamp = parts[4]
        message = parts[5]

        # Parse file names (lines after the header)
        files_changed = []
        for line in lines[1:]:
            line = line.strip()
            if line:
                files_changed.append(line)

        commits.append(
            CommitInfo(
                hash=commit_hash,
                short_hash=short_hash,
                author_name=author_name,
                author_email=author_email,
                timestamp=timestamp,
                message=message,
                files_changed=files_changed,
                insertions=0,
                deletions=0,
                branch=current_branch,
                repo_path=repo_path,
            )
        )

    return commits


def get_commit_diff_summary(repo_path: str, commit_hash: str) -> str:
    """Get a compact diff summary for a single commit.

    Returns a truncated stat + diff output suitable for embedding.
    """
    # Get stat summary first
    stat_output = _run_git(
        ["show", "--stat", "--format=", commit_hash],
        cwd=repo_path,
        timeout=15,
    )

    # Get abbreviated diff (context lines reduced)
    diff_output = _run_git(
        ["show", "--format=", "-U1", "--no-color", commit_hash],
        cwd=repo_path,
        timeout=15,
    )

    parts = []
    if stat_output:
        parts.append(stat_output.strip()[:1000])
    if diff_output:
        # Truncate diff to keep chunks manageable
        parts.append(diff_output.strip()[:MAX_DIFF_CHARS])

    return "\n".join(parts)


def create_commit_chunk(commit: CommitInfo, diff_summary: str = "") -> CommitChunk:
    """Convert a CommitInfo into an embeddable CommitChunk.

    The text field is formatted for semantic search:
    - Commit message (most important for intent matching)
    - Files changed (for file-based queries)
    - Author and date (for temporal queries)
    - Diff summary (for code-level queries)
    """
    chunk_id = hashlib.sha256(f"commit:{commit.hash}:{commit.repo_path}".encode()).hexdigest()[:16]

    parts = [
        f"Commit: {commit.message}",
        f"Author: {commit.author_name} <{commit.author_email}>",
        f"Date: {commit.timestamp}",
        f"Branch: {commit.branch}",
        f"Changes: +{commit.insertions} -{commit.deletions} in {len(commit.files_changed)} files",
    ]

    if commit.files_changed:
        files_str = ", ".join(commit.files_changed[:30])
        if len(commit.files_changed) > 30:
            files_str += f" ... and {len(commit.files_changed) - 30} more"
        parts.append(f"Files: {files_str}")

    if diff_summary:
        parts.append(f"\nDiff:\n{diff_summary}")

    text = "\n".join(parts)

    return CommitChunk(
        id=chunk_id,
        text=text,
        commit_hash=commit.hash,
        short_hash=commit.short_hash,
        author=f"{commit.author_name} <{commit.author_email}>",
        timestamp=commit.timestamp,
        message=commit.message,
        files_changed=",".join(commit.files_changed[:100]),
        branch=commit.branch,
        repo_path=commit.repo_path,
        insertions=commit.insertions,
        deletions=commit.deletions,
    )


class CommitHistoryIndexer:
    """Indexes git commit history into LanceDB for semantic search.

    Stores commits as vector-embedded chunks alongside code chunks,
    enabling queries like "when was auth refactored?" or
    "show me commits that changed the payment module".
    """

    def __init__(self, embedding_service, vector_dims: int = 384):
        self._embedding_service = embedding_service
        self._vector_dims = vector_dims

    def index_commits(
        self,
        repo_path: str,
        db_path: str,
        limit: int = DEFAULT_COMMIT_LIMIT,
        since: Optional[str] = None,
        include_diffs: bool = True,
        batch_size: int = 32,
    ) -> Dict[str, Any]:
        """Index commit history from a git repository.

        Args:
            repo_path: Path to the git repository.
            db_path: Path to LanceDB database.
            limit: Maximum commits to index.
            since: Only index commits after this date.
            include_diffs: Whether to include diff summaries (slower but richer).
            batch_size: Embedding batch size.

        Returns:
            Dict with indexing statistics.
        """
        start = time.time()

        commits = extract_commits(repo_path, limit=limit, since=since)
        if not commits:
            return {
                "total_commits": 0,
                "time_seconds": time.time() - start,
                "branch": get_current_branch(repo_path),
            }

        # Create chunks with optional diff summaries
        chunks: List[CommitChunk] = []
        for commit in commits:
            diff_summary = ""
            if include_diffs:
                diff_summary = get_commit_diff_summary(repo_path, commit.hash)
            chunks.append(create_commit_chunk(commit, diff_summary))

        # Embed in batches
        total_embedded = 0
        for i in range(0, len(chunks), batch_size):
            batch = chunks[i : i + batch_size]
            texts = [c.text for c in batch]
            vectors = self._embedding_service.embed_batch(texts)
            for chunk, vector in zip(batch, vectors):
                chunk.vector = vector
            total_embedded += len(batch)

        # Store in LanceDB commits table
        self._store_commits(db_path, chunks)

        elapsed = time.time() - start
        branch = get_current_branch(repo_path)

        logger.info(
            "Indexed %d commits from %s (branch: %s) in %.1fs",
            len(chunks),
            repo_path,
            branch,
            elapsed,
        )

        return {
            "total_commits": len(chunks),
            "branch": branch,
            "time_seconds": round(elapsed, 2),
            "oldest_commit": commits[-1].timestamp if commits else None,
            "newest_commit": commits[0].timestamp if commits else None,
        }

    def _store_commits(self, db_path: str, chunks: List[CommitChunk]) -> None:
        """Store commit chunks in a dedicated LanceDB table."""
        import pyarrow as pa

        try:
            import lancedb
        except ImportError:
            logger.error("lancedb not installed")
            return

        if not chunks:
            return

        db = lancedb.connect(db_path)

        schema = pa.schema(
            [
                pa.field("id", pa.string()),
                pa.field("vector", pa.list_(pa.float32(), self._vector_dims)),
                pa.field("text", pa.string()),
                pa.field("commit_hash", pa.string()),
                pa.field("short_hash", pa.string()),
                pa.field("author", pa.string()),
                pa.field("timestamp", pa.string()),
                pa.field("message", pa.string()),
                pa.field("files_changed", pa.string()),
                pa.field("branch", pa.string()),
                pa.field("repo_path", pa.string()),
                pa.field("insertions", pa.int32()),
                pa.field("deletions", pa.int32()),
            ]
        )

        rows = [c.to_dict() for c in chunks]

        try:
            table = db.open_table("commits")
            # Delete existing commits for this repo to avoid duplicates
            repo = chunks[0].repo_path
            try:
                table.delete(f"repo_path = '{repo}'")
            except Exception:
                pass
            table.add(rows)
        except Exception:
            # Table doesn't exist, create it
            db.create_table("commits", data=rows, schema=schema, mode="overwrite")

    def search_commits(
        self,
        db_path: str,
        query: str,
        limit: int = 10,
        branch: Optional[str] = None,
        author: Optional[str] = None,
        repo_path: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        """Search commit history by semantic similarity.

        Args:
            db_path: Path to LanceDB database.
            query: Natural language query.
            limit: Maximum results.
            branch: Filter by branch name.
            author: Filter by author name/email.
            repo_path: Filter by repository path.

        Returns:
            List of matching commit dicts with scores.
        """
        try:
            import lancedb
        except ImportError:
            return []

        db = lancedb.connect(db_path)
        try:
            table = db.open_table("commits")
        except Exception:
            return []

        query_vector = self._embedding_service.embed(query)

        # Build filter
        filters = []
        if branch:
            filters.append(f"branch = '{branch}'")
        if author:
            # Use LIKE for partial author matching
            safe_author = author.replace("'", "''")
            filters.append(f"author LIKE '%{safe_author}%'")
        if repo_path:
            filters.append(f"repo_path = '{repo_path}'")

        where_clause = " AND ".join(filters) if filters else None

        # Overfetch for relevance filtering
        overfetch = limit * 3

        try:
            search_builder = table.search(query_vector).limit(overfetch)
            if where_clause:
                search_builder = search_builder.where(where_clause)
            results = search_builder.to_list()
        except Exception as e:
            logger.warning("Commit search failed: %s", e)
            return []

        if not results:
            return []

        # Relevance threshold: drop results with distance > 2x the best result
        # (LanceDB returns L2 distance — lower is better)
        best_distance = results[0].get("_distance", 0.0)
        threshold = max(best_distance * 2.5, 0.5)  # at least 0.5 to avoid over-filtering
        results = [r for r in results if r.get("_distance", 999) <= threshold]

        # Format results — compact, no redundant fields
        formatted = []
        for row in results[:limit]:
            # Normalize score: convert distance to 0-1 similarity
            distance = row.get("_distance", 1.0)
            score = float(max(0.0, 1.0 - (distance / (threshold + 0.01))))

            entry: Dict[str, Any] = {
                "hash": row.get("short_hash", ""),
                "author": row.get("author", "").split(" <")[0],  # name only, no email
                "date": row.get("timestamp", "")[:10],  # date only, no time
                "message": row.get("message", "")[:200],
                "score": round(score, 3),
            }
            # Only include file count (not full list) to save tokens
            files = row.get("files_changed", "")
            if files:
                file_list = files.split(",")
                entry["files"] = len(file_list)
            formatted.append(entry)

        return formatted

    def get_commit_count(self, db_path: str) -> int:
        """Get the number of indexed commits."""
        return CommitHistoryIndexer.count_commits_in_db(db_path)

    @staticmethod
    def count_commits_in_db(db_path: str) -> int:
        """Get the number of indexed commits directly from the DB (no instance needed)."""
        try:
            import lancedb

            db = lancedb.connect(db_path)
            table = db.open_table("commits")
            return table.count_rows()
        except Exception as e:
            logger.debug("count_commits_in_db failed for %s: %s", db_path, e)
            return 0
