"""Export publication-grade MCP tool schemas and examples."""

# ruff: noqa: E402, I001, E501

from __future__ import annotations

import argparse
import asyncio
import gc
import inspect
import json
import os
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Annotated, Any, get_args, get_origin

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from benchmark_utils import call_tool, index_codebase
from public_proxy_repo import DEFAULT_PROXY_REPO, materialize_public_proxy_repo, seed_git_history

from contextro_mcp.schemas.inputs import (
    CommitHistoryInput,
    CommitSearchInput,
    ForgetInput,
    ImpactInput,
    IndexInput,
    RecallInput,
    RememberInput,
    RepoAddInput,
    RepoRemoveInput,
    SearchInput,
    SymbolNameInput,
)
from contextro_mcp.schemas.responses import (
    AnalyzeResponse,
    CallersResponse,
    CalleesResponse,
    CommitHistoryResponse,
    CommitSearchResponse,
    ExplainResponse,
    FindSymbolResponse,
    ForgetResponse,
    HealthResponse,
    ImpactResponse,
    RecallResponse,
    SearchResponse,
    StatusResponse,
)


INPUT_MODELS = {
    "index": IndexInput,
    "search": SearchInput,
    "find_symbol": SymbolNameInput,
    "impact": ImpactInput,
    "remember": RememberInput,
    "recall": RecallInput,
    "forget": ForgetInput,
    "commit_history": CommitHistoryInput,
    "commit_search": CommitSearchInput,
    "repo_add": RepoAddInput,
    "repo_remove": RepoRemoveInput,
}

OUTPUT_MODELS = {
    "status": StatusResponse,
    "health": HealthResponse,
    "search": SearchResponse,
    "find_symbol": FindSymbolResponse,
    "find_callers": CallersResponse,
    "find_callees": CalleesResponse,
    "analyze": AnalyzeResponse,
    "impact": ImpactResponse,
    "explain": ExplainResponse,
    "recall": RecallResponse,
    "forget": ForgetResponse,
    "commit_history": CommitHistoryResponse,
    "commit_search": CommitSearchResponse,
}

TOOL_ORDER = [
    "status",
    "health",
    "index",
    "search",
    "find_symbol",
    "find_callers",
    "find_callees",
    "explain",
    "impact",
    "analyze",
    "focus",
    "overview",
    "architecture",
    "restore",
    "audit",
    "dead_code",
    "circular_dependencies",
    "test_coverage_map",
    "code",
    "commit_search",
    "commit_history",
    "remember",
    "recall",
    "forget",
    "knowledge",
    "session_snapshot",
    "compact",
    "retrieve",
    "repo_status",
    "introspect",
]


def _configure_environment(storage_dir: Path) -> None:
    os.environ["CTX_STORAGE_DIR"] = str(storage_dir)
    os.environ["CTX_PERMISSION_LEVEL"] = "full"
    os.environ.setdefault("CTX_EMBEDDING_MODEL", "potion-code-16m")


def _reset_runtime():
    import contextro_mcp.server as server_module
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import reset_state

    reset_settings()
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    return server_module.create_server(), server_module


def _shutdown_runtime() -> None:
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import get_state, reset_state
    import contextro_mcp.server as server_module

    try:
        get_state().shutdown()
    except Exception:
        pass
    reset_settings()
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    gc.collect()


def _json_type(annotation: Any) -> tuple[str, dict[str, Any]]:
    if annotation is inspect._empty:
        return "string", {}
    origin = get_origin(annotation)
    if origin is Annotated:
        return _json_type(get_args(annotation)[0])
    if origin in (list, tuple):
        args = get_args(annotation)
        item_type, item_extra = _json_type(args[0] if args else str)
        return "array", {"items": {"type": item_type, **item_extra}}
    if origin is dict:
        return "object", {"additionalProperties": True}
    if origin is None:
        pass
    if annotation in (str, Path):
        return "string", {}
    if annotation is int:
        return "integer", {}
    if annotation is float:
        return "number", {}
    if annotation is bool:
        return "boolean", {}
    return "string", {}


def _signature_schema(fn) -> dict[str, Any]:
    signature = inspect.signature(fn)
    properties: dict[str, Any] = {}
    required: list[str] = []
    for name, parameter in signature.parameters.items():
        schema_type, extra = _json_type(parameter.annotation)
        entry = {"type": schema_type, **extra}
        if get_origin(parameter.annotation) is Annotated:
            args = get_args(parameter.annotation)
            if len(args) > 1 and isinstance(args[1], str):
                entry["description"] = args[1]
        if parameter.default is not inspect._empty:
            entry["default"] = parameter.default
        else:
            required.append(name)
        properties[name] = entry
    return {"type": "object", "properties": properties, "required": required}


