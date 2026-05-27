use anyhow::{anyhow, Context, Result};

use super::util::relativize_path;
use super::*;

pub(super) fn build_index(codebase: &str) -> Result<IndexedRepo> {
    let root = Path::new(codebase);
    if !root.is_dir() {
        return Err(anyhow!("codebase directory not found: {codebase}"));
    }

    let tracked_files = git_ls_files_count(root)?;

    let settings = Settings::default();
    let pipeline = IndexingPipeline::new(settings);

    let index_start = Instant::now();
    let (result, symbols, chunks) = pipeline
        .index(root)
        .with_context(|| format!("failed to index {codebase}"))?;
    let index_elapsed = index_start.elapsed().as_secs_f64();

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
        .map(|symbol| relativize_path(root, &symbol.filepath))
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

fn build_graph(graph: &CodeGraph, symbols: &[Symbol]) {
    let mut known: HashMap<String, String> = HashMap::new();

    for (idx, symbol) in symbols.iter().enumerate() {
        let node_id = format!("n{idx}");
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
            content: if symbol.code_snippet.is_empty() {
                symbol.signature.clone()
            } else {
                symbol.code_snippet.clone()
            },
            line_count: symbol.line_count(),
            docstring: if symbol.docstring.is_empty() {
                None
            } else {
                Some(symbol.docstring.clone())
            },
            visibility: if symbol.signature.trim_start().starts_with("pub ")
                || symbol.signature.trim_start().starts_with("pub(")
            {
                "public".into()
            } else {
                String::new()
            },
            is_async: symbol.signature.contains("async fn"),
            parent: symbol.parent.clone(),
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
                    id: format!("r{rel_count}"),
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
