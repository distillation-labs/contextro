use super::heuristic::collect_calls_heuristic;
use super::shared::*;
use super::*;

pub(super) fn parse_rust(content: &str, filepath: &str) -> Vec<Symbol> {
    // tree-sitter-rust 0.24 requires ABI 15 which is incompatible with tree-sitter 0.24.7.
    // Use the heuristic parser which already handles impl blocks, methods, calls, and docstrings.
    parse_rust_heuristic(content, filepath)
}

pub(super) fn parse_rust_heuristic(content: &str, filepath: &str) -> Vec<Symbol> {
    let lines: Vec<&str> = content.lines().collect();
    let module_doc = extract_rust_module_doc(&lines);
    let mut symbols = Vec::new();
    let mut i = 0;
    let mut current_impl: Option<String> = None;
    let mut impl_depth: i32 = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        let line_num = (i + 1) as u32;

        // Track impl blocks
        if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
            if let Some(name) = extract_rust_impl_name(trimmed) {
                current_impl = Some(name);
                impl_depth = 0;
                for ch in trimmed.chars() {
                    if ch == '{' {
                        impl_depth += 1;
                    }
                    if ch == '}' {
                        impl_depth -= 1;
                    }
                }
                i += 1;
                continue;
            }
        }

        if current_impl.is_some() {
            for ch in trimmed.chars() {
                if ch == '{' {
                    impl_depth += 1;
                }
                if ch == '}' {
                    impl_depth -= 1;
                }
            }
            if impl_depth <= 0 {
                current_impl = None;
            }
        }

        let is_fn = trimmed.contains("fn ")
            && (trimmed.starts_with("pub")
                || trimmed.starts_with("fn")
                || trimmed.starts_with("async")
                || trimmed.starts_with("unsafe")
                || lines[i].starts_with("    ")
                || lines[i].starts_with("\t"));
        let is_struct = trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ");
        let is_enum = trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ");
        let is_trait = trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ");

        if is_fn {
            if let Some(name) = trimmed
                .split("fn ")
                .nth(1)
                .and_then(|s| s.split(&['(', '<', ' '][..]).next())
            {
                if !name.is_empty() {
                    let end_line = find_block_end_braces(&lines, i);
                    if current_impl.is_some() && end_line > i {
                        apply_brace_depth_delta(&lines, i + 1, end_line, &mut impl_depth);
                    }
                    let calls = collect_calls_heuristic(&lines, i + 1, end_line);
                    let st = if current_impl.is_some() {
                        SymbolType::Method
                    } else {
                        SymbolType::Function
                    };
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: st,
                        filepath: filepath.to_string(),
                        line_start: line_num,
                        line_end: (end_line + 1) as u32,
                        language: "rust".to_string(),
                        signature: trimmed.to_string(),
                        docstring: merge_rust_doc_context(
                            &extract_rust_item_doc(&lines, i),
                            &module_doc,
                        ),
                        parent: current_impl.clone(),
                        code_snippet: snippet_from_lines(&lines, i, end_line),
                        imports: vec![],
                        calls,
                    });
                    i = end_line + 1;
                    continue;
                }
            }
        } else if is_struct || is_enum || is_trait {
            let keyword = if is_struct {
                "struct "
            } else if is_enum {
                "enum "
            } else {
                "trait "
            };
            if let Some(name) = trimmed
                .split(keyword)
                .nth(1)
                .and_then(|s| s.split(&['{', '<', '(', ' ', ';', ':'][..]).next())
            {
                if !name.is_empty() {
                    let end_line = if trimmed.ends_with(';') {
                        i
                    } else {
                        find_block_end_braces(&lines, i)
                    };
                    symbols.push(Symbol {
                        name: name.to_string(),
                        symbol_type: SymbolType::Class,
                        filepath: filepath.to_string(),
                        line_start: line_num,
                        line_end: (end_line + 1) as u32,
                        language: "rust".to_string(),
                        signature: trimmed.to_string(),
                        docstring: merge_rust_doc_context(
                            &extract_rust_item_doc(&lines, i),
                            &module_doc,
                        ),
                        parent: None,
                        code_snippet: snippet_from_lines(&lines, i, end_line),
                        imports: vec![],
                        calls: vec![],
                    });
                    i = end_line + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    symbols
}

pub(super) fn extract_rust_impl_name(line: &str) -> Option<String> {
    let s = line.strip_prefix("impl").unwrap_or(line);
    let s = if s.starts_with('<') {
        let mut depth = 0;
        let mut end = 0;
        for (j, ch) in s.chars().enumerate() {
            if ch == '<' {
                depth += 1;
            }
            if ch == '>' {
                depth -= 1;
                if depth == 0 {
                    end = j + 1;
                    break;
                }
            }
        }
        &s[end..]
    } else {
        s
    };
    let s = s.trim();
    if let Some(pos) = s.find(" for ") {
        let after = &s[pos + 5..];
        let name = after.split(&['{', '<', ' '][..]).next()?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    let name = s.split(&['{', '<', ' '][..]).next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub(super) fn find_block_end_braces(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    let mut saw_open_brace = false;
    for (i, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                saw_open_brace = true;
            }
            if ch == '}' && saw_open_brace {
                depth -= 1;
            }
        }
        if !saw_open_brace {
            continue;
        }
        if depth <= 0 {
            return i;
        }
    }
    lines.len() - 1
}

pub(super) fn apply_brace_depth_delta(lines: &[&str], start: usize, end: usize, depth: &mut i32) {
    let upper = end.min(lines.len().saturating_sub(1));
    for line in lines.iter().take(upper + 1).skip(start) {
        for ch in line.chars() {
            if ch == '{' {
                *depth += 1;
            }
            if ch == '}' {
                *depth -= 1;
            }
        }
    }
}

// ─── Shared helpers ──────────────────────────────────────────────────────────

/// Extract leading doc comments for the item starting at `row`.
pub(super) fn extract_rust_item_doc(lines: &[&str], row: usize) -> String {
    if row == 0 || lines.is_empty() {
        return String::new();
    }

    let mut doc_lines = Vec::new();
    let mut j = row;
    while j > 0 {
        j -= 1;
        let l = lines.get(j).unwrap_or(&"").trim();

        if l.starts_with("///") {
            push_doc_line(&mut doc_lines, l.trim_start_matches("///").trim());
        } else if l.ends_with("*/") {
            if let Some((start, mut block_lines)) = extract_rust_doc_block_above(lines, j) {
                block_lines.reverse();
                doc_lines.extend(block_lines);
                j = start;
            } else {
                break;
            }
        } else if l.starts_with("#[") || l.is_empty() {
            continue;
        } else {
            break;
        }
    }
    doc_lines.reverse();
    normalize_doc_lines(doc_lines)
}

pub(super) fn extract_rust_module_doc(lines: &[&str]) -> String {
    let mut doc_lines = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty()
            || (i == 0 && trimmed.starts_with("#!") && !trimmed.starts_with("#!["))
            || trimmed.starts_with("#![")
        {
            i += 1;
            continue;
        }

        if trimmed.starts_with("//!") {
            push_doc_line(&mut doc_lines, trimmed.trim_start_matches("//!").trim());
            i += 1;
            continue;
        }

        if trimmed.starts_with("/*!") {
            let (next, block_lines) = extract_rust_doc_block_below(lines, i);
            if block_lines.is_empty() {
                break;
            }
            doc_lines.extend(block_lines);
            i = next;
            continue;
        }

        break;
    }

    truncate_doc(&normalize_doc_lines(doc_lines), 220)
}

pub(super) fn extract_rust_doc_block_above(
    lines: &[&str],
    end: usize,
) -> Option<(usize, Vec<String>)> {
    let mut start = end;
    loop {
        let trimmed = lines.get(start)?.trim();
        if trimmed.starts_with("/**") || trimmed.starts_with("/*!") {
            let mut block_lines = Vec::new();
            for line in lines.iter().take(end + 1).skip(start) {
                push_doc_line(&mut block_lines, clean_rust_doc_line(line.trim()).as_str());
            }
            return Some((start, block_lines));
        }
        if start == 0 || trimmed.starts_with("/*") {
            return None;
        }
        start -= 1;
    }
}

pub(super) fn extract_rust_doc_block_below(lines: &[&str], start: usize) -> (usize, Vec<String>) {
    let mut block_lines = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        push_doc_line(&mut block_lines, clean_rust_doc_line(trimmed).as_str());
        i += 1;
        if trimmed.ends_with("*/") {
            break;
        }
    }

    (i, block_lines)
}

