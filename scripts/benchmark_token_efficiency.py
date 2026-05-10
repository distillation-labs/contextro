"""Benchmark for measuring Contextro's token efficiency.

Metrics:
- tokens_per_search: Average tokens in search response output
- tokens_per_explain: Average tokens in explain response output
- tokens_per_status: Tokens in status response
- total_output_tokens: Sum of all tool outputs for a standard workflow
- cache_hit_rate: Percentage of queries served from cache (0 at baseline)

Lower total_output_tokens = better context efficiency.
"""

import json
import sys
import time
from pathlib import Path

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from benchmark_utils import (
    benchmark_session,
    call_tool,
    estimate_tokens,
    estimate_tokens_toon,
    index_codebase,
    sync_vector_engine,
)

from contextro_mcp.token_counting import tokenizer_metadata


def create_test_codebase(tmp_dir: Path) -> Path:
    """Create a realistic test codebase for benchmarking."""
    src = tmp_dir / "src"
    src.mkdir(parents=True, exist_ok=True)

    (src / "auth.py").write_text('''"""Authentication module."""
from typing import Optional
import hashlib
import secrets

class AuthManager:
    """Manages user authentication and session tokens."""
    def __init__(self, secret_key: str, token_expiry: int = 3600):
        self.secret_key = secret_key
        self.token_expiry = token_expiry
        self._sessions: dict[str, dict] = {}

    def authenticate(self, username: str, password: str) -> Optional[str]:
        """Authenticate a user and return a session token."""
        password_hash = hashlib.sha256(password.encode()).hexdigest()
        if self._verify_credentials(username, password_hash):
            token = secrets.token_urlsafe(32)
            self._sessions[token] = {"user": username, "created": 0}
            return token
        return None

    def _verify_credentials(self, username: str, password_hash: str) -> bool:
        """Verify user credentials against store."""
        return username == "admin" and len(password_hash) == 64

    def validate_token(self, token: str) -> Optional[str]:
        """Validate a session token and return the username."""
        session = self._sessions.get(token)
        if session:
            return session["user"]
        return None

    def revoke_token(self, token: str) -> bool:
        """Revoke a session token."""
        if token in self._sessions:
            del self._sessions[token]
            return True
        return False

def create_auth_manager(config: dict) -> AuthManager:
    """Factory function to create an AuthManager from config."""
    return AuthManager(
        secret_key=config.get("secret_key", "default"),
        token_expiry=config.get("token_expiry", 3600),
    )
''')

    (src / "database.py").write_text('''"""Database connection and query module."""
from typing import Any, List, Optional
from dataclasses import dataclass, field

@dataclass(frozen=True)
class QueryResult:
    """Immutable query result."""
    rows: List[dict] = field(default_factory=list)
    affected: int = 0
    error: Optional[str] = None

class DatabasePool:
    """Connection pool for database operations."""
    def __init__(self, connection_string: str, pool_size: int = 5):
        self.connection_string = connection_string
        self.pool_size = pool_size
        self._connections: list = []
        self._available: list = []

    def execute(self, query: str, params: Optional[dict] = None) -> QueryResult:
        """Execute a query and return results."""
        try:
            conn = self._get_connection()
            return QueryResult(rows=[], affected=0)
        except Exception as e:
            return QueryResult(error=str(e))

    def _get_connection(self):
        """Get a connection from the pool."""
        if self._available:
            return self._available.pop()
        if len(self._connections) < self.pool_size:
            conn = self._create_connection()
            self._connections.append(conn)
            return conn
        raise RuntimeError("Connection pool exhausted")

    def _create_connection(self):
        """Create a new database connection."""
        return {"uri": self.connection_string, "active": True}

    def close_all(self):
        """Close all connections in the pool."""
        self._connections.clear()
        self._available.clear()

def get_user_by_id(pool: DatabasePool, user_id: int) -> Optional[dict]:
    """Fetch a user by their ID."""
    result = pool.execute("SELECT * FROM users WHERE id = :id", {"id": user_id})
    if result.rows:
        return result.rows[0]
    return None
''')

    (src / "api.py").write_text('''"""REST API handlers."""
from typing import Any
from auth import AuthManager, create_auth_manager
from database import DatabasePool, get_user_by_id

class APIServer:
    """HTTP API server with authentication."""
    def __init__(self, auth: AuthManager, db: DatabasePool):
        self.auth = auth
        self.db = db
        self._routes: dict[str, callable] = {}

    def handle_request(self, method: str, path: str, headers: dict, body: Any) -> dict:
        """Handle an incoming HTTP request."""
        token = headers.get("Authorization", "").replace("Bearer ", "")
        if path != "/login" and not self.auth.validate_token(token):
            return {"status": 401, "body": {"error": "Unauthorized"}}
        handler = self._routes.get(path)
        if not handler:
            return {"status": 404, "body": {"error": "Not found"}}
        try:
            result = handler(method=method, body=body, headers=headers)
            return {"status": 200, "body": result}
        except Exception as e:
            return {"status": 500, "body": {"error": str(e)}}

    def login(self, username: str, password: str) -> dict:
        """Handle login request."""
        token = self.auth.authenticate(username, password)
        if token:
            return {"token": token}
        return {"error": "Invalid credentials"}

def create_app(config: dict) -> APIServer:
    """Create and configure the API server."""
    auth = create_auth_manager(config)
    db = DatabasePool(config.get("database_url", "sqlite:///app.db"))
    return APIServer(auth, db)
''')

    (src / "utils.py").write_text('''"""Utility functions."""
import hashlib
import time
from typing import Any, Callable, TypeVar
from functools import wraps

T = TypeVar("T")

def retry(max_attempts: int = 3, delay: float = 1.0):
    """Decorator for retrying failed operations."""
    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @wraps(func)
        def wrapper(*args, **kwargs) -> T:
            last_error = None
            for attempt in range(max_attempts):
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    last_error = e
                    if attempt < max_attempts - 1:
                        time.sleep(delay * (attempt + 1))
            raise last_error
        return wrapper
    return decorator

def compute_hash(content: str) -> str:
    """Compute SHA-256 hash of content."""
    return hashlib.sha256(content.encode()).hexdigest()

def chunk_list(items: list, chunk_size: int) -> list[list]:
    """Split a list into chunks of specified size."""
    return [items[i:i + chunk_size] for i in range(0, len(items), chunk_size)]

class Timer:
    """Context manager for timing operations."""
    def __init__(self):
        self.elapsed = 0.0
    def __enter__(self):
        self._start = time.perf_counter()
        return self
    def __exit__(self, *args):
        self.elapsed = time.perf_counter() - self._start
''')

    return tmp_dir


