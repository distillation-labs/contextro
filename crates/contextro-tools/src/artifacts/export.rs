use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use crate::analysis::{is_generic_symbol_name, is_test_file, strip_base};
use contextro_engines::graph::CodeGraph;
use parking_lot::RwLock;
use serde_json::{json, Value};

#[derive(Clone)]
struct DocsBundleContent {
    architecture: String,
    overview: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CachedFileState {
    len: u64,
    modified_nanos: u128,
}

#[derive(Clone, Copy)]
struct DocsBundleWriteState {
    graph_epoch: u64,
    architecture: CachedFileState,
    overview: CachedFileState,
}

#[derive(Clone)]
struct SidecarWriteState {
    graph_epoch: u64,
    files: Vec<(String, CachedFileState)>,
}

static DOCS_BUNDLE_CONTENT_CACHE: OnceLock<RwLock<HashMap<String, DocsBundleContent>>> =
    OnceLock::new();
static DOCS_BUNDLE_WRITE_CACHE: OnceLock<RwLock<HashMap<String, DocsBundleWriteState>>> =
    OnceLock::new();
static SIDECAR_WRITE_CACHE: OnceLock<RwLock<HashMap<String, SidecarWriteState>>> = OnceLock::new();

fn sort_counts(counts: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs
}

fn push_count_section(
    markdown: &mut String,
    title: &str,
    items: &[(String, usize)],
    value_label: &str,
) {
    markdown.push_str(&format!("## {}\n\n", title));
    if items.is_empty() {
        markdown.push_str("_No data available._\n\n");
        return;
    }
    for (name, count) in items.iter().take(10) {
        markdown.push_str(&format!("- `{}` — {} {}\n", name, count, value_label));
    }
    markdown.push('\n');
}

fn docs_bundle_content_cache() -> &'static RwLock<HashMap<String, DocsBundleContent>> {
    DOCS_BUNDLE_CONTENT_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn docs_bundle_write_cache() -> &'static RwLock<HashMap<String, DocsBundleWriteState>> {
    DOCS_BUNDLE_WRITE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn sidecar_write_cache() -> &'static RwLock<HashMap<String, SidecarWriteState>> {
    SIDECAR_WRITE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn file_state(path: &Path) -> Option<CachedFileState> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified_nanos = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(CachedFileState {
        len: metadata.len(),
        modified_nanos,
    })
}

fn write_if_changed(path: &Path, content: &str) -> Option<CachedFileState> {
    if std::fs::read_to_string(path).ok().as_deref() != Some(content) {
        std::fs::write(path, content).ok()?;
    }
    file_state(path)
}

fn docs_bundle_output_is_current(target: &str, graph_epoch: u64) -> bool {
    let cache = docs_bundle_write_cache().read();
    let Some(entry) = cache.get(target).copied() else {
        return false;
    };
    if entry.graph_epoch != graph_epoch {
        return false;
    }

    let target = Path::new(target);
    file_state(&target.join("architecture.md")) == Some(entry.architecture)
        && file_state(&target.join("overview.md")) == Some(entry.overview)
}

fn sidecar_target_matches(
    file_path: &str,
    target_abs: &Path,
    target_rel: &str,
    target_is_dir: bool,
    codebase: Option<&str>,
) -> bool {
    if target_rel.is_empty() {
        return true;
    }

    let normalized_file = Path::new(file_path);
    if target_is_dir {
        if normalized_file == target_abs || normalized_file.starts_with(target_abs) {
            return true;
        }
    } else if normalized_file == target_abs {
        return true;
    }

    let relative_file = strip_base(file_path, codebase);
    let normalized_target_rel = target_rel.trim_matches('/').replace('\\', "/");
    let normalized_relative = relative_file.replace('\\', "/");
    let normalized_original = file_path.replace('\\', "/");
    if target_is_dir {
        normalized_relative == normalized_target_rel
            || normalized_relative.starts_with(&format!("{normalized_target_rel}/"))
            || normalized_original.contains(&format!("/{normalized_target_rel}/"))
            || normalized_original.ends_with(&format!("/{normalized_target_rel}"))
    } else {
        normalized_relative == normalized_target_rel
            || normalized_original.ends_with(&format!("/{normalized_target_rel}"))
    }
}

fn sidecar_output_is_current(cache_key: &str, graph_epoch: u64) -> Option<usize> {
    let cache = sidecar_write_cache().read();
    let entry = cache.get(cache_key)?;
    if entry.graph_epoch != graph_epoch {
        return None;
    }
    entry
        .files
        .iter()
        .all(|(path, state)| file_state(Path::new(path)) == Some(*state))
        .then_some(entry.files.len())
}

