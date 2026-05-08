"""Tests for git commit history indexer."""

import subprocess
from pathlib import Path
from unittest.mock import MagicMock

import pytest
from contextro_mcp.git.commit_indexer import (
    CommitChunk,
    CommitHistoryIndexer,
    CommitInfo,
    create_commit_chunk,
    extract_commits,
    get_current_branch,
    get_current_head,
    get_repo_root,
    is_git_repo,
)


@pytest.fixture
def git_repo(tmp_path):
    """Create a temporary git repository with some commits."""
    repo = tmp_path / "test_repo"
    repo.mkdir()

    # Initialize git repo
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

    # Create initial commit
    (repo / "main.py").write_text("def hello():\n    print('hello')\n")
    subprocess.run(
        ["git", "add", "."],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Initial commit: add hello function"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )

    # Second commit
    (repo / "utils.py").write_text("def helper():\n    return 42\n")
    subprocess.run(
        ["git", "add", "."],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Add utility helper function"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )

    # Third commit
    (repo / "main.py").write_text(
        "from utils import helper\n\ndef hello():\n    helper()\n    print('hello')\n"
    )
    subprocess.run(
        ["git", "add", "."],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Refactor: use helper in hello"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )

    return repo


class TestGitDetection:
    """Tests for git repository detection utilities."""

    def test_is_git_repo_true(self, git_repo):
        assert is_git_repo(str(git_repo)) is True

    def test_is_git_repo_false(self, tmp_path):
        assert is_git_repo(str(tmp_path)) is False

    def test_get_current_branch(self, git_repo):
        branch = get_current_branch(str(git_repo))
        # Could be 'main' or 'master' depending on git config
        assert branch in ("main", "master")

    def test_get_current_head(self, git_repo):
        head = get_current_head(str(git_repo))
        assert len(head) == 40  # Full SHA

    def test_get_repo_root(self, git_repo):
        root = get_repo_root(str(git_repo))
        assert root is not None
        assert Path(root).resolve() == git_repo.resolve()

    def test_get_repo_root_nonrepo(self, tmp_path):
        root = get_repo_root(str(tmp_path))
        # tmp_path might be inside a git repo (the workspace), or not
        # Just verify it doesn't crash
        assert root is None or isinstance(root, str)


class TestExtractCommits:
    """Tests for commit extraction."""

    def test_extract_commits_basic(self, git_repo):
        commits = extract_commits(str(git_repo), limit=10)
        assert len(commits) == 3
        # Newest first
        assert "Refactor" in commits[0].message
        assert "utility" in commits[1].message
        assert "Initial" in commits[2].message

    def test_extract_commits_limit(self, git_repo):
        commits = extract_commits(str(git_repo), limit=2)
        assert len(commits) == 2

    def test_commit_has_metadata(self, git_repo):
        commits = extract_commits(str(git_repo), limit=1)
        assert len(commits) == 1
        c = commits[0]
        assert c.hash and len(c.hash) == 40
        assert c.short_hash and len(c.short_hash) >= 7
        assert c.author_name == "Test User"
        assert c.author_email == "test@test.com"
        assert c.timestamp  # ISO 8601
        assert c.message

    def test_commit_has_file_changes(self, git_repo):
        commits = extract_commits(str(git_repo), limit=10)
        # The "Add utility helper" commit should have utils.py
        add_commit = [c for c in commits if "utility" in c.message][0]
        assert "utils.py" in add_commit.files_changed

    def test_extract_from_nonrepo(self, tmp_path):
        commits = extract_commits(str(tmp_path))
        assert commits == []

    def test_commit_to_dict(self, git_repo):
        commits = extract_commits(str(git_repo), limit=1)
        d = commits[0].to_dict()
        assert "hash" in d
        assert "message" in d
        assert "files_changed" in d
        assert isinstance(d["files_changed"], list)


class TestCommitChunking:
    """Tests for commit-to-chunk conversion."""

    def test_create_commit_chunk(self):
        commit = CommitInfo(
            hash="abc123" * 7 + "ab",
            short_hash="abc123",
            author_name="Alice",
            author_email="alice@example.com",
            timestamp="2025-01-15T10:00:00+00:00",
            message="Fix authentication bug in login flow",
            files_changed=["auth/login.py", "auth/session.py"],
            insertions=15,
            deletions=3,
            branch="main",
            repo_path="/repo",
        )
        chunk = create_commit_chunk(commit, diff_summary="+ new code")
        assert chunk.id  # deterministic hash
        assert "Fix authentication bug" in chunk.text
        assert "Alice" in chunk.text
        assert "auth/login.py" in chunk.text
        assert chunk.commit_hash == commit.hash
        assert chunk.branch == "main"

    def test_chunk_to_dict(self):
        chunk = CommitChunk(
            id="test123",
            text="test text",
            commit_hash="abc",
            short_hash="abc",
            author="Test",
            timestamp="2025-01-01",
            message="test",
            files_changed="a.py,b.py",
            branch="main",
            repo_path="/repo",
            insertions=1,
            deletions=0,
        )
        d = chunk.to_dict()
        assert d["id"] == "test123"
        assert d["commit_hash"] == "abc"
        assert d["vector"] == []


class TestCommitHistoryIndexer:
    """Tests for the full commit indexing pipeline."""

    def test_index_commits(self, git_repo, tmp_path):
        mock_svc = MagicMock()
        mock_svc.embed_batch.side_effect = lambda texts, **kw: [[0.1] * 384 for _ in texts]
        mock_svc.embed.return_value = [0.1] * 384

        indexer = CommitHistoryIndexer(
            embedding_service=mock_svc,
            vector_dims=384,
        )

        db_path = str(tmp_path / "lancedb")
        result = indexer.index_commits(
            repo_path=str(git_repo),
            db_path=db_path,
            limit=10,
            include_diffs=False,
        )

        assert result["total_commits"] == 3
        assert result["branch"] in ("main", "master")
        assert result["time_seconds"] > 0

    def test_search_commits(self, git_repo, tmp_path):
        mock_svc = MagicMock()
        mock_svc.embed_batch.side_effect = lambda texts, **kw: [[0.1] * 384 for _ in texts]
        mock_svc.embed.return_value = [0.1] * 384

        indexer = CommitHistoryIndexer(
            embedding_service=mock_svc,
            vector_dims=384,
        )

        db_path = str(tmp_path / "lancedb")
        indexer.index_commits(
            repo_path=str(git_repo),
            db_path=db_path,
            include_diffs=False,
        )

        results = indexer.search_commits(
            db_path=db_path,
            query="helper function",
            limit=5,
        )
        assert len(results) > 0
        assert "hash" in results[0]
        assert "message" in results[0]

    def test_get_commit_count(self, git_repo, tmp_path):
        mock_svc = MagicMock()
        mock_svc.embed_batch.side_effect = lambda texts, **kw: [[0.1] * 384 for _ in texts]

        indexer = CommitHistoryIndexer(
            embedding_service=mock_svc,
            vector_dims=384,
        )

        db_path = str(tmp_path / "lancedb")
        indexer.index_commits(
            repo_path=str(git_repo),
            db_path=db_path,
            include_diffs=False,
        )

        count = indexer.get_commit_count(db_path)
        assert count == 3

    def test_search_empty_db(self, tmp_path):
        mock_svc = MagicMock()
        mock_svc.embed.return_value = [0.1] * 384

        indexer = CommitHistoryIndexer(
            embedding_service=mock_svc,
            vector_dims=384,
        )

        results = indexer.search_commits(
            db_path=str(tmp_path / "nonexistent"),
            query="test",
        )
        assert results == []