async def run_benchmark() -> dict:
    """Run the full token efficiency benchmark."""
    import tempfile

    tmp_dir = Path(tempfile.mkdtemp(prefix="ctx_bench_"))
    storage_dir = tmp_dir / ".contextro"
    storage_dir.mkdir()

    codebase = create_test_codebase(tmp_dir)

    metrics = {
        "timestamp": time.time(),
        "tokenizer": tokenizer_metadata(),
        "tokens_per_search": 0,
        "tokens_per_explain": 0,
        "tokens_per_find_symbol": 0,
        "tokens_per_find_callers": 0,
        "tokens_per_status": 0,
        "total_output_tokens": 0,
        "search_results_count": 0,
        "cache_hit_rate": 0.0,
        "workflow_tool_calls": 0,
        "sandbox_rate": 0.0,
        "preview_coverage": 0.0,
    }

    with benchmark_session(storage_dir, dims=384) as (mcp, mock_svc, _server_module):
        from contextro_mcp.state import get_state

        # 1. Index
        index_result = await index_codebase(mcp, _server_module, str(codebase / "src"))
        index_tokens = estimate_tokens(index_result)
        metrics["total_output_tokens"] += index_tokens
        metrics["workflow_tool_calls"] += 1

        # Patch vector engine for search
        state = get_state()
        sync_vector_engine(state, mock_svc)

        # 2. Status
        status_result = await call_tool(mcp, "status")
        status_tokens = estimate_tokens(status_result)
        metrics["tokens_per_status"] = status_tokens
        metrics["total_output_tokens"] += status_tokens
        metrics["workflow_tool_calls"] += 1

        # 3. Search queries (simulate a typical session)
        search_queries = [
            "authentication login",
            "database connection pool",
            "retry logic error handling",
            "user management API",
            "hash password security",
        ]

        search_token_total = 0
        sandboxed_searches = 0
        preview_ratio_sum = 0.0
        for q in search_queries:
            result = await call_tool(mcp, "search", {"query": q, "limit": 10})
            tokens = estimate_tokens(result)
            search_token_total += tokens
            metrics["total_output_tokens"] += tokens
            metrics["workflow_tool_calls"] += 1
            metrics["search_results_count"] += result.get("total", 0)
            if result.get("sandboxed"):
                sandboxed_searches += 1
            full_total = result.get("full_total") or result.get("total") or 0
            shown_total = result.get("total", 0)
            if full_total > 0:
                preview_ratio_sum += shown_total / full_total

        metrics["tokens_per_search"] = search_token_total // len(search_queries)
        metrics["sandbox_rate"] = round(sandboxed_searches / len(search_queries), 4)
        metrics["preview_coverage"] = round(preview_ratio_sum / len(search_queries), 4)

        # 4. Find symbol
        symbol_queries = ["AuthManager", "DatabasePool", "retry"]
        symbol_token_total = 0
        for s in symbol_queries:
            result = await call_tool(mcp, "find_symbol", {"name": s})
            tokens = estimate_tokens(result)
            symbol_token_total += tokens
            metrics["total_output_tokens"] += tokens
            metrics["workflow_tool_calls"] += 1

        metrics["tokens_per_find_symbol"] = symbol_token_total // len(symbol_queries)

        # 5. Find callers
        caller_queries = ["authenticate", "execute", "_get_connection"]
        caller_token_total = 0
        for c in caller_queries:
            result = await call_tool(mcp, "find_callers", {"symbol_name": c})
            tokens = estimate_tokens(result)
            caller_token_total += tokens
            metrics["total_output_tokens"] += tokens
            metrics["workflow_tool_calls"] += 1

        metrics["tokens_per_find_callers"] = caller_token_total // len(caller_queries)

        # 6. Explain
        try:
            explain_result = await call_tool(mcp, "explain", {"symbol_name": "AuthManager"})
            explain_tokens = estimate_tokens(explain_result)
            metrics["tokens_per_explain"] = explain_tokens
            metrics["total_output_tokens"] += explain_tokens
            metrics["workflow_tool_calls"] += 1
        except Exception:
            metrics["tokens_per_explain"] = 0

        # 7. Repeat search (for cache hit measurement)
        repeat_queries = ["authentication login", "database connection pool"]
        for q in repeat_queries:
            result = await call_tool(mcp, "search", {"query": q, "limit": 10})
            tokens = estimate_tokens(result)
            metrics["total_output_tokens"] += tokens
            metrics["workflow_tool_calls"] += 1

        # Check cache hit rate if available
        if hasattr(state, "_query_cache"):
            cache = state._query_cache
            if hasattr(cache, "hits") and hasattr(cache, "misses"):
                total = cache.hits + cache.misses
                metrics["cache_hit_rate"] = cache.hits / total if total > 0 else 0.0

        # Measure TOON savings on the collected results
        # Re-run a subset and measure with TOON encoding
        toon_total = 0
        toon_total += estimate_tokens_toon(status_result)
        for q in search_queries[:3]:
            result = await call_tool(mcp, "search", {"query": q, "limit": 10})
            toon_total += estimate_tokens_toon(result)
        # Scale up proportionally
        json_subset = estimate_tokens(status_result)
        for q in search_queries[:3]:
            result = await call_tool(mcp, "search", {"query": q, "limit": 10})
            json_subset += estimate_tokens(result)
        if json_subset > 0:
            toon_ratio = toon_total / json_subset
            metrics["total_output_tokens_toon"] = int(metrics["total_output_tokens"] * toon_ratio)
            metrics["toon_reduction_pct"] = round((1 - toon_ratio) * 100, 1)

    # Cleanup
    import shutil

    shutil.rmtree(tmp_dir, ignore_errors=True)

    return metrics


