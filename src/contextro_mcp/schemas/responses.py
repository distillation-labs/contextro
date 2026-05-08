"""Pydantic v2 response models for Contextro tools.

Provides strict, LLM-parsable output schemas for all tool responses.
Tools construct these models internally and return .model_dump().
"""

from typing import Any, Dict, List, Optional

from pydantic import BaseModel


class ErrorResponse(BaseModel):
    """Standard error response from any tool."""

    error: str
    code: Optional[str] = None


# --- Status & Health ---


class MemoryInfo(BaseModel):
    peak_rss_mb: float


class StatusResponse(BaseModel):
    version: str
    indexed: bool
    codebase_path: Optional[str] = None
    memory: MemoryInfo
    vector_chunks: Optional[int] = None
    bm25_fts_ready: Optional[bool] = None
    graph: Optional[Dict[str, Any]] = None
    hint: Optional[str] = None


class EngineStatus(BaseModel):
    vector: bool
    bm25: bool
    graph: bool
    memory: bool


class HealthResponse(BaseModel):
    status: str
    uptime_seconds: float
    indexed: bool
    engines: EngineStatus


# --- Index ---


class IndexResponse(BaseModel):
    total_files: int
    total_symbols: int
    total_chunks: int
    time_seconds: float
    languages: Optional[Dict[str, int]] = None
    errors: Optional[List[str]] = None


# --- Search ---


class SearchResult(BaseModel):
    id: Optional[str] = None
    filepath: Optional[str] = None
    absolute_path: Optional[str] = None
    symbol_name: Optional[str] = None
    symbol_type: Optional[str] = None
    language: Optional[str] = None
    line_start: Optional[int] = None
    line_end: Optional[int] = None
    score: Optional[float] = None
    code_snippet: Optional[str] = None
    signature: Optional[str] = None
    parent: Optional[str] = None
    docstring: Optional[str] = None


class SearchResponse(BaseModel):
    query: str
    total: int
    confidence: str
    tokens: int
    results: List[Dict[str, Any]]
    budget_applied: Optional[bool] = None
    sandboxed: Optional[bool] = None
    sandbox_ref: Optional[str] = None
    full_total: Optional[int] = None
    hint: Optional[str] = None


# --- Symbol ---


class LocationInfo(BaseModel):
    file: str
    start_line: int
    end_line: int


class RelationshipInfo(BaseModel):
    type: str
    source_id: str
    target_id: str
    strength: Optional[float] = None


class SymbolDetail(BaseModel):
    id: str
    name: str
    type: str
    language: str
    location: LocationInfo
    complexity: Optional[int] = None
    line_count: Optional[int] = None
    docstring: Optional[str] = None
    visibility: Optional[str] = None
    is_async: Optional[bool] = None
    return_type: Optional[str] = None
    parameter_types: Optional[Dict[str, str]] = None
    relationships_out: Optional[List[RelationshipInfo]] = None
    relationships_in: Optional[List[RelationshipInfo]] = None
    callers: Optional[List[Dict[str, Any]]] = None
    callees: Optional[List[Dict[str, Any]]] = None


class FindSymbolResponse(BaseModel):
    total: int
    symbols: List[Dict[str, Any]]


# --- Callers/Callees ---


class CallersResponse(BaseModel):
    symbol: str
    total: int
    callers: List[Dict[str, Any]]


class CalleesResponse(BaseModel):
    symbol: str
    total: int
    callees: List[Dict[str, Any]]


# --- Analysis ---


class AnalyzeResponse(BaseModel):
    complexity: Dict[str, Any]
    dependencies: Dict[str, Any]
    code_smells: Dict[str, Any]
    quality: Dict[str, Any]


# --- Impact ---


class ImpactResponse(BaseModel):
    symbol: str
    max_depth: int
    total_impacted: int
    impacted_symbols: List[Dict[str, Any]]
    impacted_files: Dict[str, List[str]]


# --- Explain ---


class ExplainResponse(BaseModel):
    """Flexible response from the explain tool (built by ResponseBuilder)."""

    definition: Optional[Dict[str, Any]] = None
    relationships: Optional[Dict[str, Any]] = None
    related_code: Optional[List[Dict[str, Any]]] = None
    analysis: Optional[Dict[str, Any]] = None


# --- Memory ---


class MemoryResponse(BaseModel):
    id: str
    status: str


class RecallResponse(BaseModel):
    query: str
    total: int
    memories: List[Dict[str, Any]]


class ForgetResponse(BaseModel):
    deleted_count: int


# --- Commit History ---


class CommitDetail(BaseModel):
    commit_hash: str
    short_hash: str
    author: str
    timestamp: str
    message: str
    files_changed: List[str] = []
    insertions: int = 0
    deletions: int = 0
    branch: str = ""
    score: Optional[float] = None


class CommitHistoryResponse(BaseModel):
    total: int
    branch: str
    commits: List[Dict[str, Any]]


class CommitSearchResponse(BaseModel):
    query: str
    total: int
    results: List[Dict[str, Any]]


# --- Cross-Repo ---


class RepoInfo(BaseModel):
    path: str
    name: str
    branch: str = ""
    head: str = ""
    indexed: bool = False
    total_files: int = 0
    total_symbols: int = 0
    total_chunks: int = 0
    total_commits: int = 0


class RepoAddResponse(BaseModel):
    status: str
    repo: Dict[str, Any]


class RepoRemoveResponse(BaseModel):
    status: str
    removed: bool


class RepoStatusResponse(BaseModel):
    total_repos: int
    repos: List[Dict[str, Any]]
    watcher_running: bool = False
