#!/usr/bin/env python3
"""
Contextro Paired Study Harness — MCP vs Lexical Baseline
Replicates the methodology from the Contextro paper:
- 20 realistic coding tasks run through both conditions
- Measures: tool calls, tokens consumed, files read, latency
- Condition A: Lexical baseline (grep/find/read files)
- Condition B: Contextro MCP (search/find_symbol/explain/impact)
"""

import json
import os
import subprocess
import time
import sys
from pathlib import Path

PLATFORM_PATH = "/Users/japneetkalkat/platform/apps"
CONTEXTRO_BIN = str(Path(__file__).parent / "target/release/contextro")

# 20 realistic coding tasks that an agent would perform
TASKS = [
    {"id": 1, "query": "authentication flow", "type": "search", "description": "Find how authentication works"},
    {"id": 2, "query": "database connection", "type": "search", "description": "Find database connection logic"},
    {"id": 3, "query": "payment processing", "type": "search", "description": "Find payment processing code"},
    {"id": 4, "query": "user registration", "type": "search", "description": "Find user registration flow"},
    {"id": 5, "query": "error handling middleware", "type": "search", "description": "Find error handling patterns"},
    {"id": 6, "query": "API rate limiting", "type": "search", "description": "Find rate limiting implementation"},
    {"id": 7, "query": "file upload", "type": "search", "description": "Find file upload handling"},
    {"id": 8, "query": "email notification", "type": "search", "description": "Find email sending logic"},
    {"id": 9, "query": "caching strategy", "type": "search", "description": "Find caching implementation"},
    {"id": 10, "query": "websocket connection", "type": "search", "description": "Find websocket handling"},
    {"id": 11, "query": "useAuth", "type": "find_symbol", "description": "Find useAuth hook definition"},
    {"id": 12, "query": "handleSubmit", "type": "find_symbol", "description": "Find form submission handler"},
    {"id": 13, "query": "fetchUser", "type": "find_symbol", "description": "Find user fetching function"},
    {"id": 14, "query": "validateInput", "type": "find_symbol", "description": "Find input validation"},
    {"id": 15, "query": "createOrder", "type": "find_symbol", "description": "Find order creation logic"},
    {"id": 16, "query": "router", "type": "find_symbol", "description": "Find routing configuration"},
    {"id": 17, "query": "middleware", "type": "find_symbol", "description": "Find middleware definitions"},
    {"id": 18, "query": "config", "type": "find_symbol", "description": "Find configuration setup"},
    {"id": 19, "query": "how does the app handle state management", "type": "search", "description": "Understand state management"},
    {"id": 20, "query": "what happens when a request fails", "type": "search", "description": "Understand error recovery"},
]


def run_contextro_condition(tasks):
    """Run tasks through Contextro MCP (Condition B)."""
    results = []
    
    # Build MCP messages
    messages = [
        '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"study","version":"1.0"}}}',
        '{"jsonrpc":"2.0","method":"notifications/initialized"}',
        f'{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"index","arguments":{{"path":"{PLATFORM_PATH}"}}}}}}',
    ]
    
    # Add task tool calls
    for i, task in enumerate(tasks):
        msg_id = i + 10
        if task["type"] == "search":
            messages.append(f'{{"jsonrpc":"2.0","id":{msg_id},"method":"tools/call","params":{{"name":"search","arguments":{{"query":"{task["query"]}","limit":5}}}}}}')
        elif task["type"] == "find_symbol":
            messages.append(f'{{"jsonrpc":"2.0","id":{msg_id},"method":"tools/call","params":{{"name":"find_symbol","arguments":{{"name":"{task["query"]}","exact":false}}}}}}')
    
    # Run all through the binary
    input_data = "\n".join(messages) + "\n"
    
    start = time.time()
    proc = subprocess.run(
        [CONTEXTRO_BIN],
        input=input_data,
        capture_output=True,
        text=True,
        timeout=30,
    )
    total_time = time.time() - start
    
    # Parse responses
    total_tokens = 0
    tool_calls = len(tasks) + 1  # index + task calls
    
    for line in proc.stdout.strip().split("\n"):
        if not line:
            continue
        try:
            data = json.loads(line)
            rid = data.get("id")
            if rid and rid >= 10:
                content = data.get("result", {}).get("content", [{}])[0].get("text", "")
                total_tokens += len(content) // 4  # approximate tokens
                task_idx = rid - 10
                if task_idx < len(tasks):
                    results.append({
                        "task_id": tasks[task_idx]["id"],
                        "description": tasks[task_idx]["description"],
                        "response_tokens": len(content) // 4,
                        "has_results": "error" not in content.lower(),
                    })
        except (json.JSONDecodeError, IndexError):
            pass
    
    return {
        "condition": "Contextro MCP",
        "total_time_ms": round(total_time * 1000),
        "tool_calls": tool_calls,
        "total_tokens": total_tokens,
        "files_read": 0,  # MCP doesn't read files directly
        "tasks_completed": len(results),
        "tasks_with_results": sum(1 for r in results if r["has_results"]),
        "results": results,
    }


