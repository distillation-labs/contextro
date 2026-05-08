"""Tool permission model for Contextro."""

from collections.abc import Iterable
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


class ToolCategory(Enum):
    """Permission categories for MCP tools."""

    READ = "read"
    MUTATE = "mutate"
    WRITE = "write"


READ_TOOLS = frozenset(
    {
        "status",
        "search",
        "find_symbol",
        "find_callers",
        "find_callees",
        "explain",
        "overview",
        "architecture",
        "recall",
        "health",
        "session_snapshot",
        "retrieve",
        "introspect",
        "focus",
        "restore",
        "audit",
        "dead_code",
        "circular_dependencies",
        "test_coverage_map",
        "commit_history",
        "commit_search",
        "repo_status",
    }
)

MUTATE_TOOLS = frozenset(
    {
        "index",
        "analyze",
        "impact",
        "code",
        "knowledge",
        "sidecar_export",
        "skill_prompt",
        "docs_bundle",
        "repo_add",
        "repo_remove",
    }
)

WRITE_TOOLS = frozenset({"remember", "forget", "compact"})


def register_tool_permission(
    registry: dict[str, ToolCategory],
    tool_name: str,
    category: ToolCategory,
) -> None:
    """Register a single tool category and reject accidental duplicates."""
    if tool_name in registry and registry[tool_name] != category:
        raise ValueError(f"Tool '{tool_name}' already registered as {registry[tool_name].value}.")
    registry[tool_name] = category


def register_tool_permissions(
    registry: dict[str, ToolCategory],
    tool_names: Iterable[str],
    category: ToolCategory,
) -> dict[str, ToolCategory]:
    """Register many tools in one category."""
    for tool_name in sorted(tool_names):
        register_tool_permission(registry, tool_name, category)
    return registry


def _build_tool_permissions() -> dict[str, ToolCategory]:
    permissions: dict[str, ToolCategory] = {}
    register_tool_permissions(permissions, READ_TOOLS, ToolCategory.READ)
    register_tool_permissions(permissions, MUTATE_TOOLS, ToolCategory.MUTATE)
    register_tool_permissions(permissions, WRITE_TOOLS, ToolCategory.WRITE)
    return permissions


TOOL_PERMISSIONS: dict[str, ToolCategory] = _build_tool_permissions()


@dataclass(frozen=True)
class PermissionPolicy:
    """Defines which tool categories and specific tools are allowed/denied."""

    allowed_categories: frozenset[ToolCategory] = field(
        default_factory=lambda: frozenset({ToolCategory.READ})
    )
    allowed_tools: frozenset[str] = field(default_factory=frozenset)
    denied_tools: frozenset[str] = field(default_factory=frozenset)


# Preset policies
DEFAULT_POLICY = PermissionPolicy(allowed_categories=frozenset({ToolCategory.READ}))

FULL_ACCESS_POLICY = PermissionPolicy(
    allowed_categories=frozenset({ToolCategory.READ, ToolCategory.MUTATE, ToolCategory.WRITE})
)


def check_permission(tool_name: str, policy: PermissionPolicy) -> bool:
    """Check if a tool is allowed under the given policy.

    Resolution order:
    1. Explicit deny overrides everything
    2. Explicit allow overrides category check
    3. Category-based check
    4. Unknown tools are denied
    """
    if tool_name in policy.denied_tools:
        return False
    if tool_name in policy.allowed_tools:
        return True
    category = TOOL_PERMISSIONS.get(tool_name)
    if category is None:
        return False
    return category in policy.allowed_categories


def get_tool_category(tool_name: str) -> Optional[ToolCategory]:
    """Get the category for a tool, or None if unknown."""
    return TOOL_PERMISSIONS.get(tool_name)


def policy_from_level(level: str) -> PermissionPolicy:
    """Create a PermissionPolicy from a permission level string.

    Args:
        level: "read" for read-only, "full" for all access.
    """
    if level.strip().lower() == "full":
        return FULL_ACCESS_POLICY
    return DEFAULT_POLICY
