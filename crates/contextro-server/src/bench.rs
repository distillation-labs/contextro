use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use contextro_core::traits::Parser;
use rmcp::model::{CallToolResult, RawContent};
use serde_json::{json, Value};

#[allow(dead_code)]
#[path = "main.rs"]
mod server;

use server::ContextroServer;

const ITERATIONS: usize = 25;
const INDEX_ITERATIONS: usize = 3;
const REPO_MUTATION_ITERATIONS: usize = 10;

fn main() {
    let codebase = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: contextro-bench <path-to-codebase>");
        std::process::exit(1);
    });
    let platform_path = Path::new(&codebase);
    if !platform_path.is_dir() {
        eprintln!("ERROR: '{}' is not a directory", codebase);
        std::process::exit(1);
    }

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  CONTEXTRO RUST MCP — PERFORMANCE BENCHMARK                 ║");
    println!(
        "║  Target: {:<52}║",
        codebase.chars().take(52).collect::<String>()
    );
    println!("╠══════════════════════════════════════════════════════════════╣");

    let storage_dir = temp_storage_dir("bench-storage");
    let server = new_bench_server(&storage_dir);

    let index_start = Instant::now();
    let index_result = parse_tool_json(server.dispatch("index", json!({"path": codebase})));
    let idx_time = index_start.elapsed();
    ensure_success("index", &index_result);

    println!("║  INDEXING                                                    ║");
    println!(
        "║  Files: {:>5}  Symbols: {:>6}  Chunks: {:>6}               ║",
        index_result["total_files"].as_u64().unwrap_or(0),
        index_result["total_symbols"].as_u64().unwrap_or(0),
        index_result["total_chunks"].as_u64().unwrap_or(0)
    );
    println!(
        "║  Time: {:>8.2}ms                                            ║",
        idx_time.as_secs_f64() * 1000.0
    );
    println!(
        "║  Mode: {:<52}║",
        index_result["index_mode"]
            .as_str()
            .unwrap_or("unknown")
            .chars()
            .take(52)
            .collect::<String>()
    );
    println!(
        "║  Phases: discover {:>5.1}ms  parse {:>5.1}ms  chunk {:>5.1}ms      ║",
        index_result["discover_ms"].as_f64().unwrap_or(0.0),
        index_result["parse_ms"].as_f64().unwrap_or(0.0),
        index_result["chunk_ms"].as_f64().unwrap_or(0.0)
    );
    println!(
        "║  Build: graph {:>5.1}ms  bm25 {:>5.1}ms  vector {:>5.1}ms        ║",
        index_result["graph_ms"].as_f64().unwrap_or(0.0),
        index_result["bm25_ms"].as_f64().unwrap_or(0.0),
        index_result["vector_ms"].as_f64().unwrap_or(0.0)
    );
    println!(
        "║  Final: scope {:>5.1}ms  docs {:>5.1}ms  total {:>5.1}ms         ║",
        index_result["scope_ms"].as_f64().unwrap_or(0.0),
        index_result["knowledge_ms"].as_f64().unwrap_or(0.0),
        index_result["request_ms"].as_f64().unwrap_or(0.0)
    );
    println!(
        "║  Persist: snap {:>5.1}ms  prewarm {:>5.1}ms                     ║",
        index_result["snapshot_ms"].as_f64().unwrap_or(0.0),
        index_result["prewarm_ms"].as_f64().unwrap_or(0.0)
    );
    let total_symbols = index_result["total_symbols"].as_f64().unwrap_or(0.0);
    let symbols_per_sec = if idx_time.is_zero() {
        0.0
    } else {
        total_symbols / idx_time.as_secs_f64()
    };
    println!(
        "║  Symbols/sec: {:>10.0}                                      ║",
        symbols_per_sec
    );
    println!(
        "║  Graph: {:>5} nodes, {:>5} edges ({:.1}ms)                  ║",
        index_result["graph_nodes"].as_u64().unwrap_or(0),
        index_result["graph_relationships"].as_u64().unwrap_or(0),
        idx_time.as_secs_f64() * 1000.0
    );

    let fixture = BenchmarkFixture::from_codebase(platform_path);
    let retrieve_ref = seed_retrieve_ref(&server);
    let extra_repo = temp_fixture_repo();
    let tool_cases = build_tool_cases(&fixture, &codebase, retrieve_ref.as_deref());
    let public_tools = ContextroServer::tool_definitions();

    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  PUBLIC MCP TOOL LATENCY (25 iterations each)               ║");

    let mut tool_benchmarks = Vec::with_capacity(tool_cases.len() + 4);

    let index_benchmark = bench_cold_index_tool(&codebase);
    let cold_index_ms = index_benchmark.avg_ms;
    print_tool_benchmark(&index_benchmark);
    tool_benchmarks.push(index_benchmark);

    for case in &tool_cases {
        let benchmark = bench_tool_case(&server, case, ITERATIONS);
        print_tool_benchmark(&benchmark);
        tool_benchmarks.push(benchmark);
    }

    let repo_add_benchmark = bench_repo_add_tool(&extra_repo);
    print_tool_benchmark(&repo_add_benchmark);
    tool_benchmarks.push(repo_add_benchmark);

    let repo_remove_idle_benchmark = bench_repo_remove_tool(&codebase, &extra_repo, false);
    print_tool_benchmark(&repo_remove_idle_benchmark);
    tool_benchmarks.push(repo_remove_idle_benchmark);

    let repo_remove_active_benchmark = bench_repo_remove_tool(&codebase, &extra_repo, true);
    print_tool_benchmark(&repo_remove_active_benchmark);
    tool_benchmarks.push(repo_remove_active_benchmark);

    let benchmarked_tools: BTreeSet<&str> = tool_benchmarks
        .iter()
        .map(|benchmark| benchmark.tool_name)
        .collect();
    let missing_tools: Vec<String> = public_tools
        .iter()
        .map(|tool| tool.name.to_string())
        .filter(|name| !benchmarked_tools.contains(name.as_str()))
        .collect();

    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  TOOL COVERAGE                                               ║");
    println!(
        "║  Benchmarked tools: {:>2}/{:<2}                                   ║",
        benchmarked_tools.len(),
        public_tools.len()
    );
    println!(
        "║  Benchmark cases: {:>2}                                         ║",
        tool_benchmarks.len()
    );
    if missing_tools.is_empty() {
        println!("║  Missing: none                                              ║");
    } else {
        for line in wrap_list(&missing_tools, 52) {
            println!("║  Missing: {:<52}║", line);
        }
    }

    let mut ranked = tool_benchmarks.clone();
    ranked.sort_by(|a, b| {
        b.avg_ms
            .partial_cmp(&a.avg_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  HOTTEST TOOLS                                               ║");
    for benchmark in ranked.into_iter().take(5) {
        println!(
            "║  {:15} {:>7.2}ms avg  {:<24}║",
            benchmark.display_name,
            benchmark.avg_ms,
            benchmark.notes.chars().take(24).collect::<String>()
        );
    }

    let search_avg = tool_benchmarks
        .iter()
        .find(|benchmark| benchmark.tool_name == "search")
        .map(|benchmark| benchmark.avg_ms * 1000.0)
        .unwrap_or_default();
    let ops_per_sec = mixed_workload_ops_per_sec(&server, &tool_cases);

    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  SUMMARY vs TARGETS                                         ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Metric          │ Target    │ Actual    │ Status           ║");
    println!("╟──────────────────┼───────────┼───────────┼──────────────────╢");
    let first_index_ms = idx_time.as_secs_f64() * 1000.0;
    let idx_status = if cold_index_ms <= 40.0 {
        "✓ PASS"
    } else {
        "✗ NEEDS WORK"
    };
    println!(
        "║  Cold index avg  │ ≤40.0ms   │ {:>6.1}ms  │ {:16}║",
        cold_index_ms, idx_status
    );
    let first_index_label = match index_result["index_mode"].as_str().unwrap_or("unknown") {
        "fresh" => "First fresh run",
        "restored" => "First restore",
        _ => "First index run",
    };
    let first_index_status = if first_index_ms <= 40.0 {
        "✓ PASS"
    } else {
        "✗ WARMUP COST"
    };
    println!(
        "║  {:16} │ info      │ {:>6.1}ms  │ {:16}║",
        first_index_label, first_index_ms, first_index_status
    );
    let search_status = if search_avg <= 137.0 {
        "✓ PASS"
    } else {
        "✗ NEEDS WORK"
    };
    println!(
        "║  Search latency  │ ≤137µs    │ {:>6.1}µs  │ {:16}║",
        search_avg, search_status
    );
    let tp_status = if ops_per_sec >= 500.0 {
        "✓ PASS"
    } else {
        "✗ NEEDS WORK"
    };
    println!(
        "║  MCP throughput  │ ≥500/s    │ {:>7.0}/s │ {:16}║",
        ops_per_sec, tp_status
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    let _ = std::fs::remove_dir_all(storage_dir);
    let _ = std::fs::remove_dir_all(extra_repo);
}

#[derive(Clone)]
struct ToolCase {
    display_name: &'static str,
    tool_name: &'static str,
    args: Value,
    notes: &'static str,
    allow_error: bool,
}

#[derive(Clone)]
struct ToolBenchmark {
    display_name: &'static str,
    tool_name: &'static str,
    avg_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    notes: &'static str,
}

struct BenchmarkFixture {
    code_dir_rel: String,
    code_file_rel: String,
    symbol_exact: String,
    commit_query: String,
}

impl BenchmarkFixture {
    fn from_codebase(platform_path: &Path) -> Self {
        let files = contextro_indexing::discover_files(
            platform_path,
            &contextro_config::Settings::default(),
        );
        let code_file = files
            .iter()
            .find(|path| {
                matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("rs" | "py" | "ts" | "tsx" | "js" | "java" | "go")
                )
            })
            .cloned()
            .unwrap_or_else(|| {
                files
                    .first()
                    .cloned()
                    .expect("codebase contains indexable files")
            });

        let parser = contextro_parsing::TreeSitterParser::new();
        let parsed = parser
            .parse_file(code_file.to_string_lossy().as_ref())
            .expect("parse benchmark fixture file");
        let symbol = parsed
            .symbols
            .iter()
            .find(|symbol| !symbol.name.is_empty())
            .expect("fixture file contains symbols");
        let code_dir_rel = code_file
            .parent()
            .and_then(|parent| parent.strip_prefix(platform_path).ok())
            .map(path_to_string)
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| ".".into());
        let code_file_rel = code_file
            .strip_prefix(platform_path)
            .map(path_to_string)
            .unwrap_or_else(|_| code_file.to_string_lossy().to_string());

        Self {
            code_dir_rel,
            code_file_rel,
            symbol_exact: symbol.name.clone(),
            commit_query: symbol.name.to_ascii_lowercase(),
        }
    }
}

fn build_tool_cases(
    fixture: &BenchmarkFixture,
    codebase: &str,
    retrieve_ref: Option<&str>,
) -> Vec<ToolCase> {
    let knowledge_path = Path::new(codebase).join("README.md");
    let knowledge_value = if knowledge_path.exists() {
        knowledge_path.to_string_lossy().to_string()
    } else {
        "Contextro benchmark note".to_string()
    };
    let retrieve_ref = retrieve_ref.unwrap_or("missing-bench-ref");

    vec![
        ToolCase {
            display_name: "status",
            tool_name: "status",
            args: json!({}),
            notes: "server status",
            allow_error: false,
        },
        ToolCase {
            display_name: "health",
            tool_name: "health",
            args: json!({}),
            notes: "health report",
            allow_error: false,
        },
        ToolCase {
            display_name: "search",
            tool_name: "search",
            args: json!({"query": fixture.symbol_exact, "limit": 5, "mode": "hybrid"}),
            notes: "hybrid search",
            allow_error: false,
        },
        ToolCase {
            display_name: "find_symbol",
            tool_name: "find_symbol",
            args: json!({"symbol_name": fixture.symbol_exact, "exact": true}),
            notes: "exact symbol lookup",
            allow_error: false,
        },
        ToolCase {
            display_name: "find_callers",
            tool_name: "find_callers",
            args: json!({"symbol_name": fixture.symbol_exact, "limit": 10}),
            notes: "graph callers",
            allow_error: false,
        },
        ToolCase {
            display_name: "find_callees",
            tool_name: "find_callees",
            args: json!({"symbol_name": fixture.symbol_exact, "limit": 10}),
            notes: "graph callees",
            allow_error: false,
        },
        ToolCase {
            display_name: "explain",
            tool_name: "explain",
            args: json!({"symbol_name": fixture.symbol_exact}),
            notes: "symbol explanation",
            allow_error: false,
        },
        ToolCase {
            display_name: "impact",
            tool_name: "impact",
            args: json!({"symbol_name": fixture.symbol_exact, "max_depth": 3}),
            notes: "impact analysis",
            allow_error: false,
        },
        ToolCase {
            display_name: "overview",
            tool_name: "overview",
            args: json!({}),
            notes: "project overview",
            allow_error: false,
        },
        ToolCase {
            display_name: "architecture",
            tool_name: "architecture",
            args: json!({"limit": 5}),
            notes: "architecture hubs",
            allow_error: false,
        },
        ToolCase {
            display_name: "analyze",
            tool_name: "analyze",
            args: json!({"path": fixture.code_dir_rel, "top_n": 5}),
            notes: "hotspot analysis",
            allow_error: false,
        },
        ToolCase {
            display_name: "focus",
            tool_name: "focus",
            args: json!({"path": fixture.code_dir_rel}),
            notes: "file focus",
            allow_error: false,
        },
        ToolCase {
            display_name: "dead_code",
            tool_name: "dead_code",
            args: json!({"path": fixture.code_dir_rel, "limit": 5}),
            notes: "dead code heuristic",
            allow_error: false,
        },
        ToolCase {
            display_name: "circular_deps",
            tool_name: "circular_dependencies",
            args: json!({}),
            notes: "cycle detection",
            allow_error: false,
        },
        ToolCase {
            display_name: "test_coverage",
            tool_name: "test_coverage_map",
            args: json!({}),
            notes: "coverage heuristic",
            allow_error: false,
        },
        ToolCase {
            display_name: "code",
            tool_name: "code",
            args: json!({"operation": "get_document_symbols", "path": fixture.code_file_rel}),
            notes: "document symbols",
            allow_error: false,
        },
        ToolCase {
            display_name: "remember",
            tool_name: "remember",
            args: json!({"content": "Contextro benchmark memory", "memory_type": "note", "tags": ["bench"]}),
            notes: "memory write",
            allow_error: false,
        },
        ToolCase {
            display_name: "recall",
            tool_name: "recall",
            args: json!({"query": "benchmark", "limit": 5}),
            notes: "memory recall",
            allow_error: false,
        },
        ToolCase {
            display_name: "tags",
            tool_name: "tags",
            args: json!({}),
            notes: "memory tags",
            allow_error: false,
        },
        ToolCase {
            display_name: "forget",
            tool_name: "forget",
            args: json!({"tags": "bench"}),
            notes: "memory delete by tag",
            allow_error: false,
        },
        ToolCase {
            display_name: "knowledge",
            tool_name: "knowledge",
            args: json!({"command": "add", "name": "bench-doc", "value": knowledge_value}),
            notes: "knowledge add",
            allow_error: false,
        },
        ToolCase {
            display_name: "compact",
            tool_name: "compact",
            args: json!({"content": "Contextro benchmark compact payload"}),
            notes: "session archive",
            allow_error: false,
        },
        ToolCase {
            display_name: "session_snap",
            tool_name: "session_snapshot",
            args: json!({"limit": 5}),
            notes: "session events",
            allow_error: false,
        },
        ToolCase {
            display_name: "restore",
            tool_name: "restore",
            args: json!({}),
            notes: "restore status",
            allow_error: false,
        },
        ToolCase {
            display_name: "retrieve",
            tool_name: "retrieve",
            args: json!({"ref_id": retrieve_ref}),
            notes: "archive lookup",
            allow_error: false,
        },
        ToolCase {
            display_name: "commit_search",
            tool_name: "commit_search",
            args: json!({"query": fixture.commit_query, "limit": 5}),
            notes: "git commit search",
            allow_error: false,
        },
        ToolCase {
            display_name: "commit_history",
            tool_name: "commit_history",
            args: json!({"limit": 5}),
            notes: "git history",
            allow_error: false,
        },
        ToolCase {
            display_name: "repo_status",
            tool_name: "repo_status",
            args: json!({}),
            notes: "repo registry list",
            allow_error: false,
        },
        ToolCase {
            display_name: "audit",
            tool_name: "audit",
            args: json!({}),
            notes: "quality audit",
            allow_error: false,
        },
        ToolCase {
            display_name: "docs_bundle",
            tool_name: "docs_bundle",
            args: json!({"output_dir": temp_output_dir("bench-docs")}),
            notes: "docs bundle",
            allow_error: false,
        },
        ToolCase {
            display_name: "sidecar_export",
            tool_name: "sidecar_export",
            args: json!({"path": fixture.code_dir_rel, "output_dir": temp_output_dir("bench-sidecars")}),
            notes: "sidecar export",
            allow_error: false,
        },
        ToolCase {
            display_name: "skill_prompt",
            tool_name: "skill_prompt",
            args: json!({}),
            notes: "skill prompt",
            allow_error: false,
        },
        ToolCase {
            display_name: "introspect",
            tool_name: "introspect",
            args: json!({"tool": "search"}),
            notes: "tool introspection",
            allow_error: false,
        },
        ToolCase {
            display_name: "refactor_chk",
            tool_name: "refactor_check",
            args: json!({"symbol_name": fixture.symbol_exact, "max_depth": 3}),
            notes: "pre-refactor analysis",
            allow_error: false,
        },
        ToolCase {
            display_name: "completion_chk",
            tool_name: "completion_check",
            args: json!({
                "claim": "all_callers_updated",
                "symbol_name": fixture.symbol_exact,
                "changed_files": [
                    fixture.code_file_rel,
                    format!("{}/nonexistent.rs", fixture.code_dir_rel),
                ],
            }),
            notes: "refactor completeness",
            allow_error: false,
        },
    ]
}

fn seed_retrieve_ref(server: &ContextroServer) -> Option<String> {
    let result =
        parse_tool_json(server.dispatch("compact", json!({"content": "seed retrieve payload"})));
    result
        .get("ref_id")
        .and_then(Value::as_str)
        .map(String::from)
}

fn temp_fixture_repo() -> PathBuf {
    let root = temp_path("bench-repo-remove");
    std::fs::create_dir_all(root.join("src")).expect("create fixture repo src");
    std::fs::write(root.join("src/lib.rs"), "pub fn bench_fixture() {}\n")
        .expect("write fixture repo file");
    root
}

fn bench_tool_case(server: &ContextroServer, case: &ToolCase, iterations: usize) -> ToolBenchmark {
    let mut times = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let result = parse_tool_json(server.dispatch(case.tool_name, case.args.clone()));
        let elapsed = start.elapsed();
        ensure_case_result(case, &result);
        times.push(elapsed);
    }
    times.sort();

    make_tool_benchmark(case.display_name, case.tool_name, case.notes, &times)
}

