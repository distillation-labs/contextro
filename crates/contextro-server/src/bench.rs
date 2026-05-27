use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

#[allow(dead_code)]
#[path = "main.rs"]
mod server;

use server::ContextroServer;

const ITERATIONS: usize = 25;

#[path = "bench/cases.rs"]
mod cases;
#[path = "bench/helpers.rs"]
mod helpers;

use cases::{build_tool_cases, temp_fixture_repo, BenchmarkFixture, ToolCase};
use helpers::{
    ensure_case_result, ensure_success, make_tool_benchmark, new_bench_server, parse_tool_json,
    print_tool_benchmark, temp_storage_dir, wrap_list, ToolBenchmark,
};

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

fn seed_retrieve_ref(server: &ContextroServer) -> Option<String> {
    let result =
        parse_tool_json(server.dispatch("compact", json!({"content": "seed retrieve payload"})));
    result
        .get("ref_id")
        .and_then(Value::as_str)
        .map(String::from)
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
                "search"
                    | "find_symbol"
                    | "code"
                    | "overview"
                    | "status"
                    | "session_snapshot"
                    | "completion_check"
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