def main():
    """Run benchmark and print results."""
    import asyncio

    print("=" * 60)
    print("Contextro Token Efficiency Benchmark")
    print("=" * 60)

    metrics = asyncio.run(run_benchmark())

    print(f"\n{'Metric':<30} {'Value':>15}")
    print("-" * 47)
    for key, value in sorted(metrics.items()):
        if key == "timestamp":
            continue
        if isinstance(value, float):
            print(f"{key:<30} {value:>15.4f}")
        elif isinstance(value, dict):
            print(f"{key:<30} {json.dumps(value, sort_keys=True):>15}")
        else:
            print(f"{key:<30} {value:>15}")

    print("-" * 47)
    print(f"{'TOTAL OUTPUT TOKENS':<30} {metrics['total_output_tokens']:>15}")
    print(f"{'TOOL CALLS':<30} {metrics['workflow_tool_calls']:>15}")
    avg_tokens = metrics["total_output_tokens"] // max(1, metrics["workflow_tool_calls"])
    print(f"{'AVG TOKENS/CALL':<30} {avg_tokens:>15}")

    # TOON comparison
    if "total_output_tokens_toon" in metrics:
        toon_savings = metrics["total_output_tokens"] - metrics["total_output_tokens_toon"]
        pct = (
            (toon_savings / metrics["total_output_tokens"]) * 100
            if metrics["total_output_tokens"] > 0
            else 0
        )
        print(f"{'TOON TOKENS':<30} {metrics['total_output_tokens_toon']:>15}")
        print(f"{'TOON SAVINGS':<30} {f'-{toon_savings} ({pct:.1f}%)':>15}")

    print("=" * 60)

    # Write results to JSON for comparison
    results_path = Path(__file__).parent / "token_benchmark_results.json"
    with open(results_path, "w") as f:
        json.dump(metrics, f, indent=2)
    print(f"\nResults saved to: {results_path}")

    return metrics


if __name__ == "__main__":
    main()
