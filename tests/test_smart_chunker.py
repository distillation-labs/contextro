"""Tests for smart context-aware chunking."""

import pytest

from contextia_mcp.core.models import Symbol, SymbolType
from contextia_mcp.indexing.smart_chunker import (
    create_file_context_chunks,
    create_relationship_chunks,
    create_smart_chunks,
)


@pytest.fixture
def sample_symbols():
    """Create a set of symbols with call relationships."""
    return [
        Symbol(
            name="authenticate",
            type=SymbolType.FUNCTION,
            filepath="/app/auth.py",
            line_start=10,
            line_end=25,
            language="python",
            signature="def authenticate(username: str, password: str) -> bool",
            docstring="Authenticate a user with credentials.",
            calls=("verify_password", "get_user", "create_session"),
            imports=("hashlib", "db.users"),
        ),
        Symbol(
            name="verify_password",
            type=SymbolType.FUNCTION,
            filepath="/app/auth.py",
            line_start=30,
            line_end=40,
            language="python",
            signature="def verify_password(stored_hash: str, password: str) -> bool",
            docstring="Verify a password against stored hash.",
            calls=("hashlib.sha256",),
            imports=("hashlib",),
        ),
        Symbol(
            name="get_user",
            type=SymbolType.FUNCTION,
            filepath="/app/auth.py",
            line_start=45,
            line_end=55,
            language="python",
            signature="def get_user(username: str) -> Optional[User]",
            docstring="Fetch user from database.",
            calls=("db.query",),
            imports=("db",),
        ),
        Symbol(
            name="handle_request",
            type=SymbolType.FUNCTION,
            filepath="/app/server.py",
            line_start=10,
            line_end=30,
            language="python",
            signature="def handle_request(req: Request) -> Response",
            docstring="Main request handler.",
            calls=("authenticate", "process_data"),
            imports=("auth", "processor"),
        ),
        Symbol(
            name="process_data",
            type=SymbolType.FUNCTION,
            filepath="/app/server.py",
            line_start=35,
            line_end=50,
            language="python",
            signature="def process_data(data: dict) -> dict",
            docstring="Process incoming data.",
            calls=(),
            imports=(),
        ),
    ]


class TestRelationshipChunks:
    """Tests for relationship chunk creation."""

    def test_creates_chunks_for_callers(self, sample_symbols):
        chunks = create_relationship_chunks(sample_symbols)
        # authenticate calls 3 functions, handle_request calls 2,
        # verify_password calls 1, get_user calls 1
        assert len(chunks) >= 3

    def test_chunk_contains_caller_info(self, sample_symbols):
        chunks = create_relationship_chunks(sample_symbols)
        # Find the authenticate chunk
        auth_chunk = next(
            (c for c in chunks if "authenticate" in c.symbol_name), None
        )
        assert auth_chunk is not None
        assert "verify_password" in auth_chunk.text
        assert "get_user" in auth_chunk.text
        assert "create_session" in auth_chunk.text

    def test_chunk_has_correct_metadata(self, sample_symbols):
        chunks = create_relationship_chunks(sample_symbols)
        for chunk in chunks:
            assert chunk.id  # has deterministic ID
            assert chunk.filepath
            assert chunk.symbol_type == "relationship"
            assert chunk.language == "python"

    def test_no_chunks_for_no_calls(self):
        symbols = [
            Symbol(
                name="simple",
                type=SymbolType.FUNCTION,
                filepath="/app/simple.py",
                line_start=1,
                line_end=5,
                language="python",
                signature="def simple(): pass",
                calls=(),
            ),
        ]
        chunks = create_relationship_chunks(symbols)
        assert len(chunks) == 0


class TestFileContextChunks:
    """Tests for file-level context chunk creation."""

    def test_creates_chunks_for_multi_symbol_files(self, sample_symbols):
        chunks = create_file_context_chunks(sample_symbols)
        # auth.py has 3 symbols, server.py has 2 — both should get chunks
        assert len(chunks) == 2

    def test_chunk_contains_all_signatures(self, sample_symbols):
        chunks = create_file_context_chunks(sample_symbols)
        auth_chunk = next(
            (c for c in chunks if "auth.py" in c.filepath), None
        )
        assert auth_chunk is not None
        assert "authenticate" in auth_chunk.text
        assert "verify_password" in auth_chunk.text
        assert "get_user" in auth_chunk.text

    def test_chunk_contains_imports(self, sample_symbols):
        chunks = create_file_context_chunks(sample_symbols)
        auth_chunk = next(
            (c for c in chunks if "auth.py" in c.filepath), None
        )
        assert auth_chunk is not None
        assert "hashlib" in auth_chunk.text

    def test_chunk_has_correct_type(self, sample_symbols):
        chunks = create_file_context_chunks(sample_symbols)
        for chunk in chunks:
            assert chunk.symbol_type == "file_overview"

    def test_skips_single_symbol_files(self):
        symbols = [
            Symbol(
                name="lonely",
                type=SymbolType.FUNCTION,
                filepath="/app/lonely.py",
                line_start=1,
                line_end=5,
                language="python",
                signature="def lonely(): pass",
            ),
        ]
        chunks = create_file_context_chunks(symbols)
        assert len(chunks) == 0


class TestSmartChunks:
    """Tests for the combined smart chunking function."""

    def test_creates_both_types(self, sample_symbols):
        chunks = create_smart_chunks(sample_symbols)
        types = {c.symbol_type for c in chunks}
        assert "relationship" in types
        assert "file_overview" in types

    def test_can_disable_relationships(self, sample_symbols):
        chunks = create_smart_chunks(
            sample_symbols, include_relationships=False
        )
        types = {c.symbol_type for c in chunks}
        assert "relationship" not in types
        assert "file_overview" in types

    def test_can_disable_file_context(self, sample_symbols):
        chunks = create_smart_chunks(
            sample_symbols, include_file_context=False
        )
        types = {c.symbol_type for c in chunks}
        assert "relationship" in types
        assert "file_overview" not in types

    def test_empty_symbols(self):
        chunks = create_smart_chunks([])
        assert chunks == []

    def test_chunks_have_unique_ids(self, sample_symbols):
        chunks = create_smart_chunks(sample_symbols)
        ids = [c.id for c in chunks]
        assert len(ids) == len(set(ids))
