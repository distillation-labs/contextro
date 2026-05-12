use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
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
use contextro_indexing::{create_chunks, IndexingPipeline};

const DEFAULT_TASKS: usize = 1000;
const DEFAULT_SEARCH_LIMIT: usize = 5;
const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "mts", "cjs"];

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

#[derive(Debug, Clone)]
struct GrepHit {
    relative_file: String,
    absolute_file: PathBuf,
    line_number: usize,
    line_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvalMode {
    Any,
    All,
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

struct CliArgs {
    codebase: String,
    output_dir: String,
    tasks: usize,
}

fn parse_args() -> Result<CliArgs> {
    let mut codebase: Option<String> = None;
    let mut output_dir: Option<String> = None;
    let mut tasks = DEFAULT_TASKS;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--codebase" => {
                codebase = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value for --codebase"))?,
                );
            }
            "--output-dir" => {
                output_dir = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value for --output-dir"))?,
                );
            }
            "--tasks" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --tasks"))?;
                tasks = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid --tasks value '{}'", value))?;
            }
            "--help" | "-h" => {
                println!(
                    "contextro-study --codebase PATH --output-dir DIR [--tasks N]\n\
                     Options:\n\
                     - --codebase PATH   path to the codebase to study (required)\n\
                     - --output-dir DIR  directory to write study results (required)\n\
                     - --tasks N         number of tasks to generate (default: {})",
                    DEFAULT_TASKS
                );
                std::process::exit(0);
            }
            other => return Err(anyhow!("unknown argument '{}'", other)),
        }
    }

    let codebase = codebase.ok_or_else(|| anyhow!("--codebase PATH is required"))?;
    let output_dir = output_dir.ok_or_else(|| anyhow!("--output-dir DIR is required"))?;

    if tasks < 100 {
        return Err(anyhow!("--tasks must be at least 100"));
    }

    Ok(CliArgs {
        codebase,
        output_dir,
        tasks,
    })
}

fn build_index(codebase: &str) -> Result<IndexedRepo> {
    let root = Path::new(codebase);
    if !root.is_dir() {
        return Err(anyhow!("codebase directory not found: {}", codebase));
    }

    let tracked_files = git_ls_files_count(root)?;

    let settings = Settings::default();
    let pipeline = IndexingPipeline::new(settings);

    let index_start = Instant::now();
    let (result, symbols) = pipeline
        .index(root)
        .with_context(|| format!("failed to index {}", codebase))?;
    let index_elapsed = index_start.elapsed().as_secs_f64();

    let chunks = create_chunks(&symbols);
    let bm25 = Bm25Engine::new_in_memory();
    let bm25_start = Instant::now();
    bm25.index_chunks(&chunks);
    let bm25_index_ms = bm25_start.elapsed().as_secs_f64() * 1000.0;

    let graph = CodeGraph::new();
    let graph_start = Instant::now();
    build_graph(&graph, &symbols);
    let graph_build_ms = graph_start.elapsed().as_secs_f64() * 1000.0;

    let cache = QueryCache::new(256, 300.0);
    let indexed_files = symbols
        .iter()
        .map(|s| relativize_path(root, &s.filepath))
        .collect::<HashSet<_>>();

    let index_snapshot = IndexSnapshot {
        total_files: result.total_files,
        total_symbols: result.total_symbols,
        total_chunks: chunks.len(),
        graph_nodes: graph.node_count(),
        graph_relationships: graph.relationship_count(),
        time_seconds: if result.time_seconds > 0.0 {
            result.time_seconds
        } else {
            index_elapsed
        },
        bm25_index_ms,
        graph_build_ms,
    };

    Ok(IndexedRepo {
        codebase: codebase.to_string(),
        tracked_files,
        symbols,
        graph,
        bm25,
        cache,
        indexed_files,
        index_snapshot,
    })
}

