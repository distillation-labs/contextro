//! Fast parallel file discovery with .gitignore support.
//!
//! Uses the `ignore` crate (same engine as ripgrep) for native .gitignore
//! handling and parallel directory walking.

use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::Path;

/// Default directories to skip (in addition to .gitignore)
const DEFAULT_SKIP_DIRS: &[&str] = &[
    "node_modules",
    "__pycache__",
    ".contextia",
    "venv",
    ".venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    "dist",
    "build",
    ".eggs",
    ".ruff_cache",
    ".hg",
    ".svn",
    ".git",
    "target",
    ".cargo",
];

pub fn discover_files_impl(
    root: &str,
    extensions: Option<Vec<String>>,
    max_file_size_bytes: Option<u64>,
    skip_dirs: Option<Vec<String>>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let root_path = Path::new(root);
    if !root_path.is_dir() {
        return Err(format!("Not a directory: {}", root).into());
    }

    let max_size = max_file_size_bytes.unwrap_or(10 * 1024 * 1024); // 10MB default

    // Build extension set for filtering
    let ext_set: Option<HashSet<String>> = extensions.map(|exts| {
        exts.into_iter()
            .map(|e| e.trim_start_matches('.').to_lowercase())
            .collect()
    });

    // Build skip dirs set
    let skip_set: HashSet<String> = skip_dirs
        .unwrap_or_default()
        .into_iter()
        .chain(DEFAULT_SKIP_DIRS.iter().map(|s| s.to_string()))
        .collect();

    // Use ignore crate's WalkBuilder for .gitignore-aware parallel walking
    let walker = WalkBuilder::new(root_path)
        .hidden(true) // skip hidden files
        .git_ignore(true) // respect .gitignore
        .git_global(true) // respect global gitignore
        .git_exclude(true) // respect .git/info/exclude
        .follow_links(false)
        .threads(num_cpus())
        .build_parallel();

    let files = std::sync::Mutex::new(Vec::new());

    walker.run(|| {
        let ext_set = &ext_set;
        let skip_set = &skip_set;
        let files = &files;

        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            let path = entry.path();

            // Skip directories in skip set
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if skip_set.contains(name) {
                        return ignore::WalkState::Skip;
                    }
                }
                return ignore::WalkState::Continue;
            }

            // Only process regular files
            if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            // Check extension
            if let Some(ref exts) = ext_set {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                if !exts.contains(&ext) {
                    return ignore::WalkState::Continue;
                }
            }

            // Check file size
            if let Ok(meta) = path.metadata() {
                if meta.len() > max_size {
                    return ignore::WalkState::Continue;
                }
            }

            // Add to results
            if let Some(path_str) = path.to_str() {
                files.lock().unwrap().push(path_str.to_string());
            }

            ignore::WalkState::Continue
        })
    });

    let mut result = files.into_inner().unwrap();
    result.sort();
    Ok(result)
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
