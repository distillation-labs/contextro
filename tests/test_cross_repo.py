"""Tests for cross-repository context management."""

import subprocess

import pytest
from contextro_mcp.git.cross_repo import CrossRepoManager, RepoContext


@pytest.fixture
def git_repo(tmp_path):
    """Create a temporary git repository."""
    repo = tmp_path / "repo_a"
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
        ["git", "config", "user.name", "Test"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    (repo / "main.py").write_text("print('a')\n")
    subprocess.run(
        ["git", "add", "."],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "init"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    return repo


@pytest.fixture
def second_repo(tmp_path):
    """Create a second temporary git repository."""
    repo = tmp_path / "repo_b"
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
        ["git", "config", "user.name", "Test"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    (repo / "api.py").write_text("print('b')\n")
    subprocess.run(
        ["git", "add", "."],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "init b"],
        cwd=str(repo),
        capture_output=True,
        check=True,
    )
    return repo


class TestRepoContext:
    """Tests for RepoContext dataclass."""

    def test_to_dict(self):
        ctx = RepoContext(
            path="/test/repo",
            name="test-repo",
            branch="main",
            head="abc123def456",
            indexed=True,
            total_files=10,
            total_symbols=50,
        )
        d = ctx.to_dict()
        assert d["path"] == "/test/repo"
        assert d["name"] == "test-repo"
        assert d["branch"] == "main"
        assert d["head"] == "abc123def456"[:12]
        assert d["indexed"] is True
        assert d["total_files"] == 10


class TestCrossRepoManager:
    """Tests for CrossRepoManager."""

    def test_register_repo(self, git_repo):
        manager = CrossRepoManager()
        ctx = manager.register_repo(str(git_repo), name="repo-a")
        assert ctx.name == "repo-a"
        assert ctx.branch in ("main", "master")
        assert manager.repo_count == 1

    def test_register_repo_default_name(self, git_repo):
        manager = CrossRepoManager()
        ctx = manager.register_repo(str(git_repo))
        assert ctx.name == git_repo.name

    def test_register_duplicate(self, git_repo):
        manager = CrossRepoManager()
        ctx1 = manager.register_repo(str(git_repo))
        ctx2 = manager.register_repo(str(git_repo))
        assert ctx1 is ctx2
        assert manager.repo_count == 1

    def test_unregister_repo(self, git_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo))
        assert manager.unregister_repo(str(git_repo)) is True
        assert manager.repo_count == 0

    def test_unregister_nonexistent(self):
        manager = CrossRepoManager()
        assert manager.unregister_repo("/nonexistent") is False

    def test_get_repo(self, git_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo), name="test")
        ctx = manager.get_repo(str(git_repo))
        assert ctx is not None
        assert ctx.name == "test"

    def test_get_repo_by_name(self, git_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo), name="my-repo")
        ctx = manager.get_repo_by_name("my-repo")
        assert ctx is not None
        assert ctx.path == str(git_repo.resolve())

    def test_get_repo_by_name_not_found(self):
        manager = CrossRepoManager()
        assert manager.get_repo_by_name("nonexistent") is None

    def test_multiple_repos(self, git_repo, second_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo), name="repo-a")
        manager.register_repo(str(second_repo), name="repo-b")
        assert manager.repo_count == 2
        assert len(manager.repos) == 2
        assert len(manager.repo_paths) == 2

    def test_update_repo_stats(self, git_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo))
        manager.update_repo_stats(
            str(git_repo),
            total_files=100,
            total_symbols=500,
            total_chunks=200,
            total_commits=50,
            languages={"python": 80, "javascript": 20},
        )
        ctx = manager.get_repo(str(git_repo))
        assert ctx.indexed is True
        assert ctx.total_files == 100
        assert ctx.total_symbols == 500
        assert ctx.total_commits == 50
        assert ctx.languages["python"] == 80

    def test_update_branch(self, git_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo))
        manager.update_branch(str(git_repo), "feature", "abc123")
        ctx = manager.get_repo(str(git_repo))
        assert ctx.branch == "feature"
        assert ctx.head == "abc123"

    def test_get_all_status(self, git_repo, second_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo), name="a")
        manager.register_repo(str(second_repo), name="b")
        manager.update_repo_stats(str(git_repo), total_files=10)
        manager.update_repo_stats(str(second_repo), total_files=20)

        status = manager.get_all_status()
        assert status["total_repos"] == 2
        assert status["total_files"] == 30
        assert len(status["repos"]) == 2

    def test_find_repo_for_file(self, git_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo))
        filepath = str(git_repo / "main.py")
        ctx = manager.find_repo_for_file(filepath)
        assert ctx is not None
        assert ctx.path == str(git_repo.resolve())

    def test_find_repo_for_file_not_found(self):
        manager = CrossRepoManager()
        assert manager.find_repo_for_file("/nonexistent/file.py") is None

    def test_cross_repo_summary(self, git_repo, second_repo):
        manager = CrossRepoManager()
        manager.register_repo(str(git_repo), name="a")
        manager.register_repo(str(second_repo), name="b")
        manager.update_repo_stats(
            str(git_repo),
            total_files=10,
            languages={"python": 8, "javascript": 2},
        )
        manager.update_repo_stats(
            str(second_repo),
            total_files=5,
            languages={"python": 3, "rust": 2},
        )

        summary = manager.get_cross_repo_summary()
        assert summary["total_repos"] == 2
        assert summary["shared_languages"]["python"] == 11
        assert summary["shared_languages"]["javascript"] == 2
        assert summary["shared_languages"]["rust"] == 2
        assert summary["total_files_across_repos"] == 15