fn generate_tasks(indexed: &IndexedRepo, total: usize) -> Result<Vec<StudyTask>> {
    let symbol_target = total * 30 / 100;
    let search_target = total * 30 / 100;
    let lookup_target = total * 20 / 100;
    let document_target = total - symbol_target - search_target - lookup_target;

    let unique_symbols = collect_unique_symbols(&indexed.symbols);
    let file_candidates = collect_document_files(&indexed.symbols, &indexed.codebase);

    if unique_symbols.len() < symbol_target + search_target + lookup_target * 3 {
        return Err(anyhow!(
            "not enough unique symbols to generate {} tasks (found {})",
            total,
            unique_symbols.len()
        ));
    }
    if file_candidates.len() < document_target {
        return Err(anyhow!(
            "not enough file candidates for document-symbol tasks (need {}, found {})",
            document_target,
            file_candidates.len()
        ));
    }

    let mut tasks = Vec::with_capacity(total);
    let mut cursor = 0usize;

    for symbol in unique_symbols.iter().take(symbol_target) {
        let relative = relativize_path(Path::new(&indexed.codebase), &symbol.filepath);
        tasks.push(StudyTask {
            id: format!("sym_{:04}", tasks.len() + 1),
            category: "symbol_discovery".into(),
            prompt: format!("Find {}.", symbol.name),
            mcp_tool: "find_symbol".into(),
            mcp_args: json!({"name": symbol.name, "exact": true}),
            baseline_strategy: "git_grep_exact_plus_definition_window".into(),
            expected_files: vec![relative],
            expected_symbols: vec![symbol.name.clone()],
            target_file: None,
        });
        cursor += 1;
    }

    for symbol in unique_symbols.iter().skip(cursor).take(search_target) {
        let relative = relativize_path(Path::new(&indexed.codebase), &symbol.filepath);
        tasks.push(StudyTask {
            id: format!("search_{:04}", tasks.len() + 1),
            category: "exact_search".into(),
            prompt: format!("Search the codebase for {}.", symbol.name),
            mcp_tool: "search".into(),
            mcp_args: json!({"query": symbol.name, "limit": DEFAULT_SEARCH_LIMIT, "mode": "bm25"}),
            baseline_strategy: "git_grep_exact_plus_match_windows".into(),
            expected_files: vec![relative],
            expected_symbols: vec![symbol.name.clone()],
            target_file: None,
        });
        cursor += 1;
    }

    let lookup_symbols = unique_symbols
        .iter()
        .skip(cursor)
        .take(lookup_target * 3)
        .cloned()
        .collect::<Vec<_>>();
    for chunk in lookup_symbols.chunks(3).take(lookup_target) {
        let joined = chunk
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        tasks.push(StudyTask {
            id: format!("lookup_{:04}", tasks.len() + 1),
            category: "batch_lookup".into(),
            prompt: format!("Show me the source locations for {}.", joined),
            mcp_tool: "code.lookup_symbols".into(),
            mcp_args: json!({
                "operation": "lookup_symbols",
                "symbols": chunk.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(","),
            }),
            baseline_strategy: "three_exact_greps_plus_targeted_reads".into(),
            expected_files: chunk
                .iter()
                .map(|s| relativize_path(Path::new(&indexed.codebase), &s.filepath))
                .collect(),
            expected_symbols: chunk.iter().map(|s| s.name.clone()).collect(),
            target_file: None,
        });
    }

    for candidate in file_candidates.into_iter().take(document_target) {
        tasks.push(StudyTask {
            id: format!("doc_{:04}", tasks.len() + 1),
            category: "document_symbols".into(),
            prompt: format!(
                "List the functions and classes defined in {}.",
                candidate.relative_file
            ),
            mcp_tool: "code.get_document_symbols".into(),
            mcp_args: json!({
                "operation": "get_document_symbols",
                "file_path": candidate.absolute_file,
            }),
            baseline_strategy: "bounded_file_read".into(),
            expected_files: vec![candidate.relative_file.clone()],
            expected_symbols: candidate.expected_symbols,
            target_file: Some(candidate.relative_file),
        });
    }

    Ok(tasks)
}

#[derive(Debug, Clone)]
struct FileCandidate {
    relative_file: String,
    absolute_file: String,
    expected_symbols: Vec<String>,
}

