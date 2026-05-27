use super::*;

pub(crate) fn pattern_search(args: &Value, codebase: Option<&str>) -> Value {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    if pattern.is_empty() {
        return json!({"error": "Missing required parameter: pattern"});
    }
    let language = args.get("language").and_then(|v| v.as_str());
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let search_path = args.get("path").and_then(|v| v.as_str());

    let target = match resolve_search_target(file_path.or(search_path), codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };

    // If the pattern contains ast-grep metavariables ($NAME, $$$), convert them.
    // Otherwise, treat the pattern as a regex first; fall back to literal string
    // matching if the regex is invalid. This lets callers use "impl.*Engine" style
    // patterns without needing to know about ast-grep syntax.
    let re = if pattern.contains('$') {
        let regex_pattern = pattern_to_regex(pattern);
        regex_lite::Regex::new(&regex_pattern)
            .unwrap_or_else(|_| regex_lite::Regex::new(&regex_lite::escape(pattern)).unwrap())
    } else {
        regex_lite::Regex::new(pattern)
            .unwrap_or_else(|_| regex_lite::Regex::new(&regex_lite::escape(pattern)).unwrap())
    };

    let mut matches: Vec<Value> = Vec::new();
    let files = collect_files(target.to_string_lossy().as_ref(), language);

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (line_num, line) in content.lines().enumerate() {
            if re.is_match(line) {
                matches.push(json!({
                    "file": strip_base(file, codebase),
                    "line": line_num + 1,
                    "code": line.trim(),
                }));
                if matches.len() >= 50 {
                    return json!({"pattern": pattern, "matches": matches, "total": matches.len(), "truncated": true});
                }
            }
        }
    }

    json!({"pattern": pattern, "matches": matches, "total": matches.len()})
}

/// Pattern rewrite: find and replace using structural patterns.
pub(crate) fn pattern_rewrite(args: &Value, codebase: Option<&str>) -> Value {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    let replacement = args
        .get("replacement")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if pattern.is_empty() || replacement.is_empty() {
        return json!({"error": "Missing required parameters: pattern and replacement"});
    }

    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let search_path = args.get("path").and_then(|v| v.as_str());
    let language = args.get("language").and_then(|v| v.as_str());
    let target = match resolve_search_target(file_path.or(search_path), codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };

    let regex_pattern = pattern_to_regex(pattern);
    let re = match regex_lite::Regex::new(&regex_pattern) {
        Ok(r) => r,
        Err(_) => return json!({"error": "Invalid pattern"}),
    };

    let files = collect_files(target.to_string_lossy().as_ref(), language);
    let mut changes: Vec<Value> = Vec::new();
    let mut total_replacements = 0;

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let new_content = re.replace_all(&content, replacement);
        if new_content != content {
            let count = content
                .lines()
                .zip(new_content.lines())
                .filter(|(a, b)| a != b)
                .count();
            total_replacements += count;

            // Generate diff
            let diff_lines: Vec<String> = content
                .lines()
                .zip(new_content.lines())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .take(5)
                .map(|(i, (old, new))| {
                    format!("L{}: -{}\nL{}: +{}", i + 1, old.trim(), i + 1, new.trim())
                })
                .collect();

            changes.push(json!({
                "file": strip_base(file, codebase),
                "replacements": count,
                "diff_preview": diff_lines.join("\n"),
            }));

            if !dry_run {
                std::fs::write(file, new_content.as_ref()).ok();
            }
        }
    }

    json!({
        "pattern": pattern,
        "replacement": replacement,
        "dry_run": dry_run,
        "changes": changes,
        "total_files": changes.len(),
        "total_replacements": total_replacements,
    })
}