def _infer_schema(value: Any) -> dict[str, Any]:
    if isinstance(value, dict):
        return {
            "type": "object",
            "properties": {key: _infer_schema(subvalue) for key, subvalue in value.items()},
            "required": list(value.keys()),
        }
    if isinstance(value, list):
        if not value:
            return {"type": "array", "items": {}}
        return {"type": "array", "items": _infer_schema(value[0])}
    if isinstance(value, bool):
        return {"type": "boolean"}
    if isinstance(value, int):
        return {"type": "integer"}
    if isinstance(value, float):
        return {"type": "number"}
    if value is None:
        return {"type": "null"}
    return {"type": "string"}


async def _tool_lookup(mcp) -> dict[str, Any]:
    tools = await mcp.list_tools()
    return {tool.name: tool for tool in tools if tool.name in TOOL_ORDER}


async def _build_examples(mcp, proxy_repo: Path) -> dict[str, dict[str, Any]]:
    examples: dict[str, dict[str, Any]] = {}
    search_example = await call_tool(mcp, "search", {"query": "invoice reminder overdue enterprise", "limit": 5})
    sandbox_ref = search_example.get("sandbox_ref")
    if not sandbox_ref:
        sandboxed_search = await call_tool(
            mcp,
            "search",
            {"query": "generated filler module", "limit": 20, "context_budget": 120},
        )
        search_example = sandboxed_search if sandboxed_search.get("sandbox_ref") else search_example
        sandbox_ref = search_example.get("sandbox_ref")

    await call_tool(mcp, "remember", {"content": "Publication schema example memory", "tags": "publication-schema"})
    compact_example = await call_tool(
        mcp,
        "compact",
        {"content": "Publication schema example archive payload for appendix generation."},
    )

    examples["status"] = {"args": {}, "result": await call_tool(mcp, "status")}
    examples["health"] = {"args": {}, "result": await call_tool(mcp, "health")}
    examples["index"] = {"args": {"path": str(proxy_repo)}, "result": await call_tool(mcp, "index", {"path": str(proxy_repo)})}
    examples["search"] = {"args": {"query": "invoice reminder overdue enterprise", "limit": 5}, "result": search_example}
    examples["find_symbol"] = {"args": {"name": "PartnerProfileProjector"}, "result": await call_tool(mcp, "find_symbol", {"name": "PartnerProfileProjector"})}
    examples["find_callers"] = {"args": {"symbol_name": "emit_partner_metric"}, "result": await call_tool(mcp, "find_callers", {"symbol_name": "emit_partner_metric"})}
    examples["find_callees"] = {"args": {"symbol_name": "prepare_partner_onboarding_context"}, "result": await call_tool(mcp, "find_callees", {"symbol_name": "prepare_partner_onboarding_context"})}
    examples["explain"] = {"args": {"symbol_name": "schedule_digest_delivery"}, "result": await call_tool(mcp, "explain", {"symbol_name": "schedule_digest_delivery"})}
    examples["impact"] = {"args": {"symbol_name": "resolve_notification_channels", "max_depth": 5}, "result": await call_tool(mcp, "impact", {"symbol_name": "resolve_notification_channels", "max_depth": 5})}
    examples["analyze"] = {"args": {"path": "packages/partners/src/partners"}, "result": await call_tool(mcp, "analyze", {"path": "packages/partners/src/partners"})}
    examples["focus"] = {"args": {"path": "packages/partners/src/partners/onboarding.py", "include_code": True}, "result": await call_tool(mcp, "focus", {"path": "packages/partners/src/partners/onboarding.py", "include_code": True})}
    examples["overview"] = {"args": {}, "result": await call_tool(mcp, "overview")}
    examples["architecture"] = {"args": {}, "result": await call_tool(mcp, "architecture")}
    examples["restore"] = {"args": {}, "result": await call_tool(mcp, "restore")}
    examples["audit"] = {"args": {}, "result": await call_tool(mcp, "audit")}
    examples["dead_code"] = {"args": {}, "result": await call_tool(mcp, "dead_code")}
    examples["circular_dependencies"] = {"args": {}, "result": await call_tool(mcp, "circular_dependencies")}
    examples["test_coverage_map"] = {"args": {}, "result": await call_tool(mcp, "test_coverage_map")}
    examples["code"] = {
        "args": {"operation": "search_symbols", "symbol_name": "ProfileProjector", "path": "packages/catalog/src", "limit": 5},
        "result": await call_tool(mcp, "code", {"operation": "search_symbols", "symbol_name": "ProfileProjector", "path": "packages/catalog/src", "limit": 5}),
    }
    examples["commit_search"] = {"args": {"query": "digest delay handling", "limit": 3}, "result": await call_tool(mcp, "commit_search", {"query": "digest delay handling", "limit": 3})}
    examples["commit_history"] = {"args": {"limit": 3}, "result": await call_tool(mcp, "commit_history", {"limit": 3})}
    examples["remember"] = {"args": {"content": "Publication schema example memory", "tags": "publication-schema"}, "result": {"id": "see-publication-schema-tag", "status": "stored"}}
    examples["recall"] = {"args": {"query": "publication schema example memory", "limit": 1}, "result": await call_tool(mcp, "recall", {"query": "publication schema example memory", "limit": 1})}
    examples["forget"] = {"args": {"tags": "publication-schema"}, "result": await call_tool(mcp, "forget", {"tags": "publication-schema"})}
    examples["knowledge"] = {"args": {"command": "show"}, "result": await call_tool(mcp, "knowledge", {"command": "show"})}
    examples["session_snapshot"] = {"args": {}, "result": await call_tool(mcp, "session_snapshot")}
    examples["compact"] = {"args": {"content": "Publication schema example archive payload for appendix generation."}, "result": compact_example}
    if sandbox_ref:
        examples["retrieve"] = {"args": {"ref_id": sandbox_ref}, "result": await call_tool(mcp, "retrieve", {"ref_id": sandbox_ref})}
    else:
        examples["retrieve"] = {"args": {"ref_id": "sx_example"}, "result": {"error": "No sandboxed result available in this sample run."}}
    examples["repo_status"] = {"args": {}, "result": await call_tool(mcp, "repo_status")}
    examples["introspect"] = {"args": {"query": "search and session recovery tools"}, "result": await call_tool(mcp, "introspect", {"query": "search and session recovery tools"})}
    return examples