fn collect_unique_symbols(symbols: &[Symbol]) -> Vec<Symbol> {
    let mut grouped: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    for symbol in symbols {
        if !is_reasonable_symbol_name(&symbol.name) || !is_source_file(&symbol.filepath) {
            continue;
        }
        grouped
            .entry(symbol.name.as_str())
            .or_default()
            .push(symbol);
    }

    let mut unique = grouped
        .into_values()
        .filter_map(|items| {
            if items.len() == 1 {
                Some(items[0].clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    unique.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.filepath.cmp(&b.filepath))
            .then_with(|| a.line_start.cmp(&b.line_start))
    });
    unique
}

fn collect_document_files(symbols: &[Symbol], codebase: &str) -> Vec<FileCandidate> {
    let mut files: HashMap<String, Vec<&Symbol>> = HashMap::new();
    for symbol in symbols {
        if !is_source_file(&symbol.filepath) {
            continue;
        }
        files
            .entry(symbol.filepath.clone())
            .or_default()
            .push(symbol);
    }

    let mut candidates = Vec::new();
    for (absolute, mut items) in files {
        items.sort_by_key(|s| s.line_start);
        if items.len() < 3 || items.len() > 14 {
            continue;
        }
        let expected = items
            .iter()
            .take(3)
            .map(|s| (s.name.clone(), s.line_start))
            .collect::<Vec<_>>();
        let max_line = expected.iter().map(|(_, line)| *line).max().unwrap_or(0);
        if max_line > 180 {
            continue;
        }

        candidates.push(FileCandidate {
            relative_file: relativize_path(Path::new(codebase), &absolute),
            absolute_file: absolute,
            expected_symbols: expected.into_iter().map(|(name, _)| name).collect(),
        });
    }

    candidates.sort_by(|a, b| a.relative_file.cmp(&b.relative_file));
    candidates
}

fn run_tasks(
    indexed: &IndexedRepo,
    tokenizer: &CoreBPE,
    tasks: &[StudyTask],
) -> Result<Vec<TaskResult>> {
    let mut results = Vec::with_capacity(tasks.len() * 2);
    for (idx, task) in tasks.iter().enumerate() {
        if idx % 50 == 0 || idx + 1 == tasks.len() {
            eprintln!(
                "[study] running task {}/{} ({})",
                idx + 1,
                tasks.len(),
                task.id
            );
        }
        results.push(run_baseline_task(indexed, tokenizer, task)?);
        results.push(run_contextro_task(indexed, tokenizer, task)?);
    }
    Ok(results)
}

fn run_contextro_task(
    indexed: &IndexedRepo,
    tokenizer: &CoreBPE,
    task: &StudyTask,
) -> Result<TaskResult> {
    let start = Instant::now();
    let response = match task.category.as_str() {
        "symbol_discovery" => handle_find_symbol_like(
            task.mcp_args
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            true,
            &indexed.graph,
            Some(&indexed.codebase),
        ),
        "exact_search" => contextro_tools::search::handle_search(
            &task.mcp_args,
            &indexed.bm25,
            &indexed.graph,
            &indexed.cache,
            &contextro_engines::vector::VectorIndex::new(),
        ),
        "batch_lookup" | "document_symbols" => contextro_tools::code::handle_code(
            &task.mcp_args,
            &indexed.graph,
            Some(&indexed.codebase),
        ),
        other => return Err(anyhow!("unsupported MCP task category '{}'", other)),
    };
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    let rendered = response.to_string();
    let evidence = matched_evidence(task, &rendered);
    let success = evaluate_rendered(task, &rendered);

    Ok(TaskResult {
        task_id: task.id.clone(),
        arm: "contextro".into(),
        category: task.category.clone(),
        completed: response.get("error").is_none(),
        success,
        wall_clock_ms: round3(elapsed),
        tokens_estimate: token_count(tokenizer, &rendered),
        tool_calls: 1,
        files_read: 0,
        evidence,
        error: response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn run_baseline_task(
    indexed: &IndexedRepo,
    tokenizer: &CoreBPE,
    task: &StudyTask,
) -> Result<TaskResult> {
    let start = Instant::now();
    let outcome = match task.category.as_str() {
        "symbol_discovery" => baseline_symbol_lookup(indexed, task, 2)?,
        "exact_search" => baseline_symbol_lookup(indexed, task, 3)?,
        "batch_lookup" => baseline_lookup_symbols(indexed, task)?,
        "document_symbols" => baseline_document_symbols(indexed, task)?,
        other => return Err(anyhow!("unsupported baseline task category '{}'", other)),
    };
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    let evidence = matched_evidence(task, &outcome.rendered);
    let success = evaluate_rendered(task, &outcome.rendered);

    Ok(TaskResult {
        task_id: task.id.clone(),
        arm: "stronger_local".into(),
        category: task.category.clone(),
        completed: outcome.error.is_empty(),
        success,
        wall_clock_ms: round3(elapsed),
        tokens_estimate: token_count(tokenizer, &outcome.rendered),
        tool_calls: outcome.tool_calls,
        files_read: outcome.files_read,
        evidence,
        error: outcome.error,
    })
}

struct BaselineOutcome {
    rendered: String,
    tool_calls: usize,
    files_read: usize,
    error: String,
}

fn baseline_symbol_lookup(
    indexed: &IndexedRepo,
    task: &StudyTask,
    max_files: usize,
) -> Result<BaselineOutcome> {
    let symbol = task
        .expected_symbols
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("missing expected symbol for {}", task.id))?;
    let hits = git_grep(indexed, &symbol, true)?;
    let ranked = rank_symbol_hits(&symbol, hits);
    let mut rendered = String::new();
    let mut reads = 0usize;

    rendered.push_str(&format!("git grep -F {}\n", symbol));
    for hit in ranked.iter().take(10) {
        rendered.push_str(&format!(
            "{}:{}:{}\n",
            hit.relative_file,
            hit.line_number,
            hit.line_text.trim()
        ));
    }

    let mut seen = HashSet::new();
    for hit in ranked {
        if seen.insert(hit.relative_file.clone()) {
            reads += 1;
            rendered.push_str("\n--- file window ---\n");
            rendered.push_str(&read_window(&hit.absolute_file, hit.line_number, 20, 4000)?);
            if reads >= max_files {
                break;
            }
        }
    }

    Ok(BaselineOutcome {
        rendered,
        tool_calls: 1 + reads,
        files_read: reads,
        error: String::new(),
    })
}

fn baseline_lookup_symbols(indexed: &IndexedRepo, task: &StudyTask) -> Result<BaselineOutcome> {
    let mut rendered = String::new();
    let mut reads = 0usize;
    let mut tool_calls = 0usize;

    for symbol in &task.expected_symbols {
        let hits = rank_symbol_hits(symbol, git_grep(indexed, symbol, true)?);
        rendered.push_str(&format!("git grep -F {}\n", symbol));
        for hit in hits.iter().take(5) {
            rendered.push_str(&format!(
                "{}:{}:{}\n",
                hit.relative_file,
                hit.line_number,
                hit.line_text.trim()
            ));
        }
        if let Some(primary) = hits.first() {
            reads += 1;
            tool_calls += 2;
            rendered.push_str("\n--- file window ---\n");
            rendered.push_str(&read_window(
                &primary.absolute_file,
                primary.line_number,
                18,
                3200,
            )?);
            rendered.push('\n');
        } else {
            tool_calls += 1;
        }
    }

    Ok(BaselineOutcome {
        rendered,
        tool_calls,
        files_read: reads,
        error: String::new(),
    })
}

fn baseline_document_symbols(indexed: &IndexedRepo, task: &StudyTask) -> Result<BaselineOutcome> {
    let relative = task
        .target_file
        .as_ref()
        .ok_or_else(|| anyhow!("missing target file for {}", task.id))?;
    let absolute = Path::new(&indexed.codebase).join(relative);
    let content = fs::read_to_string(&absolute)
        .with_context(|| format!("failed to read {}", absolute.to_string_lossy()))?;
    let excerpt = content
        .lines()
        .take(220)
        .enumerate()
        .map(|(idx, line)| format!("L{}: {}", idx + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(BaselineOutcome {
        rendered: format!("open {}\n{}", relative, excerpt),
        tool_calls: 1,
        files_read: 1,
        error: String::new(),
    })
}

fn git_grep(indexed: &IndexedRepo, pattern: &str, fixed: bool) -> Result<Vec<GrepHit>> {
    let mut command = Command::new("git");
    command.current_dir(&indexed.codebase);
    command.arg("grep").arg("-n").arg("-I");
    if fixed {
        command.arg("-F");
    } else {
        command.arg("-E");
    }
    command.arg(pattern).arg("--");

    let output = command
        .output()
        .with_context(|| format!("failed to run git grep for '{}'", pattern))?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(anyhow!(
            "git grep failed for '{}': {}",
            pattern,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut hits = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.splitn(3, ':');
        let file = match parts.next() {
            Some(value) => value,
            None => continue,
        };
        let line_number = parts
            .next()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1);
        let line_text = parts.next().unwrap_or_default().to_string();
        let normalized = normalize_relative(file);
        if !indexed.indexed_files.contains(&normalized) {
            continue;
        }
        hits.push(GrepHit {
            relative_file: normalized.clone(),
            absolute_file: Path::new(&indexed.codebase).join(&normalized),
            line_number,
            line_text,
        });
    }
    Ok(hits)
}

fn rank_symbol_hits(symbol: &str, mut hits: Vec<GrepHit>) -> Vec<GrepHit> {
    hits.sort_by_key(|hit| {
        (
            Reverse(definition_score(symbol, &hit.line_text)),
            hit.relative_file.clone(),
            hit.line_number,
        )
    });
    hits
}

fn definition_score(symbol: &str, line: &str) -> usize {
    let lowered = line.to_lowercase();
    let symbol_lower = symbol.to_lowercase();
    let mut score = 0usize;
    if lowered.contains(&format!("function {}", symbol_lower))
        || lowered.contains(&format!("class {}", symbol_lower))
        || lowered.contains(&format!("interface {}", symbol_lower))
        || lowered.contains(&format!("type {}", symbol_lower))
        || lowered.contains(&format!("enum {}", symbol_lower))
    {
        score += 10;
    }
    if lowered.contains(&format!("const {}", symbol_lower))
        || lowered.contains(&format!("let {}", symbol_lower))
        || lowered.contains(&format!("var {}", symbol_lower))
    {
        score += 6;
    }
    if lowered.contains("export ") {
        score += 3;
    }
    if lowered.contains(&format!("{}(", symbol_lower)) {
        score += 2;
    }
    score
}

fn read_window(path: &Path, center_line: usize, radius: usize, max_chars: usize) -> Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.to_string_lossy()))?;
    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(String::new());
    }

    let start = center_line.saturating_sub(radius + 1);
    let end = usize::min(lines.len(), center_line + radius);
    let mut rendered = lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, line)| format!("L{}: {}", start + offset + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    if rendered.len() > max_chars {
        rendered.truncate(max_chars);
    }
    Ok(rendered)
}

fn handle_find_symbol_like(
    name: &str,
    exact: bool,
    graph: &CodeGraph,
    codebase: Option<&str>,
) -> Value {
    if name.is_empty() {
        return json!({"error": "Missing required parameter: name"});
    }

    let matches = graph.find_nodes_by_name(name, exact);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    let symbols: Vec<Value> = matches
        .iter()
        .take(20)
        .map(|node| {
            json!({
                "name": node.name,
                "type": node.node_type.to_string(),
                "file": strip_base(&node.location.file_path, codebase),
                "line": node.location.start_line,
                "language": node.language,
            })
        })
        .collect();

    json!({"total": symbols.len(), "symbols": symbols})
}

fn summarize(
    timestamp_unix: u64,
    codebase: &str,
    indexed: &IndexedRepo,
    tasks: &[StudyTask],
    results: &[TaskResult],
) -> StudySummary {
    let tokenizer = TokenizerMeta {
        library: "tiktoken-rs".into(),
        encoding: "cl100k_base".into(),
    };

    let mut arms = BTreeMap::new();
    for arm in ["stronger_local", "contextro"] {
        let arm_results = results
            .iter()
            .filter(|result| result.arm == arm)
            .cloned()
            .collect::<Vec<_>>();
        arms.insert(arm.to_string(), summarize_arm(&arm_results));
    }

    let mut categories = BTreeMap::new();
    for task in tasks {
        *categories.entry(task.category.clone()).or_insert(0) += 1;
    }

    let stronger_total = arms
        .get("stronger_local")
        .map(|summary| summary.tokens.sum)
        .unwrap_or(0.0);
    let contextro_total = arms
        .get("contextro")
        .map(|summary| summary.tokens.sum)
        .unwrap_or(0.0);
    let overall_token_reduction_pct = if stronger_total > 0.0 {
        round1((1.0 - contextro_total / stronger_total) * 100.0)
    } else {
        0.0
    };

    let mut by_category = BTreeMap::new();
    let category_names = categories.keys().cloned().collect::<Vec<_>>();
    for category in category_names {
        let stronger = results
            .iter()
            .filter(|result| result.arm == "stronger_local" && result.category == category)
            .cloned()
            .collect::<Vec<_>>();
        let contextro = results
            .iter()
            .filter(|result| result.arm == "contextro" && result.category == category)
            .cloned()
            .collect::<Vec<_>>();
        let stronger_tokens = stronger.iter().map(|r| r.tokens_estimate).sum::<usize>();
        let contextro_tokens = contextro.iter().map(|r| r.tokens_estimate).sum::<usize>();
        let reduction = if stronger_tokens > 0 {
            round1((1.0 - contextro_tokens as f64 / stronger_tokens as f64) * 100.0)
        } else {
            0.0
        };
        by_category.insert(
            category.clone(),
            CategorySummary {
                tasks: stronger.len(),
                stronger_local_success_rate: success_rate(&stronger),
                contextro_success_rate: success_rate(&contextro),
                stronger_local_tokens: stronger_tokens,
                contextro_tokens,
                token_reduction_pct: reduction,
            },
        );
    }

    StudySummary {
        timestamp_unix,
        codebase: codebase.to_string(),
        tokenizer,
        index: indexed.index_snapshot.clone(),
        tasks: tasks.len(),
        categories,
        excluded_capabilities: excluded_capabilities(indexed),
        arms,
        overall_token_reduction_pct,
        by_category,
        notes: vec![
            "This study uses scripted deterministic retrieval tasks, not autonomous coding loops."
                .into(),
            "Call-graph-dependent tasks were excluded because the current TypeScript/Javascript parser path emits zero calls on this codebase.".into(),
        ],
    }
}

fn summarize_arm(results: &[TaskResult]) -> ArmSummary {
    ArmSummary {
        completed: results.iter().filter(|r| r.completed).count(),
        successful: results.iter().filter(|r| r.success).count(),
        success_rate: success_rate(results),
        tokens: stats(results.iter().map(|r| r.tokens_estimate as f64).collect()),
        latency_ms: stats(results.iter().map(|r| r.wall_clock_ms).collect()),
        tool_calls: stats(results.iter().map(|r| r.tool_calls as f64).collect()),
        files_read: stats(results.iter().map(|r| r.files_read as f64).collect()),
    }
}

fn stats(mut values: Vec<f64>) -> StatSummary {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let count = values.len();
    let sum = values.iter().sum::<f64>();
    let mean = if count > 0 { sum / count as f64 } else { 0.0 };
    let median = percentile(&values, 0.50);
    let p95 = percentile(&values, 0.95);
    StatSummary {
        count,
        sum: round3(sum),
        mean: round3(mean),
        median: round3(median),
        p95: round3(p95),
    }
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() - 1) as f64 * p).round() as usize;
    values[index]
}