def run_lexical_condition(tasks):
    """Run tasks through lexical baseline (grep/find - Condition A)."""
    results = []
    total_tokens = 0
    files_read = 0
    tool_calls = 0
    
    start = time.time()
    
    for task in tasks:
        query = task["query"]
        task_start = time.time()
        
        # Simulate what an agent does WITHOUT MCP: grep for the query
        try:
            # Step 1: grep for the query (agent would do this)
            proc = subprocess.run(
                ["grep", "-rl", query.split()[0], PLATFORM_PATH],
                capture_output=True, text=True, timeout=5,
            )
            tool_calls += 1
            matching_files = [f for f in proc.stdout.strip().split("\n") if f][:5]
            files_read += len(matching_files)
            
            # Step 2: Read each matching file (agent would do this)
            file_content_tokens = 0
            for f in matching_files[:3]:  # Agent typically reads 3-5 files
                try:
                    content = Path(f).read_text(errors="ignore")
                    file_content_tokens += len(content) // 4
                    tool_calls += 1
                except:
                    pass
            
            total_tokens += file_content_tokens
            task_time = time.time() - task_start
            
            results.append({
                "task_id": task["id"],
                "description": task["description"],
                "response_tokens": file_content_tokens,
                "has_results": len(matching_files) > 0,
                "files_read": len(matching_files),
            })
        except subprocess.TimeoutExpired:
            tool_calls += 1
            results.append({
                "task_id": task["id"],
                "description": task["description"],
                "response_tokens": 0,
                "has_results": False,
                "files_read": 0,
            })
    
    total_time = time.time() - start
    
    return {
        "condition": "Lexical Baseline (grep)",
        "total_time_ms": round(total_time * 1000),
        "tool_calls": tool_calls,
        "total_tokens": total_tokens,
        "files_read": files_read,
        "tasks_completed": len(results),
        "tasks_with_results": sum(1 for r in results if r["has_results"]),
        "results": results,
    }


def print_comparison(contextro, lexical):
    """Print the paired study comparison table."""
    print("\n" + "=" * 70)
    print("  PAIRED STUDY: Contextro MCP vs Lexical Baseline")
    print(f"  Codebase: {PLATFORM_PATH}")
    print(f"  Tasks: {len(TASKS)}")
    print("=" * 70)
    
    print(f"\n{'Metric':<30} {'Lexical':<15} {'Contextro':<15} {'Reduction':<15}")
    print("-" * 70)
    
    def pct(a, b):
        if a == 0: return "N/A"
        return f"{((a - b) / a) * 100:.0f}%"
    
    print(f"{'Total time (ms)':<30} {lexical['total_time_ms']:<15} {contextro['total_time_ms']:<15} {pct(lexical['total_time_ms'], contextro['total_time_ms']):<15}")
    print(f"{'Tool calls':<30} {lexical['tool_calls']:<15} {contextro['tool_calls']:<15} {pct(lexical['tool_calls'], contextro['tool_calls']):<15}")
    print(f"{'Total tokens consumed':<30} {lexical['total_tokens']:<15} {contextro['total_tokens']:<15} {pct(lexical['total_tokens'], contextro['total_tokens']):<15}")
    print(f"{'Files read':<30} {lexical['files_read']:<15} {contextro['files_read']:<15} {pct(lexical['files_read'], contextro['files_read']):<15}")
    print(f"{'Tasks with results':<30} {lexical['tasks_with_results']:<15} {contextro['tasks_with_results']:<15} {'—':<15}")
    
    print("\n" + "=" * 70)
    print("  CONCLUSION")
    print("=" * 70)
    
    token_reduction = ((lexical['total_tokens'] - contextro['total_tokens']) / max(lexical['total_tokens'], 1)) * 100
    speed_improvement = lexical['total_time_ms'] / max(contextro['total_time_ms'], 1)
    call_reduction = ((lexical['tool_calls'] - contextro['tool_calls']) / max(lexical['tool_calls'], 1)) * 100
    
    print(f"  Token reduction:    {token_reduction:.0f}%")
    print(f"  Speed improvement:  {speed_improvement:.1f}x faster")
    print(f"  Tool call reduction: {call_reduction:.0f}%")
    print(f"  Files read saved:   {lexical['files_read']} → {contextro['files_read']}")
    print("=" * 70)


if __name__ == "__main__":
    print("Running paired study on /Users/japneetkalkat/platform/apps...")
    print(f"Binary: {CONTEXTRO_BIN}")
    print(f"Tasks: {len(TASKS)}")
    print()
    
    print("[1/2] Running Condition A: Lexical Baseline (grep + file reads)...")
    lexical = run_lexical_condition(TASKS)
    print(f"  Done: {lexical['total_time_ms']}ms, {lexical['tool_calls']} calls, {lexical['total_tokens']} tokens")
    
    print("[2/2] Running Condition B: Contextro MCP...")
    contextro = run_contextro_condition(TASKS)
    print(f"  Done: {contextro['total_time_ms']}ms, {contextro['tool_calls']} calls, {contextro['total_tokens']} tokens")
    
    print_comparison(contextro, lexical)
    
    # Save results
    output = {"contextro": contextro, "lexical": lexical, "tasks": TASKS}
    with open("paired_study_results.json", "w") as f:
        json.dump(output, f, indent=2)
    print(f"\nFull results saved to paired_study_results.json")
