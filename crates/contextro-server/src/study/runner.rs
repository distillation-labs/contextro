use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use super::util::{normalize_relative, round3, strip_base, token_count};
use super::*;

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

struct BaselineOutcome {
    rendered: String,
    tool_calls: usize,
    files_read: usize,
    error: String,
}

pub(super) fn run_tasks(
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
        other => return Err(anyhow!("unsupported MCP task category '{other}'")),
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
        other => return Err(anyhow!("unsupported baseline task category '{other}'")),
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

    rendered.push_str(&format!("git grep -F {symbol}\n"));
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
        rendered.push_str(&format!("git grep -F {symbol}\n"));
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
        rendered: format!("open {relative}\n{excerpt}"),
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
        .with_context(|| format!("failed to run git grep for '{pattern}'"))?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(anyhow!(
            "git grep failed for '{pattern}': {}",
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
    if lowered.contains(&format!("function {symbol_lower}"))
        || lowered.contains(&format!("class {symbol_lower}"))
        || lowered.contains(&format!("interface {symbol_lower}"))
        || lowered.contains(&format!("type {symbol_lower}"))
        || lowered.contains(&format!("enum {symbol_lower}"))
    {
        score += 10;
    }
    if lowered.contains(&format!("const {symbol_lower}"))
        || lowered.contains(&format!("let {symbol_lower}"))
        || lowered.contains(&format!("var {symbol_lower}"))
    {
        score += 6;
    }
    if lowered.contains("export ") {
        score += 3;
    }
    if lowered.contains(&format!("{symbol_lower}(")) {
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
        return json!({"error": format!("Symbol '{name}' not found.")});
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