fn bench_cold_index_tool(codebase: &str) -> ToolBenchmark {
    let mut times = Vec::with_capacity(INDEX_ITERATIONS);
    for _ in 0..INDEX_ITERATIONS {
        let storage_dir = temp_storage_dir("bench-index");
        let project_storage_dir = contextro_config::project_storage_dir(codebase);
        let snapshot_path = project_storage_dir.join("repo-snapshot.json");
        let hashes_path = project_storage_dir.join("file_hashes.json");
        let saved_snapshot = std::fs::read(&snapshot_path).ok();
        let saved_hashes = std::fs::read(&hashes_path).ok();
        let _ = std::fs::remove_file(project_storage_dir.join("repo-snapshot.json"));
        let _ = std::fs::remove_file(project_storage_dir.join("file_hashes.json"));
        let server = new_bench_server(&storage_dir);
        let start = Instant::now();
        let result = parse_tool_json(server.dispatch("index", json!({"path": codebase})));
        let elapsed = start.elapsed();
        ensure_success("index", &result);
        if let Some(bytes) = saved_snapshot {
            let _ = std::fs::create_dir_all(&project_storage_dir);
            let _ = std::fs::write(&snapshot_path, bytes);
        } else {
            let _ = std::fs::remove_file(&snapshot_path);
        }
        if let Some(bytes) = saved_hashes {
            let _ = std::fs::create_dir_all(&project_storage_dir);
            let _ = std::fs::write(&hashes_path, bytes);
        } else {
            let _ = std::fs::remove_file(&hashes_path);
        }
        times.push(elapsed);
        let _ = std::fs::remove_dir_all(storage_dir);
    }
    times.sort();
    make_tool_benchmark("index", "index", "cold index", &times)
}