pub fn handle_docs_bundle(
    args: &Value,
    graph: &CodeGraph,
    codebase: Option<&str>,
    graph_epoch: u64,
) -> Value {
    if graph.node_count() == 0 {
        return json!({
            "error": "No indexed graph loaded. Run index(path) before docs_bundle.",
            "hint": "Call index({\"path\":\"/path/to/repo\"}) first so Contextro can build the graph used by docs_bundle."
        });
    }

    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .unwrap_or(".contextro-docs");
    let base = codebase.unwrap_or(".");
    let target = if Path::new(output_dir).is_absolute() {
        output_dir.to_string()
    } else {
        format!("{}/{}", base, output_dir)
    };

    if docs_bundle_output_is_current(&target, graph_epoch) {
        return json!({"status": "generated", "output_dir": target, "files": ["architecture.md", "overview.md"]});
    }

    let cache_key = format!("{graph_epoch}:{base}");
    let cached_content = {
        let cache = docs_bundle_content_cache().read();
        cache.get(&cache_key).cloned()
    };
    let content = if let Some(cached) = cached_content {
        cached
    } else {
        let snapshot = graph.node_degree_snapshot();
        let nodes = snapshot.nodes();
        let mut file_counts: HashMap<String, usize> = HashMap::new();
        let mut language_counts: HashMap<String, usize> = HashMap::new();
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        let mut directory_counts: HashMap<String, usize> = HashMap::new();

        for node in nodes {
            *file_counts
                .entry(node.location.file_path.clone())
                .or_default() += 1;
            *language_counts.entry(node.language.clone()).or_default() += 1;
            *type_counts.entry(node.node_type.to_string()).or_default() += 1;

            let directory = Path::new(&node.location.file_path)
                .parent()
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".into());
            *directory_counts.entry(directory).or_default() += 1;
        }

        let total_files = file_counts.len();
        let top_languages = sort_counts(language_counts);
        let top_symbol_types = sort_counts(type_counts);
        let top_files = sort_counts(file_counts)
            .into_iter()
            .map(|(file, count)| (strip_base(&file, codebase), count))
            .collect::<Vec<_>>();
        let top_directories = sort_counts(directory_counts)
            .into_iter()
            .map(|(directory, count)| (strip_base(&directory, codebase), count))
            .collect::<Vec<_>>();

        let mut scored: Vec<_> = nodes
            .iter()
            .filter(|n| !is_generic_symbol_name(&n.name))
            .filter(|n| !is_test_file(&n.location.file_path))
            .map(|n| {
                let (i, o) = snapshot.degree(&n.id);
                (
                    n.name.clone(),
                    strip_base(&n.location.file_path, codebase),
                    i + o,
                )
            })
            .collect();
        scored.sort_by_key(|b| std::cmp::Reverse(b.2));

        let mut architecture = String::from("# Architecture\n\n");
        architecture.push_str("## Hub Symbols\n\n");
        if scored.is_empty() {
            architecture.push_str("_No hub symbols found._\n");
        } else {
            for (name, file, degree) in scored.iter().take(10) {
                architecture.push_str(&format!(
                    "- **{}** (`{}`) — {} connections\n",
                    name, file, degree
                ));
            }
        }
        architecture.push('\n');
        push_count_section(
            &mut architecture,
            "Top Directories",
            &top_directories,
            "symbols",
        );

        let mut overview = String::from("# Overview\n\n");
        overview.push_str("## Summary\n\n");
        if let Some(codebase) = codebase {
            overview.push_str(&format!("- Codebase: `{}`\n", codebase));
        }
        overview.push_str(&format!(
            "- Total symbols: {}\n- Total relationships: {}\n- Total files: {}\n\n",
            graph.node_count(),
            graph.relationship_count(),
            total_files
        ));
        push_count_section(&mut overview, "Languages", &top_languages, "symbols");
        push_count_section(&mut overview, "Symbol Types", &top_symbol_types, "nodes");
        push_count_section(
            &mut overview,
            "Top Directories",
            &top_directories,
            "symbols",
        );
        push_count_section(&mut overview, "Top Files", &top_files, "symbols");

        overview.push_str("## Hub Symbols\n\n");
        if scored.is_empty() {
            overview.push_str("_No hub symbols found._\n");
        } else {
            for (name, file, degree) in scored.iter().take(10) {
                overview.push_str(&format!(
                    "- **{}** (`{}`) — {} connections\n",
                    name, file, degree
                ));
            }
        }
        overview.push('\n');

        let content = DocsBundleContent {
            architecture,
            overview,
        };
        let mut cache = docs_bundle_content_cache().write();
        if cache.len() >= 8 {
            cache.clear();
        }
        cache.insert(cache_key, content.clone());
        content
    };

    std::fs::create_dir_all(&target).ok();
    let target_path = Path::new(&target);
    let architecture =
        write_if_changed(&target_path.join("architecture.md"), &content.architecture);
    let overview = write_if_changed(&target_path.join("overview.md"), &content.overview);
    if let (Some(architecture), Some(overview)) = (architecture, overview) {
        let mut cache = docs_bundle_write_cache().write();
        if cache.len() >= 16 {
            cache.clear();
        }
        cache.insert(
            target.clone(),
            DocsBundleWriteState {
                graph_epoch,
                architecture,
                overview,
            },
        );
    }

    json!({"status": "generated", "output_dir": target, "files": ["architecture.md", "overview.md"]})
}

