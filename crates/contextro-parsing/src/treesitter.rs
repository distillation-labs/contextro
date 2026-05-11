//! Tree-sitter based multi-language code parser.
//!
//! Extracts symbols, imports, and calls from source files.

use contextro_core::models::{ParsedFile, Symbol, SymbolType};
use contextro_core::traits::Parser;
use contextro_core::ContextroError;

use crate::language::get_language_for_file;

/// Production tree-sitter parser for symbol extraction.
pub struct TreeSitterParser;

impl TreeSitterParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TreeSitterParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for TreeSitterParser {
    fn can_parse(&self, filepath: &str) -> bool {
        get_language_for_file(filepath).is_some()
    }

    fn parse_file(&self, filepath: &str) -> Result<ParsedFile, ContextroError> {
        let language = get_language_for_file(filepath)
            .ok_or_else(|| ContextroError::parse(format!("Unsupported file: {}", filepath)))?;

        let content = std::fs::read_to_string(filepath)
            .map_err(|e| ContextroError::parse(format!("Failed to read {}: {}", filepath, e)))?;

        let start = std::time::Instant::now();
        let symbols = extract_symbols_simple(&content, filepath, language);
        let imports = extract_imports_simple(&content, language);
        let parse_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(ParsedFile {
            filepath: filepath.to_string(),
            language: language.to_string(),
            symbols,
            imports,
            parse_time_ms,
            error: None,
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        // Return a static slice of common extensions
        &[
            ".py", ".js", ".ts", ".rs", ".go", ".java", ".c", ".cpp", ".rb",
        ]
    }
}

/// Simple regex-free symbol extraction using line-by-line heuristics.
/// This is a fast fallback; the full tree-sitter implementation will replace this.
fn extract_symbols_simple(content: &str, filepath: &str, language: &str) -> Vec<Symbol> {
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
            "rust" => {
                if let Some(sym) = parse_rust_def(trimmed, filepath, line_num, &lines, i) {
                    symbols.push(sym);
                }
            }
            "javascript" | "typescript" => {
                if let Some(sym) = parse_js_def(trimmed, filepath, language, line_num, &lines, i) {
                    symbols.push(sym);
                }
            }
            _ => {
                // Generic: look for function-like patterns
                if let Some(sym) = parse_generic_def(trimmed, filepath, language, line_num) {
                    symbols.push(sym);
                }
            }
        }
    }

    symbols
}

fn parse_python_def(
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
    let docstring = extract_python_docstring(lines, idx);
    let calls = extract_python_calls(lines, idx, end_line);

    let code_end = std::cmp::min(end_line, idx + 50);
    let code_snippet = lines[idx..=code_end].join("\n");

    Some(Symbol {
        name,
        symbol_type,
        filepath: filepath.to_string(),
        line_start: line_num,
        line_end: (end_line + 1) as u32,
        language: "python".to_string(),
        signature: line.to_string(),
        docstring: docstring.unwrap_or_default(),
        parent: None,
        code_snippet,
        imports: vec![],
        calls,
    })
}

fn parse_rust_def(
    line: &str,
    filepath: &str,
    line_num: u32,
    lines: &[&str],
    idx: usize,
) -> Option<Symbol> {
    let is_fn = line.contains("fn ")
        && (line.starts_with("pub") || line.starts_with("fn") || line.starts_with("async"));
    let is_struct = line.starts_with("pub struct ") || line.starts_with("struct ");
    let is_impl = line.starts_with("impl ") || line.starts_with("pub impl ");

    if !is_fn && !is_struct && !is_impl {
        return None;
    }

    let (symbol_type, name) = if is_fn {
        let parts: Vec<&str> = line.split("fn ").collect();
        let name_part = parts.get(1)?;
        let name = name_part.split(&['(', '<', ' '][..]).next()?.to_string();
        (SymbolType::Function, name)
    } else if is_struct {
        let parts: Vec<&str> = line.split("struct ").collect();
        let name_part = parts.get(1)?;
        let name = name_part
            .split(&['{', '<', '(', ' '][..])
            .next()?
            .to_string();
        (SymbolType::Class, name)
    } else {
        return None;
    };

    if name.is_empty() {
        return None;
    }

    let end_line = find_block_end_braces(lines, idx);

    Some(Symbol {
        name,
        symbol_type,
        filepath: filepath.to_string(),
        line_start: line_num,
        line_end: (end_line + 1) as u32,
        language: "rust".to_string(),
        signature: line.to_string(),
        docstring: String::new(),
        parent: None,
        code_snippet: String::new(),
        imports: vec![],
        calls: vec![],
    })
}