fn bench_repo_add_tool(extra_repo: &Path) -> ToolBenchmark {
    let mut times = Vec::with_capacity(REPO_MUTATION_ITERATIONS);
    let repo_path = extra_repo.to_string_lossy().to_string();
    for _ in 0..REPO_MUTATION_ITERATIONS {
        let storage_dir = temp_storage_dir("bench-repo-add");
        let server = new_bench_server(&storage_dir);
        let start = Instant::now();
        let result = parse_tool_json(server.dispatch("repo_add", json!({"path": repo_path})));
        let elapsed = start.elapsed();
        ensure_success("repo_add", &result);
        times.push(elapsed);
        let _ = std::fs::remove_dir_all(storage_dir);
    }
    times.sort();
    make_tool_benchmark("repo_add", "repo_add", "repo register", &times)
}

fn bench_repo_remove_tool(codebase: &str, extra_repo: &Path, active_scope: bool) -> ToolBenchmark {
    let mut times = Vec::with_capacity(REPO_MUTATION_ITERATIONS);
    let repo_path = extra_repo.to_string_lossy().to_string();
    for _ in 0..REPO_MUTATION_ITERATIONS {
        let storage_dir = temp_storage_dir(if active_scope {
            "bench-repo-remove-active"
        } else {
            "bench-repo-remove-idle"
        });
        let server = new_bench_server(&storage_dir);

        let base_index = parse_tool_json(server.dispatch("index", json!({"path": codebase})));
        ensure_success("index", &base_index);
        let add_result = parse_tool_json(server.dispatch("repo_add", json!({"path": repo_path})));
        ensure_success("repo_add", &add_result);

        if !active_scope {
            let restore_base = parse_tool_json(server.dispatch("index", json!({"path": codebase})));
            ensure_success("index", &restore_base);
        }

        let start = Instant::now();
        let result = parse_tool_json(server.dispatch("repo_remove", json!({"path": repo_path})));
        let elapsed = start.elapsed();
        ensure_success("repo_remove", &result);
        times.push(elapsed);
        let _ = std::fs::remove_dir_all(storage_dir);
    }
    times.sort();

    let display_name = if active_scope {
        "repo_rm_active"
    } else {
        "repo_rm_idle"
    };
    let notes = if active_scope {
        "active scope restore"
    } else {
        "inactive repo remove"
    };
    make_tool_benchmark(display_name, "repo_remove", notes, &times)
}

