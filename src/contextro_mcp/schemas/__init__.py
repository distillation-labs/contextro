"""Strict input/output schemas for Contextro tools."""

from contextro_mcp.schemas.inputs import (
    AnalyzeInput,
    ForgetInput,
    ImpactInput,
    IndexInput,
    RecallInput,
    RememberInput,
    SearchInput,
    SymbolNameInput,
)
from contextro_mcp.schemas.responses import (
    AnalyzeResponse,
    CalleesResponse,
    CallersResponse,
    ErrorResponse,
    ExplainResponse,
    FindSymbolResponse,
    ForgetResponse,
    HealthResponse,
    ImpactResponse,
    IndexResponse,
    MemoryResponse,
    RecallResponse,
    SearchResponse,
    SearchResult,
    StatusResponse,
    SymbolDetail,
)

__all__ = [
    # Inputs
    "IndexInput",
    "SearchInput",
    "SymbolNameInput",
    "AnalyzeInput",
    "ImpactInput",
    "RememberInput",
    "RecallInput",
    "ForgetInput",
    # Responses
    "ErrorResponse",
    "StatusResponse",
    "HealthResponse",
    "IndexResponse",
    "SearchResult",
    "SearchResponse",
    "SymbolDetail",
    "FindSymbolResponse",
    "CallersResponse",
    "CalleesResponse",
    "AnalyzeResponse",
    "ImpactResponse",
    "ExplainResponse",
    "MemoryResponse",
    "RecallResponse",
    "ForgetResponse",
]
