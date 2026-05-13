//! Parallel file discovery using the `ignore` crate.

use std::path::{Path, PathBuf};

use contextro_config::Settings;
use ignore::WalkBuilder;

/// Directories to always skip during file discovery.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    "out",
    ".cache",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".next",
    ".nuxt",
    "vendor",
    "Pods",
    ".gradle",
];

/// Discover source files under a root directory, respecting .gitignore.
pub fn discover_files(root: &Path, settings: &Settings) -> Vec<PathBuf> {
    let max_size = settings.max_file_size_mb as u64 * 1024 * 1024;
    let supported_extensions = contextro_parsing::get_supported_extensions();

    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);

    let walker = builder.build();
    let mut files = Vec::new();

    for entry in walker.flatten() {
        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();

        // Skip files in excluded directories
        if path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| SKIP_DIRS.contains(&s))
                .unwrap_or(false)
        }) {
            continue;
        }

        // Check extension
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        if !supported_extensions.contains(&ext) {
            continue;
        }

        // Check file size
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > max_size {
                continue;
            }
        }

        files.push(path.to_path_buf());
    }

    files.sort();
    files
}

/// Compute xxHash3 content hashes for files in parallel.
pub fn hash_files(paths: &[PathBuf]) -> Vec<(PathBuf, String)> {
    use rayon::prelude::*;
    use xxhash_rust::xxh3::xxh3_64;

    paths
        .par_iter()
        .filter_map(|path| {
            let content = std::fs::read(path).ok()?;
            let hash = xxh3_64(&content);
            Some((path.clone(), format!("{:016x}", hash)))
        })
        .collect()
}

/// Diff two file state maps to find added, modified, and deleted files.
pub fn diff_file_states(
    current: &[(PathBuf, String)],
    stored: &std::collections::HashMap<String, String>,
) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<String>) {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    let current_map: std::collections::HashMap<String, &str> = current
        .iter()
        .map(|(p, h)| (p.to_string_lossy().to_string(), h.as_str()))
        .collect();

    for (path, hash) in current {
        let path_str = path.to_string_lossy().to_string();
        match stored.get(&path_str) {
            None => added.push(path.clone()),
            Some(old_hash) => {
                if old_hash != hash {
                    modified.push(path.clone());
                }
            }
        }
    }

    for path in stored.keys() {
        if !current_map.contains_key(path) {
            deleted.push(path.clone());
        }
    }

    (added, modified, deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_files() {
        let tmp = std::env::temp_dir().join("ctx_test_discover");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("src/main.py"), "def hello(): pass").unwrap();
        fs::write(tmp.join("src/readme.md"), "# Hello").unwrap();

        let settings = Settings::default();
        let files = discover_files(&tmp, &settings);
        assert!(files
            .iter()
            .any(|f| f.to_string_lossy().contains("main.py")));
        assert!(!files
            .iter()
            .any(|f| f.to_string_lossy().contains("readme.md")));

        fs::remove_dir_all(tmp).ok();
    }
}

/// Save file hashes to disk for incremental re-indexing.
pub fn save_hashes(hashes: &[(PathBuf, String)], storage_dir: &std::path::Path) {
    let map: std::collections::HashMap<&str, &str> = hashes
        .iter()
        .map(|(p, h)| (p.to_str().unwrap_or(""), h.as_str()))
        .collect();
    let path = storage_dir.join("file_hashes.json");
    if let Ok(json) = serde_json::to_string(&map) {
        std::fs::write(path, json).ok();
    }
}

/// Load previously stored file hashes from disk.
pub fn load_hashes(storage_dir: &std::path::Path) -> std::collections::HashMap<String, String> {
    let path = storage_dir.join("file_hashes.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