fn mixed_workload_ops_per_sec(server: &ContextroServer, tool_cases: &[ToolCase]) -> f64 {
    let selected: Vec<&ToolCase> = tool_cases
        .iter()
        .filter(|case| {
            matches!(
                case.tool_name,
                "search" | "find_symbol" | "code" | "overview" | "status" | "session_snapshot" | "completion_check"
            )
        })
        .collect();

    let start = Instant::now();
    let mut ops = 0u64;
    while start.elapsed() < Duration::from_secs(3) {
        for case in &selected {
            let result = parse_tool_json(server.dispatch(case.tool_name, case.args.clone()));
            ensure_case_result(case, &result);
            ops += 1;
        }
    }

    ops as f64 / start.elapsed().as_secs_f64()
}

fn make_tool_benchmark(
    display_name: &'static str,
    tool_name: &'static str,
    notes: &'static str,
    times: &[Duration],
) -> ToolBenchmark {
    let avg_ms = times
        .iter()
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .sum::<f64>()
        / times.len() as f64;
    let p50_ms = percentile_ms(times, 0.50);
    let p95_ms = percentile_ms(times, 0.95);

    ToolBenchmark {
        display_name,
        tool_name,
        avg_ms,
        p50_ms,
        p95_ms,
        notes,
    }
}

