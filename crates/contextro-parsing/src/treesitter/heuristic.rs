use super::shared::is_keyword;
use super::*;

pub(super) fn parse_heuristic(content: &str, filepath: &str, language: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_num = (i + 1) as u32;

        match language {
            "python" => {
                if let Some(sym) = parse_python_def(trimmed, filepath, line_num, &lines, i) {
                    symbols.push(sym);
                }
            }
            _ => {
                if let Some(sym) = parse_generic_def(trimmed, filepath, language, line_num) {
                    symbols.push(sym);
                }
            }
        }
    }
    symbols
}

pub(super) fn parse_python_def(
    line: &str,
    filepath: &str,
    line_num: u32,
    lines: &[&str],
    idx: usize,
) -> Option<Symbol> {
    let (symbol_type, prefix) = if line.starts_with("def ") || line.starts_with("async def ") {
        (
            SymbolType::Function,
            if line.starts_with("async") {
                "async def "
            } else {
                "def "
            },
        )
    } else if line.starts_with("class ") {
        (SymbolType::Class, "class ")
    } else {
        return None;
    };

    let rest = line.strip_prefix(prefix)?;
    let name = rest.split(&['(', ':', ' '][..]).next()?.to_string();
    if name.is_empty() {
        return None;
    }

    let end_line = find_block_end_python(lines, idx);
    let calls = collect_calls_heuristic(lines, idx + 1, end_line);
    let code_end = end_line.min(idx + 50);
    let code_snippet = lines[idx..=code_end].join("\n");

    Some(Symbol {
        name,
        symbol_type,
        filepath: filepath.to_string(),
        line_start: line_num,
        line_end: (end_line + 1) as u32,
        language: "python".to_string(),
        signature: line.to_string(),
        docstring: extract_python_docstring(lines, idx).unwrap_or_default(),
        parent: None,
        code_snippet,
        imports: vec![],
        calls,
    })
}

pub(super) fn parse_generic_def(
    line: &str,
    filepath: &str,
    language: &str,
    line_num: u32,
) -> Option<Symbol> {
    if line.contains("func ") || line.contains("def ") || line.contains("function ") {
        let name = line
            .split(&['(', '{', ' '][..])
            .find(|s| {
                !s.is_empty()
                    && ![
                        "func", "def", "function", "pub", "async", "export", "static",
                    ]
                    .contains(s)
            })?
            .to_string();
        if name.is_empty() || name.len() > 100 {
            return None;
        }
        return Some(Symbol {
            name,
            symbol_type: SymbolType::Function,
            filepath: filepath.to_string(),
            line_start: line_num,
            line_end: line_num,
            language: language.to_string(),
            signature: line.to_string(),
            docstring: String::new(),
            parent: None,
            code_snippet: String::new(),
            imports: vec![],
            calls: vec![],
        });
    }
    None
}

pub(super) fn collect_calls_heuristic(lines: &[&str], start: usize, end: usize) -> Vec<String> {
    let mut calls = Vec::new();
    let upper = end.min(lines.len().saturating_sub(1));
    for line in lines.iter().take(upper + 1).skip(start) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        let bytes = trimmed.as_bytes();
        let len = bytes.len();
        let mut j = 0;
        while j < len {
            if bytes[j] == b'(' && j > 0 {
                let mut k = j - 1;
                if bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' {
                    while k > 0 && (bytes[k - 1].is_ascii_alphanumeric() || bytes[k - 1] == b'_') {
                        k -= 1;
                    }
                    let candidate = &trimmed[k..j];
                    if candidate.len() > 1
                        && !is_keyword(candidate)
                        && candidate
                            .chars()
                            .next()
                            .map(|c| c.is_alphabetic())
                            .unwrap_or(false)
                        && !calls.contains(&candidate.to_string())
                    {
                        calls.push(candidate.to_string());
                    }
                }
            }
            // JSX
            if bytes[j] == b'<' && j + 1 < len && !trimmed[j + 1..].starts_with('/') {
                let rest = &trimmed[j + 1..];
                let name_end = rest
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(rest.len());
                let comp = &rest[..name_end];
                if !comp.is_empty()
                    && comp
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    && !calls.contains(&comp.to_string())
                {
                    calls.push(comp.to_string());
                }
            }
            j += 1;
        }
    }
    calls
}

pub(super) fn find_block_end_python(lines: &[&str], start: usize) -> usize {
    if start >= lines.len() {
        return start;
    }
    let indent = lines[start].len() - lines[start].trim_start().len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim().is_empty() {
            continue;
        }
        if line.len() - line.trim_start().len() <= indent {
            return i.saturating_sub(1);
        }
    }
    lines.len() - 1
}

pub(super) fn extract_python_docstring(lines: &[&str], def_idx: usize) -> Option<String> {
    let next_idx = def_idx + 1;
    if next_idx >= lines.len() {
        return None;
    }
    let next_line = lines[next_idx].trim();
    if next_line.starts_with("\"\"\"") || next_line.starts_with("'''") {
        let quote = &next_line[..3];
        if next_line.len() > 6 && next_line.ends_with(quote) {
            return Some(next_line[3..next_line.len() - 3].to_string());
        }
        let mut doc = String::new();
        for line in lines.iter().skip(next_idx + 1) {
            if line.trim().contains(quote) {
                break;
            }
            doc.push_str(line.trim());
            doc.push('\n');
        }
        return Some(doc.trim().to_string());
    }
    None
}

pub(super) fn extract_imports(content: &str, language: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        match language {
            "python" if t.starts_with("import ") || t.starts_with("from ") => {
                imports.push(t.to_string())
            }
            "javascript" | "typescript"
                if t.starts_with("import ")
                    || (t.starts_with("const ") && t.contains("require(")) =>
            {
                imports.push(t.to_string())
            }
            "rust" if t.starts_with("use ") => imports.push(t.to_string()),
            "go" if t.starts_with("import ") => imports.push(t.to_string()),
            _ => {}
        }
    }
    imports
}