fn success_rate(results: &[TaskResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    results.iter().filter(|r| r.success).count() as f64 / results.len() as f64
}

fn build_config(
    timestamp_unix: u64,
    codebase: &str,
    indexed: &IndexedRepo,
    tasks: &[StudyTask],
    tasks_requested: usize,
) -> StudyConfig {
    let mut categories = BTreeMap::new();
    for task in tasks {
        *categories.entry(task.category.clone()).or_insert(0) += 1;
    }

    StudyConfig {
        timestamp_unix,
        codebase: codebase.to_string(),
        tracked_files: indexed.tracked_files,
        tasks_requested,
        tasks_generated: tasks.len(),
        tokenizer: TokenizerMeta {
            library: "tiktoken-rs".into(),
            encoding: "cl100k_base".into(),
        },
        index: indexed.index_snapshot.clone(),
        categories,
        excluded_capabilities: excluded_capabilities(indexed),
        limitations: vec![
            "TypeScript/Javascript graph edges are zero on this repo because the current parser path does not populate calls for those languages.".into(),
            "The no-MCP arm is a stronger local baseline built from exact grep and bounded file reads, not an autonomous agent.".into(),
        ],
    }
}

fn excluded_capabilities(indexed: &IndexedRepo) -> Vec<String> {
    let mut capabilities = Vec::new();
    if indexed.index_snapshot.graph_relationships == 0 {
        capabilities.push("find_callers".into());
        capabilities.push("find_callees".into());
        capabilities.push("impact".into());
        capabilities.push("relationship-rich explain".into());
    }
    capabilities
}

fn evaluate_rendered(task: &StudyTask, rendered: &str) -> bool {
    let mode = match task.category.as_str() {
        "batch_lookup" | "document_symbols" => EvalMode::All,
        _ => EvalMode::Any,
    };

    match task.category.as_str() {
        "document_symbols" => evaluate_strings(&task.expected_symbols, rendered, mode),
        "batch_lookup" => evaluate_strings(&task.expected_files, rendered, mode),
        _ => evaluate_strings(&task.expected_files, rendered, mode),
    }
}

fn evaluate_strings(expected: &[String], rendered: &str, mode: EvalMode) -> bool {
    if expected.is_empty() {
        return false;
    }
    match mode {
        EvalMode::Any => expected.iter().any(|item| rendered.contains(item)),
        EvalMode::All => expected.iter().all(|item| rendered.contains(item)),
    }
}

fn matched_evidence(task: &StudyTask, rendered: &str) -> Vec<String> {
    let mut matches = Vec::new();
    for item in task
        .expected_files
        .iter()
        .chain(task.expected_symbols.iter())
    {
        if rendered.contains(item) {
            matches.push(item.clone());
        }
    }
    matches
}

fn build_graph(graph: &CodeGraph, symbols: &[Symbol]) {
    let mut known: HashMap<String, String> = HashMap::new();

    for (idx, symbol) in symbols.iter().enumerate() {
        let node_id = format!("n{}", idx);
        known.insert(symbol.name.clone(), node_id.clone());
        graph.add_node(UniversalNode {
            id: node_id,
            name: symbol.name.clone(),
            node_type: match symbol.symbol_type {
                SymbolType::Class => NodeType::Class,
                SymbolType::Variable => NodeType::Variable,
                SymbolType::Method | SymbolType::Function => NodeType::Function,
            },
            location: UniversalLocation {
                file_path: symbol.filepath.clone(),
                start_line: symbol.line_start,
                end_line: symbol.line_end,
                start_column: 0,
                end_column: 0,
                language: symbol.language.clone(),
            },
            language: symbol.language.clone(),
            line_count: symbol.line_count(),
            docstring: if symbol.docstring.is_empty() {
                None
            } else {
                Some(symbol.docstring.clone())
            },
            ..Default::default()
        });
    }

    let mut rel_count = 0usize;
    for symbol in symbols {
        let Some(caller_id) = known.get(&symbol.name).cloned() else {
            continue;
        };
        for call in &symbol.calls {
            if let Some(callee_id) = known.get(call) {
                if &caller_id == callee_id {
                    continue;
                }
                graph.add_relationship(UniversalRelationship {
                    id: format!("r{}", rel_count),
                    source_id: caller_id.clone(),
                    target_id: callee_id.clone(),
                    relationship_type: RelationshipType::Calls,
                    strength: 1.0,
                });
                rel_count += 1;
            }
        }
    }
}

fn git_ls_files_count(root: &Path) -> Result<usize> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("ls-files")
        .output()
        .with_context(|| format!("failed to run git ls-files in {}", root.to_string_lossy()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().count())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let rendered = serde_json::to_string_pretty(value)?;
    fs::write(path, rendered)
        .with_context(|| format!("failed to write {}", path.to_string_lossy()))?;
    Ok(())
}

fn relativize_path(root: &Path, absolute: &str) -> String {
    strip_base(absolute, Some(root.to_string_lossy().as_ref()))
}

fn strip_base(file: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|base| Path::new(file).strip_prefix(base).ok())
        .map(|path| normalize_relative(&path.to_string_lossy()))
        .unwrap_or_else(|| normalize_relative(file))
}

fn normalize_relative(path: &str) -> String {
    path.trim_start_matches("./").replace('\\', "/")
}

fn is_reasonable_symbol_name(name: &str) -> bool {
    let len_ok = (3..=100).contains(&name.len());
    let starts_ok = name
        .chars()
        .next()
        .map(|ch| ch.is_ascii_alphabetic() || ch == '_')
        .unwrap_or(false);
    let chars_ok = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'));
    len_ok && starts_ok && chars_ok
}

fn is_source_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SOURCE_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

fn token_count(tokenizer: &CoreBPE, text: &str) -> usize {
    tokenizer.encode_with_special_tokens(text).len()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn truncate_pad(value: &str, width: usize) -> String {
    let shortened = if value.len() > width {
        let keep = width.saturating_sub(1);
        format!("{}…", &value[..keep])
    } else {
        value.to_string()
    };
    format!("{shortened:<width$}")
}
