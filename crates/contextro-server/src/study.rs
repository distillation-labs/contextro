use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use tiktoken_rs::{cl100k_base, CoreBPE};

use contextro_config::Settings;
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::models::{Symbol, SymbolType};
use contextro_core::NodeType;
use contextro_engines::bm25::Bm25Engine;
use contextro_engines::cache::QueryCache;
use contextro_engines::graph::CodeGraph;
use contextro_indexing::IndexingPipeline;

#[path = "study/cli.rs"]
mod cli;
#[path = "study/index.rs"]
mod index;
#[path = "study/runner.rs"]
mod runner;
#[path = "study/summary.rs"]
mod summary;
#[path = "study/tasks.rs"]
mod tasks;
#[path = "study/util.rs"]
mod util;

use cli::parse_args;
use index::build_index;
use runner::run_tasks;
use summary::{build_config, summarize};
use tasks::generate_tasks;
use util::{truncate_pad, unix_timestamp, write_json};

const DEFAULT_TASKS: usize = 1000;
const DEFAULT_SEARCH_LIMIT: usize = 5;
const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "mts", "cjs", "rs"];

#[derive(Debug, Clone, Serialize)]
struct StudyTask {
    id: String,
    category: String,
    prompt: String,
    mcp_tool: String,
    mcp_args: Value,
    baseline_strategy: String,
    expected_files: Vec<String>,
    expected_symbols: Vec<String>,
    target_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TaskResult {
    task_id: String,
    arm: String,
    category: String,
    completed: bool,
    success: bool,
    wall_clock_ms: f64,
    tokens_estimate: usize,
    tool_calls: usize,
    files_read: usize,
    evidence: Vec<String>,
    error: String,
}

#[derive(Debug, Clone, Serialize)]
struct StatSummary {
    count: usize,
    sum: f64,
    mean: f64,
    median: f64,
    p95: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ArmSummary {
    completed: usize,
    successful: usize,
    success_rate: f64,
    tokens: StatSummary,
    latency_ms: StatSummary,
    tool_calls: StatSummary,
    files_read: StatSummary,
}

#[derive(Debug, Clone, Serialize)]
struct CategorySummary {
    tasks: usize,
    stronger_local_success_rate: f64,
    contextro_success_rate: f64,
    stronger_local_tokens: usize,
    contextro_tokens: usize,
    token_reduction_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
struct TokenizerMeta {
    library: String,
    encoding: String,
}

#[derive(Debug, Clone, Serialize)]
struct StudyConfig {
    timestamp_unix: u64,
    codebase: String,
    tracked_files: usize,
    tasks_requested: usize,
    tasks_generated: usize,
    tokenizer: TokenizerMeta,
    index: IndexSnapshot,
    categories: BTreeMap<String, usize>,
    excluded_capabilities: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct IndexSnapshot {
    total_files: usize,
    total_symbols: usize,
    total_chunks: usize,
    graph_nodes: usize,
    graph_relationships: usize,
    time_seconds: f64,
    bm25_index_ms: f64,
    graph_build_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
struct StudySummary {
    timestamp_unix: u64,
    codebase: String,
    tokenizer: TokenizerMeta,
    index: IndexSnapshot,
    tasks: usize,
    categories: BTreeMap<String, usize>,
    excluded_capabilities: Vec<String>,
    arms: BTreeMap<String, ArmSummary>,
    overall_token_reduction_pct: f64,
    by_category: BTreeMap<String, CategorySummary>,
    notes: Vec<String>,
}

struct IndexedRepo {
    codebase: String,
    tracked_files: usize,
    symbols: Vec<Symbol>,
    graph: CodeGraph,
    bm25: Bm25Engine,
    cache: QueryCache,
    indexed_files: HashSet<String>,
    index_snapshot: IndexSnapshot,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let tokenizer = cl100k_base().context("failed to load cl100k_base tokenizer")?;
    let indexed = build_index(&args.codebase)?;
    let tasks = generate_tasks(&indexed, args.tasks)?;

    let timestamp_unix = unix_timestamp();
    let config = build_config(timestamp_unix, &args.codebase, &indexed, &tasks, args.tasks);
    let output_dir = Path::new(&args.output_dir);
    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "failed to create output directory {}",
            output_dir.to_string_lossy()
        )
    })?;

