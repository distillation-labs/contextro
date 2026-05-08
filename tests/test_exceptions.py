"""Tests for core/exceptions.py."""

import pytest
from contextro_mcp.core.exceptions import (
    ConfigurationError,
    ContextroException,
    EmbeddingError,
    GraphError,
    IndexingError,
    ParseError,
    SearchError,
)


def test_contextro_exception_base():
    with pytest.raises(ContextroException):
        raise ContextroException("base error")


def test_parse_error():
    err = ParseError("/test.py", "python", "syntax error")
    assert err.filepath == "/test.py"
    assert err.language == "python"
    assert "syntax error" in str(err)


def test_indexing_error():
    with pytest.raises(IndexingError):
        raise IndexingError("indexing failed")


def test_embedding_error():
    with pytest.raises(EmbeddingError):
        raise EmbeddingError("model load failed")


def test_search_error():
    with pytest.raises(SearchError):
        raise SearchError("query failed")


def test_configuration_error():
    with pytest.raises(ConfigurationError):
        raise ConfigurationError("invalid config")


def test_graph_error():
    with pytest.raises(GraphError):
        raise GraphError("graph op failed")


def test_all_inherit_from_contextro():
    assert issubclass(ParseError, ContextroException)
    assert issubclass(IndexingError, ContextroException)
    assert issubclass(EmbeddingError, ContextroException)
    assert issubclass(SearchError, ContextroException)
    assert issubclass(ConfigurationError, ContextroException)
    assert issubclass(GraphError, ContextroException)