pub(super) fn clean_rust_doc_line(line: &str) -> String {
    line.trim()
        .trim_start_matches("/**")
        .trim_start_matches("/*!")
        .trim_start_matches("/*")
        .trim_start_matches('*')
        .trim_end_matches("*/")
        .trim()
        .to_string()
}

pub(super) fn push_doc_line(doc_lines: &mut Vec<String>, line: &str) {
    let trimmed = line.trim();
    if !trimmed.is_empty() {
        doc_lines.push(trimmed.to_string());
    }
}

pub(super) fn normalize_doc_lines(lines: Vec<String>) -> String {
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn merge_rust_doc_context(item_doc: &str, module_doc: &str) -> String {
    let item_doc = truncate_doc(item_doc.trim(), 360);
    let module_doc = truncate_doc(module_doc.trim(), 220);

    if item_doc.is_empty() {
        return module_doc;
    }
    if module_doc.is_empty() || item_doc.chars().count() > 220 {
        return item_doc;
    }

    let normalized_item = normalize_identifierish(&item_doc);
    let normalized_module = normalize_identifierish(&module_doc);
    if !normalized_module.is_empty() && normalized_item.contains(&normalized_module) {
        return item_doc;
    }

    truncate_doc(
        &format!("{}\n\nModule context: {}", item_doc, module_doc),
        500,
    )
}

pub(super) fn truncate_doc(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

pub(super) fn normalize_identifierish(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}
