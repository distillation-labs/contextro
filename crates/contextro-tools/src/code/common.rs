use super::*;

pub(crate) fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

pub(crate) fn strip_base(file: &str, codebase: Option<&str>) -> String {
    if let Some(base) = codebase {
        let file_path = Path::new(file);
        if let Ok(stripped) = file_path.strip_prefix(base) {
            return stripped.to_string_lossy().to_string();
        }

        let canonical_file = std::fs::canonicalize(file_path).ok();
        let canonical_base = std::fs::canonicalize(base).ok();
        if let (Some(canonical_file), Some(canonical_base)) = (canonical_file, canonical_base) {
            if let Ok(stripped) = canonical_file.strip_prefix(&canonical_base) {
                return stripped.to_string_lossy().to_string();
            }
        }
    }

    file.to_string()
}

pub(crate) fn get_document_path_arg(args: &Value) -> Option<&str> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .and_then(|value| value.as_str())
        .filter(|path| !path.is_empty())
}

pub(crate) fn resolve_search_target(
    path: Option<&str>,
    codebase: Option<&str>,
) -> Result<PathBuf, Value> {
    if let Some(path) = path.filter(|path| !path.is_empty()) {
        return resolve_existing_path(path, codebase);
    }

    let base = codebase
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if base.exists() {
        Ok(base.canonicalize().unwrap_or(base))
    } else {
        Err(json!({"error": format!("Path not found: {}", base.to_string_lossy())}))
    }
}

pub(crate) fn resolve_existing_path(path: &str, codebase: Option<&str>) -> Result<PathBuf, Value> {
    let abs_path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        codebase
            .map(|base| Path::new(base).join(path))
            .unwrap_or_else(|| PathBuf::from(path))
    };
    if abs_path.exists() {
        Ok(abs_path.canonicalize().unwrap_or(abs_path))
    } else {
        Err(json!({"error": format!("Path not found: {}", path)}))
    }
}

pub(crate) fn path_matches(file_path: &str, target_path: &Path, target_is_dir: bool) -> bool {
    let normalized_file =
        std::fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
    if target_is_dir {
        normalized_file == target_path || normalized_file.starts_with(target_path)
    } else {
        normalized_file == target_path
    }
}

/// Convert ast-grep-style pattern to regex.
/// $NAME -> captures a word, $$$ -> captures anything.
pub(crate) fn pattern_to_regex(pattern: &str) -> String {
    let escaped = regex_lite::escape(pattern);
    let triple_re = regex_lite::Regex::new(r"\\\$\\\$\\\$[A-Z_]+").unwrap();
    let result = triple_re.replace_all(&escaped, ".*").to_string();
    let result = result.replace("\\$\\$\\$", ".*");
    let re = regex_lite::Regex::new(r"\\\$[A-Z_]+").unwrap();
    re.replace_all(&result, r"[^\s(),]+").to_string()
}

/// Collect files from a path, optionally filtered by language extension.
/// Uses the `ignore` crate so it respects .gitignore and handles unlimited depth.
pub(crate) fn collect_files(path: &str, language: Option<&str>) -> Vec<String> {
    let p = Path::new(path);
    if p.is_file() {
        return vec![path.to_string()];
    }

    let extensions: Option<Vec<&str>> = language.map(|lang| match lang {
        "python" => vec!["py"],
        "rust" => vec!["rs"],
        "javascript" | "js" => vec!["js", "jsx"],
        "typescript" | "ts" => vec!["ts", "tsx"],
        "go" => vec!["go"],
        "java" => vec!["java"],
        "c" => vec!["c", "h"],
        "cpp" | "c++" => vec!["cpp", "hpp", "cc", "cxx"],
        "ruby" => vec!["rb"],
        _ => vec![],
    });

    let mut files = Vec::new();
    for entry in ignore::Walk::new(p).flatten() {
        let ep = entry.path();
        if !ep.is_file() {
            continue;
        }
        if let Some(exts) = &extensions {
            if let Some(ext) = ep.extension().and_then(|e| e.to_str()) {
                if exts.contains(&ext) {
                    files.push(ep.to_string_lossy().to_string());
                }
            }
        } else {
            // No language filter — include all non-binary files
            if let Some(ext) = ep.extension().and_then(|e| e.to_str()) {
                if ![
                    "png", "jpg", "jpeg", "gif", "svg", "ico", "woff", "woff2", "ttf", "eot",
                    "pdf", "zip", "gz", "tar", "lock", "map", "min.js",
                ]
                .contains(&ext)
                {
                    files.push(ep.to_string_lossy().to_string());
                }
            }
        }
    }
    files
}
