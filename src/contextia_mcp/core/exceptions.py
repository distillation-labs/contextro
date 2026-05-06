"""Custom exceptions for Contextia."""


class ContextiaException(Exception):
    """Base exception for all Contextia operations."""

    pass


class ParseError(ContextiaException):
    """Raised when file parsing fails."""

    def __init__(self, filepath: str, language: str, details: str):
        self.filepath = filepath
        self.language = language
        self.details = details
        super().__init__(f"Failed to parse {filepath} ({language}): {details}")


class IndexingError(ContextiaException):
    """Raised when indexing operation fails."""

    pass


class EmbeddingError(ContextiaException):
    """Raised when embedding generation fails."""

    pass


class SearchError(ContextiaException):
    """Raised when search operation fails."""

    pass


class ConfigurationError(ContextiaException):
    """Raised when configuration is invalid."""

    pass


class GraphError(ContextiaException):
    """Raised when graph operations fail."""

    pass


class MemoryStoreError(ContextiaException):
    """Raised when memory store operations fail."""

    pass


class FusionError(ContextiaException):
    """Raised when fusion or reranking fails."""

    pass


class AuthenticationError(ContextiaException):
    """Raised when authentication fails."""

    pass


class AuthorizationError(ContextiaException):
    """Raised when a tool access is denied by permission policy."""

    pass


class RateLimitError(ContextiaException):
    """Raised when rate limit is exceeded."""

    pass