async def export_schemas(output_path: Path, proxy_repo_root: Path) -> dict[str, Any]:
    if not proxy_repo_root.exists():
        materialize_public_proxy_repo(proxy_repo_root)

    temp_dir = Path(tempfile.mkdtemp(prefix="ctx_schema_export_"))
    working_repo = temp_dir / "repo"
    storage_dir = temp_dir / ".contextro"
    shutil.copytree(proxy_repo_root, working_repo)
    seed_git_history(working_repo)
    storage_dir.mkdir()

    _configure_environment(storage_dir)
    mcp, server_module = _reset_runtime()
    try:
        await index_codebase(mcp, server_module, str(working_repo), timeout_seconds=300)
        tool_lookup = await _tool_lookup(mcp)
        examples = await _build_examples(mcp, working_repo)

        export_rows = []
        for tool_name in TOOL_ORDER:
            tool = tool_lookup[tool_name]
            input_schema = (
                INPUT_MODELS[tool_name].model_json_schema()
                if tool_name in INPUT_MODELS
                else _signature_schema(tool.fn)
            )
            example = examples[tool_name]
            output_schema = (
                OUTPUT_MODELS[tool_name].model_json_schema()
                if tool_name in OUTPUT_MODELS
                else _infer_schema(example["result"])
            )
            export_rows.append(
                {
                    "tool": tool_name,
                    "description": tool.description,
                    "annotations": (
                        tool.annotations.model_dump(exclude_none=True)
                        if tool.annotations is not None
                        else {}
                    ),
                    "input_schema": input_schema,
                    "example_request": example["args"],
                    "output_schema": output_schema,
                    "example_response": example["result"],
                }
            )

        payload = {
            "schema_version": 1,
            "source": "scripts/export_tool_api_schemas.py",
            "proxy_repo": str(proxy_repo_root),
            "tool_count": len(export_rows),
            "tools": export_rows,
        }
        output_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
        return payload
    finally:
        _shutdown_runtime()
        shutil.rmtree(temp_dir, ignore_errors=True)


def main() -> dict[str, Any]:
    parser = argparse.ArgumentParser(description="Export publication-grade MCP tool schemas.")
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "docs" / "publication" / "contextro-tool-api-schemas.json",
        help="Where to write the schema export JSON.",
    )
    parser.add_argument(
        "--proxy-repo",
        type=Path,
        default=DEFAULT_PROXY_REPO,
        help="Path to the tracked public proxy repository.",
    )
    args = parser.parse_args()
    payload = asyncio.run(export_schemas(args.output.resolve(), args.proxy_repo.resolve()))
    print(json.dumps({"tool_count": payload["tool_count"], "output": str(args.output)}, indent=2))
    return payload


if __name__ == "__main__":
    main()