fn parse_js_def(
    line: &str,
    filepath: &str,
    language: &str,
    line_num: u32,
    lines: &[&str],
    idx: usize,
) -> Option<Symbol> {
    let is_fn = line.starts_with("function ")
        || line.starts_with("export function ")
        || line.starts_with("async function ")
        || line.starts_with("export async function ");
    let is_class = line.starts_with("class ") || line.starts_with("export class ");

    if !is_fn && !is_class {
        return None;
    }

    let (symbol_type, keyword) = if is_fn {
        (SymbolType::Function, "function ")
    } else {
        (SymbolType::Class, "class ")
    };

    let parts: Vec<&str> = line.split(keyword).collect();
    let name_part = parts.get(1)?;
    let name = name_part
        .split(&['(', '{', '<', ' '][..])
        .next()?
        .to_string();

    if name.is_empty() {
        return None;
    }

    let end_line = find_block_end_braces(lines, idx);

    Some(Symbol {
        name,
        symbol_type,
        filepath: filepath.to_string(),
        line_start: line_num,
        line_end: (end_line + 1) as u32,
        language: language.to_string(),
        signature: line.to_string(),
        docstring: String::new(),
        parent: None,
        code_snippet: String::new(),
        imports: vec![],
        calls: vec![],
    })
}

fn parse_generic_def(line: &str, filepath: &str, language: &str, line_num: u32) -> Option<Symbol> {
    // Very basic: look for common function patterns
    if line.contains("func ") || line.contains("def ") || line.contains("function ") {
        let name = line
            .split(&['(', '{', ' '][..])
            .filter(|s| {
                !s.is_empty()
                    && ![
                        "func", "def", "function", "pub", "async", "export", "static",
                    ]
                    .contains(s)
            })
            .next()?
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

fn find_block_end_python(lines: &[&str], start: usize) -> usize {
    if start >= lines.len() {
        return start;
    }
    let indent = lines[start].len() - lines[start].trim_start().len();
    for i in (start + 1)..lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            continue;
        }
        let current_indent = line.len() - line.trim_start().len();
        if current_indent <= indent {
            return i.saturating_sub(1);
        }
    }
    lines.len() - 1
}

fn find_block_end_braces(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    for i in start..lines.len() {
        for ch in lines[i].chars() {
            if ch == '{' {
                depth += 1;
            }
            if ch == '}' {
                depth -= 1;
            }
        }
        if depth <= 0 && i > start {
            return i;
        }
    }
    lines.len() - 1
}

fn extract_python_docstring(lines: &[&str], def_idx: usize) -> Option<String> {
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
        for i in (next_idx + 1)..lines.len() {
            if lines[i].trim().contains(quote) {
                break;
            }
            doc.push_str(lines[i].trim());
            doc.push('\n');
        }
        return Some(doc.trim().to_string());
    }
    None
}

fn extract_python_calls(lines: &[&str], start: usize, end: usize) -> Vec<String> {
    let mut calls = Vec::new();
    for i in (start + 1)..=std::cmp::min(end, lines.len() - 1) {
        let line = lines[i].trim();
        // Simple heuristic: find identifiers followed by (
        for part in line.split(&[' ', '=', ',', '(', '.'][..]) {
            let trimmed = part.trim();
            if trimmed.len() > 1
                && trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic())
                    .unwrap_or(false)
                && line.contains(&format!("{}(", trimmed))
                && ![
                    "if", "for", "while", "return", "print", "self", "cls", "not", "and", "or",
                    "in",
                ]
                .contains(&trimmed)
            {
                if !calls.contains(&trimmed.to_string()) {
                    calls.push(trimmed.to_string());
                }
            }
        }
    }
    calls
}

fn extract_imports_simple(content: &str, language: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        match language {
            "python" if trimmed.starts_with("import ") || trimmed.starts_with("from ") => {
                imports.push(trimmed.to_string());
            }
            "javascript" | "typescript"
                if trimmed.starts_with("import ")
                    || trimmed.starts_with("const ") && trimmed.contains("require(") =>
            {
                imports.push(trimmed.to_string());
            }
            "rust" if trimmed.starts_with("use ") => {
                imports.push(trimmed.to_string());
            }
            "go" if trimmed.starts_with("import ") => {
                imports.push(trimmed.to_string());
            }
            _ => {}
        }
    }
    imports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_python_file() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_parse.py");
        std::fs::write(&tmp, "def hello():\n    \"\"\"Say hello.\"\"\"\n    print(\"hello\")\n\nclass Foo:\n    pass\n").unwrap();

        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        assert!(result.is_successful());
        assert_eq!(result.language, "python");
        assert!(result.symbols.len() >= 2);

        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"Foo"));

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_can_parse() {
        let parser = TreeSitterParser::new();
        assert!(parser.can_parse("main.py"));
        assert!(parser.can_parse("app.ts"));
        assert!(!parser.can_parse("readme.md"));
    }
}