    let config_path = output_dir.join("platform-1000-study-config.json");
    let tasks_path = output_dir.join("platform-1000-study-tasks.json");
    let results_path = output_dir.join("platform-1000-study-results.json");
    let summary_path = output_dir.join("platform-1000-study-summary.json");

    write_json(&config_path, &config)?;
    write_json(&tasks_path, &tasks)?;
    let results = run_tasks(&indexed, &tokenizer, &tasks)?;
    let summary = summarize(timestamp_unix, &args.codebase, &indexed, &tasks, &results);
    write_json(&results_path, &results)?;
    write_json(&summary_path, &summary)?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  CONTEXTRO PLATFORM STUDY                                   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Codebase: {:<48}║", truncate_pad(&args.codebase, 48));
    println!(
        "║  Indexed files: {:>6}  Symbols: {:>6}  Chunks: {:>6}      ║",
        indexed.index_snapshot.total_files,
        indexed.index_snapshot.total_symbols,
        indexed.index_snapshot.total_chunks,
    );
    println!(
        "║  Graph edges: {:>6}  Tasks: {:>8}                     ║",
        indexed.index_snapshot.graph_relationships,
        tasks.len()
    );
    println!("╠══════════════════════════════════════════════════════════════╣");
    if let Some(local) = summary.arms.get("stronger_local") {
        println!(
            "║  Stronger local tokens: {:>10.0}  success: {:>5.1}%         ║",
            local.tokens.sum,
            local.success_rate * 100.0
        );
    }
    if let Some(contextro) = summary.arms.get("contextro") {
        println!(
            "║  Contextro tokens:     {:>10.0}  success: {:>5.1}%         ║",
            contextro.tokens.sum,
            contextro.success_rate * 100.0
        );
    }
    println!(
        "║  Token reduction: {:>8.1}%                                  ║",
        summary.overall_token_reduction_pct
    );
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Artifacts: {:<47}║",
        truncate_pad(&summary_path.to_string_lossy(), 47)
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn test_symbol(name: &str, filepath: &str, line_start: usize) -> Symbol {
        Symbol {
            name: name.to_string(),
            symbol_type: SymbolType::Function,
            filepath: filepath.to_string(),
            line_start: line_start as u32,
            line_end: (line_start + 2) as u32,
            language: "rust".to_string(),
            signature: format!("fn {name}()"),
            docstring: String::new(),
            parent: None,
            code_snippet: String::new(),
            imports: Vec::new(),
            calls: Vec::new(),
        }
    }

    #[test]
    fn source_filter_includes_rust_files() {
        assert!(util::is_source_file("src/study.rs"));
        assert!(util::is_source_file("src/app.ts"));
        assert!(!util::is_source_file("Cargo.toml"));
    }

    #[test]
    fn unique_symbol_collection_keeps_rust_symbols() {
        let symbols = vec![
            test_symbol("alpha_task", "/repo/crates/a/src/lib.rs", 10),
            test_symbol("alpha_task", "/repo/crates/b/src/lib.rs", 12),
            test_symbol("beta_task", "/repo/crates/a/src/study.rs", 20),
            test_symbol("ignore_json", "/repo/data/config.json", 30),
        ];

        let unique = tasks::collect_unique_symbols(&symbols);

        assert_eq!(unique.len(), 1);
        assert_eq!(unique[0].name, "beta_task");
        assert_eq!(unique[0].filepath, "/repo/crates/a/src/study.rs");
    }

    #[test]
    fn document_file_candidates_include_rust_files() {
        let symbols = vec![
            test_symbol("alpha_task", "/repo/crates/a/src/study.rs", 10),
            test_symbol("beta_task", "/repo/crates/a/src/study.rs", 20),
            test_symbol("gamma_task", "/repo/crates/a/src/study.rs", 30),
            test_symbol("delta_task", "/repo/crates/a/src/study.rs", 40),
        ];

        let candidates = tasks::collect_document_files(&symbols, "/repo");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].relative_file, "crates/a/src/study.rs");
        assert_eq!(
            candidates[0].expected_symbols,
            vec!["alpha_task", "beta_task", "gamma_task"]
        );
    }
}
