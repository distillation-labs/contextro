use anyhow::{anyhow, Result};
use serde_json::json;

use super::util::{is_reasonable_symbol_name, is_source_file, relativize_path};
use super::*;

#[derive(Debug, Clone)]
pub(super) struct FileCandidate {
    pub(super) relative_file: String,
    pub(super) absolute_file: String,
    pub(super) expected_symbols: Vec<String>,
}

pub(super) fn generate_tasks(indexed: &IndexedRepo, total: usize) -> Result<Vec<StudyTask>> {
    let symbol_target = total * 30 / 100;
    let search_target = total * 30 / 100;
    let base_lookup_target = total * 20 / 100;

    let unique_symbols = collect_unique_symbols(&indexed.symbols);
    let file_candidates = collect_document_files(&indexed.symbols, &indexed.codebase);
    let document_target = usize::min(
        total - symbol_target - search_target - base_lookup_target,
        file_candidates.len(),
    );
    let lookup_target = total - symbol_target - search_target - document_target;

    if unique_symbols.len() < symbol_target + search_target + lookup_target * 3 {
        return Err(anyhow!(
            "not enough unique symbols to generate {total} tasks (found {})",
            unique_symbols.len()
        ));
    }
    if file_candidates.len() < document_target {
        return Err(anyhow!(
            "not enough file candidates for document-symbol tasks (need {document_target}, found {})",
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
            .map(|symbol| symbol.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        tasks.push(StudyTask {
            id: format!("lookup_{:04}", tasks.len() + 1),
            category: "batch_lookup".into(),
            prompt: format!("Show me the source locations for {joined}."),
            mcp_tool: "code.lookup_symbols".into(),
            mcp_args: json!({
                "operation": "lookup_symbols",
                "symbols": chunk.iter().map(|symbol| symbol.name.clone()).collect::<Vec<_>>().join(","),
            }),
            baseline_strategy: "three_exact_greps_plus_targeted_reads".into(),
            expected_files: chunk
                .iter()
                .map(|symbol| relativize_path(Path::new(&indexed.codebase), &symbol.filepath))
                .collect(),
            expected_symbols: chunk.iter().map(|symbol| symbol.name.clone()).collect(),
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

pub(super) fn collect_unique_symbols(symbols: &[Symbol]) -> Vec<Symbol> {
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

    unique.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.filepath.cmp(&right.filepath))
            .then_with(|| left.line_start.cmp(&right.line_start))
    });
    unique
}

pub(super) fn collect_document_files(symbols: &[Symbol], codebase: &str) -> Vec<FileCandidate> {
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
        items.sort_by_key(|symbol| symbol.line_start);
        if items.len() < 2 {
            continue;
        }
        let expected = items
            .iter()
            .take(3)
            .map(|symbol| (symbol.name.clone(), symbol.line_start))
            .collect::<Vec<_>>();
        let max_line = expected.iter().map(|(_, line)| *line).max().unwrap_or(0);
        if max_line > 260 {
            continue;
        }

        candidates.push(FileCandidate {
            relative_file: relativize_path(Path::new(codebase), &absolute),
            absolute_file: absolute,
            expected_symbols: expected.into_iter().map(|(name, _)| name).collect(),
        });
    }

    candidates.sort_by(|left, right| left.relative_file.cmp(&right.relative_file));
    candidates
}