pub fn handle_sidecar_export(
    args: &Value,
    graph: &CodeGraph,
    codebase: Option<&str>,
    graph_epoch: u64,
) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let base = codebase.unwrap_or(".");
    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .unwrap_or(".contextro-sidecars");
    let target = if path == "." || path.is_empty() {
        base.to_string()
    } else if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", base, path)
    };
    let target_abs = std::fs::canonicalize(&target).unwrap_or_else(|_| PathBuf::from(&target));
    let target_rel = if path == "." || path.is_empty() {
        String::new()
    } else if Path::new(path).is_absolute() {
        strip_base(path, codebase)
    } else {
        path.trim_start_matches("./")
            .trim_end_matches('/')
            .to_string()
    };
    let target_is_dir = Path::new(&target).is_dir();
    if !Path::new(&target).exists() {
        return json!({"error": format!("Path not found: {}", path)});
    }

    let out_base = if Path::new(output_dir).is_absolute() {
        output_dir.to_string()
    } else {
        format!("{}/{}", base, output_dir)
    };
    std::fs::create_dir_all(&out_base).ok();
    let cache_key = format!("{path}:{out_base}");
    if let Some(sidecars) = sidecar_output_is_current(&cache_key, graph_epoch) {
        return json!({"status": "exported", "sidecars": sidecars, "path": path, "output_dir": out_base});
    }

    let snapshot = graph.node_degree_snapshot();
    let nodes = snapshot.nodes();
    let mut files_written = 0;
    let mut matches_by_file: HashMap<String, bool> = HashMap::new();
    let mut created_dirs = HashSet::new();
    let mut written_states = Vec::new();

    let mut by_file: HashMap<String, Vec<&_>> = HashMap::new();
    for node in nodes {
        let matches_target = *matches_by_file
            .entry(node.location.file_path.clone())
            .or_insert_with(|| {
                sidecar_target_matches(
                    &node.location.file_path,
                    &target_abs,
                    &target_rel,
                    target_is_dir,
                    codebase,
                )
            });
        if matches_target {
            by_file
                .entry(node.location.file_path.clone())
                .or_default()
                .push(node);
        }
    }

    for (file_path, syms) in &by_file {
        let rel = Path::new(file_path)
            .strip_prefix(base)
            .unwrap_or(Path::new(file_path));
        let sidecar_name = format!("{}.graph.md", rel.to_string_lossy());
        let sidecar_path = format!("{}/{}", out_base, sidecar_name);

        if let Some(parent) = Path::new(&sidecar_path).parent() {
            if created_dirs.insert(parent.to_path_buf()) {
                std::fs::create_dir_all(parent).ok();
            }
        }

        let mut content = format!(
            "# {}\n\n## Symbols\n\n",
            Path::new(file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        for sym in syms {
            let (in_d, out_d) = snapshot.degree(&sym.id);
            content.push_str(&format!(
                "- `{}` ({}) L{} — {} callers, {} callees\n",
                sym.name, sym.node_type, sym.location.start_line, in_d, out_d
            ));
        }
        if let Some(state) = write_if_changed(Path::new(&sidecar_path), &content) {
            files_written += 1;
            written_states.push((sidecar_path.clone(), state));
        }
    }

    if files_written == 0 {
        return json!({
            "error": format!("No indexed files matched path: {}", path),
            "path": path,
            "output_dir": out_base,
            "hint": "Pass a file or source subtree from the indexed codebase, not the output directory."
        });
    }

    let mut cache = sidecar_write_cache().write();
    if cache.len() >= 16 {
        cache.clear();
    }
    cache.insert(
        cache_key,
        SidecarWriteState {
            graph_epoch,
            files: written_states,
        },
    );

    json!({"status": "exported", "sidecars": files_written, "path": path, "output_dir": out_base})
}
