"""Custom exceptions for Contextro."""


class ContextroException(Exception):
    """Base exception for all Contextro operations."""

    pass


class ParseError(ContextroException):
    """Raised when file parsing fails."""

    def __init__(self, filepath: str, language: str, details: str):
        self.filepath = filepath
        self.language = language
        self.details = details
        super().__init__(f"Failed to parse {filepath} ({language}): {details}")


class IndexingError(ContextroException):
    """Raised when indexing operation fails."""

    pass


class EmbeddingError(ContextroException):
    """Raised when embedding generation fails."""

    pass


class SearchError(ContextroException):
    """Raised when search operation fails."""

    pass


class ConfigurationError(ContextroException):
    """Raised when configuration is invalid."""

    pass


class GraphError(ContextroException):
    """Raised when graph operations fail."""

    pass


class MemoryStoreError(ContextroException):
    """Raised when memory store operations fail."""

    pass


class FusionError(ContextroException):
    """Raised when fusion or reranking fails."""

    pass


class AuthenticationError(ContextroException):
    """Raised when authentication fails."""

    pass


class AuthorizationError(ContextroException):
    """Raised when a tool access is denied by permission policy."""

    pass


class RateLimitError(ContextroException):
    """Raised when rate limit is exceeded."""

    pass