fn print_tool_benchmark(benchmark: &ToolBenchmark) {
    println!(
        "║  {:15} avg:{:>7.2}ms  p50:{:>6.2}ms  p95:{:>7.2}ms  ║",
        benchmark.display_name, benchmark.avg_ms, benchmark.p50_ms, benchmark.p95_ms
    );
}

fn parse_tool_json(result: CallToolResult) -> Value {
    let Some(content) = result.content.first() else {
        panic!("tool returned no content");
    };
    let text = match &content.raw {
        RawContent::Text(text) => text.text.clone(),
        other => panic!("unexpected non-text tool content: {other:?}"),
    };
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse tool json: {error}; payload={text}"))
}

fn ensure_success(tool_name: &str, result: &Value) {
    if result.get("error").is_some() {
        panic!("tool '{tool_name}' failed: {result}");
    }
}

fn ensure_case_result(case: &ToolCase, result: &Value) {
    if result.get("error").is_some() && !case.allow_error {
        panic!(
            "tool '{}' failed during benchmark: {}",
            case.tool_name, result
        );
    }
}

fn percentile_ms(times: &[Duration], percentile: f64) -> f64 {
    let index = ((times.len().saturating_sub(1)) as f64 * percentile).round() as usize;
    times[index].as_secs_f64() * 1000.0
}

fn wrap_list(items: &[String], width: usize) -> Vec<String> {
    if items.is_empty() {
        return vec!["none".into()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for item in items {
        let candidate = if current.is_empty() {
            item.clone()
        } else {
            format!("{current}, {item}")
        };
        if candidate.chars().count() > width && !current.is_empty() {
            lines.push(current);
            current = item.clone();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn new_bench_server(storage_dir: &Path) -> ContextroServer {
    std::fs::create_dir_all(storage_dir).expect("create bench storage dir");

    let mut settings = contextro_config::Settings::default();
    settings.storage_dir = storage_dir.to_string_lossy().to_string();
    ContextroServer::with_settings(settings)
}

fn temp_storage_dir(name: &str) -> PathBuf {
    temp_path(name)
}

fn temp_output_dir(name: &str) -> String {
    temp_path(name).to_string_lossy().to_string()
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("contextro-bench-{unique}-{name}"))
}
