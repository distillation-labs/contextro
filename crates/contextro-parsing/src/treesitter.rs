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
        &[
            ".py", ".js", ".ts", ".rs", ".go", ".java", ".c", ".cpp", ".rb",
        ]
    }
}

/// Simple regex-free symbol extraction using line-by-line heuristics.
fn extract_symbols_simple(content: &str, filepath: &str, language: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    match language {
        "python" => extract_python_symbols(&lines, filepath, &mut symbols),
        "rust" => extract_rust_symbols(&lines, filepath, &mut symbols),
        "javascript" | "typescript" => {
            extract_js_symbols(&lines, filepath, language, &mut symbols)
        }
        _ => extract_generic_symbols(&lines, filepath, language, &mut symbols),
    }

    symbols
}

// ─── Python ──────────────────────────────────────────────────────────────────

fn extract_python_symbols(lines: &[&str], filepath: &str, symbols: &mut Vec<Symbol>) {
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_num = (i + 1) as u32;
        if let Some(sym) = parse_python_def(trimmed, filepath, line_num, lines, i) {
            symbols.push(sym);
        }
    }
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
    let calls = extract_calls_from_body(lines, idx + 1, end_line);

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

// ─── JavaScript / TypeScript ─────────────────────────────────────────────────

fn extract_js_symbols(lines: &[&str], filepath: &str, language: &str, symbols: &mut Vec<Symbol>) {
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        let line_num = (i + 1) as u32;

        if let Some(sym) = parse_js_symbol(trimmed, filepath, language, line_num, lines, i) {
            let end = sym.line_end as usize;
            // If it's a class, scan inside for methods before advancing
            if sym.symbol_type == SymbolType::Class {
                let class_end = end;
                let mut j = i + 1;
                while j < class_end && j < lines.len() {
                    let inner = lines[j].trim();
                    let inner_line_num = (j + 1) as u32;
                    if let Some(n) = extract_js_method_name(inner) {
                        let method_end = find_block_end_braces(lines, j);
                        let docstring = extract_jsdoc(lines, j);
                        let calls = extract_calls_from_body(lines, j + 1, method_end);
                        let code_end = std::cmp::min(method_end, j + 50);
                        let code_snippet = lines[j..=code_end].join("\n");
                        symbols.push(Symbol {
                            name: n,
                            symbol_type: SymbolType::Method,
                            filepath: filepath.to_string(),
                            line_start: inner_line_num,
                            line_end: (method_end + 1) as u32,
                            language: language.to_string(),
                            signature: inner.to_string(),
                            docstring,
                            parent: Some(sym.name.clone()),
                            code_snippet,
                            imports: vec![],
                            calls,
                        });
                        j = method_end + 1;
                    } else {
                        j += 1;
                    }
                }
            }
            symbols.push(sym);
            i = end;
        } else {
            i += 1;
        }
    }
}

fn parse_js_symbol(
    line: &str,
    filepath: &str,
    language: &str,
    line_num: u32,
    lines: &[&str],
    idx: usize,
) -> Option<Symbol> {
    // 1. function declarations: function foo(, export function foo(, async function foo(
    // 2. class declarations: class Foo, export class Foo
    // 3. arrow/const: const foo = (, export const foo = (, const foo = async (
    // 4. object methods inside class bodies: methodName( or async methodName(

    let (symbol_type, name) = if let Some(n) = extract_js_function_name(line) {
        (SymbolType::Function, n)
    } else if let Some(n) = extract_js_class_name(line) {
        (SymbolType::Class, n)
    } else if let Some(n) = extract_js_arrow_name(line) {
        (SymbolType::Function, n)
    } else if let Some(n) = extract_js_method_name(line) {
        (SymbolType::Method, n)
    } else {
        return None;
    };

    if name.is_empty() || name.len() > 100 {
        return None;
    }

    let end_line = find_block_end_braces(lines, idx);
    let docstring = extract_jsdoc(lines, idx);
    let calls = extract_calls_from_body(lines, idx + 1, end_line);
    let code_end = std::cmp::min(end_line, idx + 50);
    let code_snippet = lines[idx..=code_end].join("\n");

    Some(Symbol {
        name,
        symbol_type,
        filepath: filepath.to_string(),
        line_start: line_num,
        line_end: (end_line + 1) as u32,
        language: language.to_string(),
        signature: line.to_string(),
        docstring,
        parent: None,
        code_snippet,
        imports: vec![],
        calls,
    })
}

/// Matches: function foo(, export function foo(, async function foo(, export async function foo(
fn extract_js_function_name(line: &str) -> Option<String> {
    let s = line
        .strip_prefix("export ")
        .unwrap_or(line);
    let s = s.strip_prefix("async ").unwrap_or(s);
    let s = s.strip_prefix("function ")?;
    // Handle function* for generators
    let s = s.strip_prefix("* ").or(Some(s)).unwrap_or(s);
    let s = s.strip_prefix('*').unwrap_or(s);
    let name = s.split(&['(', '<', ' '][..]).next()?;
    if name.is_empty() { None } else { Some(name.to_string()) }
}

/// Matches: class Foo, export class Foo, export default class Foo, abstract class Foo
fn extract_js_class_name(line: &str) -> Option<String> {
    let s = line
        .strip_prefix("export ")
        .unwrap_or(line);
    let s = s.strip_prefix("default ").unwrap_or(s);
    let s = s.strip_prefix("abstract ").unwrap_or(s);
    let s = s.strip_prefix("class ")?;
    let name = s.split(&['{', '<', ' ', '('][..]).next()?;
    if name.is_empty() { None } else { Some(name.to_string()) }
}

/// Matches: const foo = (, export const foo = (, const foo = async (,
/// const foo = () =>, const Foo: React.FC = (
fn extract_js_arrow_name(line: &str) -> Option<String> {
    let s = line.strip_prefix("export ").unwrap_or(line);
    let s = s.strip_prefix("default ").unwrap_or(s);
    // Must start with const/let/var
    let s = s.strip_prefix("const ")
        .or_else(|| s.strip_prefix("let "))
        .or_else(|| s.strip_prefix("var "))?;
    // Extract name before = or :
    let name = s.split(&['=', ':', '<', ' '][..]).next()?.trim();
    if name.is_empty() || name.len() > 80 {
        return None;
    }
    // Verify it's a function assignment (has = and ( or => somewhere)
    if line.contains('=') && (line.contains('(') || line.contains("=>")) {
        Some(name.to_string())
    } else {
        None
    }
}

/// Matches class methods: async methodName(, methodName(, static methodName(,
/// private methodName(, public async methodName(, get propName(, set propName(
fn extract_js_method_name(line: &str) -> Option<String> {
    // Must have ( but not be a standalone call or control flow
    if !line.contains('(') || line.starts_with("//") || line.starts_with("/*") {
        return None;
    }
    // Skip control flow and common non-method patterns
    let first_word = line.split_whitespace().next().unwrap_or("");
    if ["if", "for", "while", "switch", "return", "import", "from", "throw", "new", "catch", "else"]
        .contains(&first_word)
    {
        return None;
    }

    let s = line;
    // Strip access modifiers and keywords
    let s = s.strip_prefix("static ").unwrap_or(s);
    let s = s.strip_prefix("override ").unwrap_or(s);
    let s = s.strip_prefix("private ").unwrap_or(s);
    let s = s.strip_prefix("protected ").unwrap_or(s);
    let s = s.strip_prefix("public ").unwrap_or(s);
    let s = s.strip_prefix("readonly ").unwrap_or(s);
    let s = s.strip_prefix("abstract ").unwrap_or(s);
    let s = s.strip_prefix("async ").unwrap_or(s);
    let s = s.strip_prefix("get ").unwrap_or(s);
    let s = s.strip_prefix("set ").unwrap_or(s);
    let s = s.strip_prefix("* ").unwrap_or(s); // generator

    // The next token before ( should be the method name
    let name = s.split(&['(', '<', ' '][..]).next()?.trim();
    // Validate: must be a valid identifier
    if name.is_empty()
        || name.len() > 80
        || !name.chars().next().map(|c| c.is_alphabetic() || c == '_' || c == '#').unwrap_or(false)
        || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '#' || c == '$')
    {
        return None;
    }
    // Skip if it looks like a function call (no { or => on the line, or ends with ;)
    if !line.contains('{') && !line.contains("=>") && !line.ends_with('{') {
        // Could be an interface method signature (ends with ; or has no body)
        // Still extract it as a method
        if !line.ends_with(';') && !line.ends_with(',') {
            return None;
        }
    }
    Some(name.to_string())
}

/// Extract JSDoc comment above a definition.
fn extract_jsdoc(lines: &[&str], def_idx: usize) -> String {
    if def_idx == 0 {
        return String::new();
    }
    // Walk backwards to find /** ... */
    let prev = lines[def_idx - 1].trim();
    if prev.ends_with("*/") {
        let mut doc_lines = Vec::new();
        let mut j = def_idx - 1;
        loop {
            let l = lines[j].trim();
            doc_lines.push(
                l.trim_start_matches("/**")
                    .trim_start_matches("/*")
                    .trim_start_matches('*')
                    .trim_end_matches("*/")
                    .trim(),
            );
            if l.starts_with("/**") || l.starts_with("/*") || j == 0 {
                break;
            }
            j -= 1;
        }
        doc_lines.reverse();
        let doc: String = doc_lines
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        return doc;
    }
    // Single-line // comment
    if prev.starts_with("//") {
        return prev.trim_start_matches('/').trim().to_string();
    }
    String::new()
}

// ─── Rust ────────────────────────────────────────────────────────────────────

fn extract_rust_symbols(lines: &[&str], filepath: &str, symbols: &mut Vec<Symbol>) {
    let mut i = 0;
    let mut current_impl: Option<String> = None;
    let mut impl_depth: i32 = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        let line_num = (i + 1) as u32;

        // Track impl blocks for parent context
        if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
            if let Some(name) = extract_rust_impl_name(trimmed) {
                current_impl = Some(name);
                impl_depth = 0;
                // Count braces on this line
                for ch in trimmed.chars() {
                    if ch == '{' { impl_depth += 1; }
                    if ch == '}' { impl_depth -= 1; }
                }
                i += 1;
                continue;
            }
        }

        // Track brace depth for impl scope
        if current_impl.is_some() {
            for ch in trimmed.chars() {
                if ch == '{' { impl_depth += 1; }
                if ch == '}' { impl_depth -= 1; }
            }
            if impl_depth <= 0 {
                current_impl = None;
            }
        }

        if let Some(sym) = parse_rust_symbol(trimmed, filepath, line_num, lines, i, &current_impl) {
            i = sym.line_end as usize;
            symbols.push(sym);
        } else {
            i += 1;
        }
    }
}

fn extract_rust_impl_name(line: &str) -> Option<String> {
    // impl Foo { or impl<T> Foo<T> { or impl Trait for Foo {
    let s = line.strip_prefix("impl").unwrap_or(line);
    // Skip generic params
    let s = if s.starts_with('<') {
        // Find matching >
        let mut depth = 0;
        let mut end = 0;
        for (j, ch) in s.chars().enumerate() {
            if ch == '<' { depth += 1; }
            if ch == '>' { depth -= 1; if depth == 0 { end = j + 1; break; } }
        }
        &s[end..]
    } else {
        s
    };
    let s = s.trim();
    // If "Trait for Type", extract Type
    if let Some(pos) = s.find(" for ") {
        let after_for = &s[pos + 5..];
        let name = after_for.split(&['{', '<', ' '][..]).next()?.trim();
        if !name.is_empty() { return Some(name.to_string()); }
    }
    let name = s.split(&['{', '<', ' '][..]).next()?.trim();
    if name.is_empty() { None } else { Some(name.to_string()) }
}

fn parse_rust_symbol(
    line: &str,
    filepath: &str,
    line_num: u32,
    lines: &[&str],
    idx: usize,
    current_impl: &Option<String>,
) -> Option<Symbol> {
    let is_fn = line.contains("fn ")
        && (line.starts_with("pub")
            || line.starts_with("fn")
            || line.starts_with("async")
            || line.starts_with("unsafe")
            || line.starts_with("const fn")
            || line.starts_with("extern")
            // Methods inside impl blocks (indented)
            || lines[idx].starts_with("    ")
            || lines[idx].starts_with("\t"));
    let is_struct = line.starts_with("pub struct ") || line.starts_with("struct ");
    let is_enum = line.starts_with("pub enum ") || line.starts_with("enum ");
    let is_trait = line.starts_with("pub trait ") || line.starts_with("trait ");

    if !is_fn && !is_struct && !is_enum && !is_trait {
        return None;
    }

    let (symbol_type, name) = if is_fn {
        let parts: Vec<&str> = line.split("fn ").collect();
        let name_part = parts.get(1)?;
        let name = name_part.split(&['(', '<', ' '][..]).next()?.to_string();
        let st = if current_impl.is_some() {
            SymbolType::Method
        } else {
            SymbolType::Function
        };
        (st, name)
    } else if is_struct {
        let parts: Vec<&str> = line.split("struct ").collect();
        let name_part = parts.get(1)?;
        let name = name_part.split(&['{', '<', '(', ' ', ';'][..]).next()?.to_string();
        (SymbolType::Class, name)
    } else if is_enum {
        let parts: Vec<&str> = line.split("enum ").collect();
        let name_part = parts.get(1)?;
        let name = name_part.split(&['{', '<', ' '][..]).next()?.to_string();
        (SymbolType::Class, name)
    } else {
        // trait
        let parts: Vec<&str> = line.split("trait ").collect();
        let name_part = parts.get(1)?;
        let name = name_part.split(&['{', '<', ':', ' '][..]).next()?.to_string();
        (SymbolType::Class, name)
    };

    if name.is_empty() {
        return None;
    }

    let end_line = find_block_end_braces(lines, idx);
    let docstring = extract_rust_doc_comment(lines, idx);
    let calls = if is_fn {
        extract_calls_from_body(lines, idx + 1, end_line)
    } else {
        vec![]
    };
    let code_end = std::cmp::min(end_line, idx + 50);
    let code_snippet = lines[idx..=code_end].join("\n");

    Some(Symbol {
        name,
        symbol_type,
        filepath: filepath.to_string(),
        line_start: line_num,
        line_end: (end_line + 1) as u32,
        language: "rust".to_string(),
        signature: line.to_string(),
        docstring,
        parent: current_impl.clone(),
        code_snippet,
        imports: vec![],
        calls,
    })
}

/// Extract /// doc comments above a definition.
fn extract_rust_doc_comment(lines: &[&str], def_idx: usize) -> String {
    if def_idx == 0 {
        return String::new();
    }
    let mut doc_lines = Vec::new();
    let mut j = def_idx - 1;
    loop {
        let l = lines[j].trim();
        if l.starts_with("///") {
            doc_lines.push(l.trim_start_matches('/').trim());
        } else if l.starts_with("#[") || l.is_empty() {
            // Skip attributes and blank lines between doc and def
            if !l.is_empty() && !l.starts_with("#[") {
                break;
            }
        } else {
            break;
        }
        if j == 0 {
            break;
        }
        j -= 1;
    }
    doc_lines.reverse();
    doc_lines.join(" ")
}

// ─── Generic fallback ────────────────────────────────────────────────────────

fn extract_generic_symbols(lines: &[&str], filepath: &str, language: &str, symbols: &mut Vec<Symbol>) {
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_num = (i + 1) as u32;
        if let Some(sym) = parse_generic_def(trimmed, filepath, language, line_num) {
            symbols.push(sym);
        }
    }
}

fn parse_generic_def(line: &str, filepath: &str, language: &str, line_num: u32) -> Option<Symbol> {
    if line.contains("func ") || line.contains("def ") || line.contains("function ") {
        let name = line
            .split(&['(', '{', ' '][..])
            .find(|s| {
                !s.is_empty()
                    && !["func", "def", "function", "pub", "async", "export", "static"]
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

// ─── Shared helpers ──────────────────────────────────────────────────────────

/// Extract function calls and JSX component usages from a block of code lines.
/// Looks for identifier( patterns and <ComponentName patterns, filtering keywords.
fn extract_calls_from_body(lines: &[&str], start: usize, end: usize) -> Vec<String> {
    let mut calls = Vec::new();
    let upper = std::cmp::min(end, lines.len().saturating_sub(1));

    let keywords: &[&str] = &[
        "if", "for", "while", "return", "print", "self", "cls", "not", "and", "or",
        "in", "switch", "case", "else", "new", "typeof", "instanceof", "delete",
        "void", "throw", "catch", "try", "finally", "yield", "await", "match",
        "let", "const", "var", "mut", "ref", "pub", "fn", "use", "mod", "impl",
        "struct", "enum", "trait", "type", "where", "as", "super", "crate",
    ];

    for line in lines.iter().take(upper + 1).skip(start) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        let bytes = trimmed.as_bytes();
        let len = bytes.len();
        let mut j = 0;
        while j < len {
            // identifier( — function call
            if bytes[j] == b'(' && j > 0 {
                let end_pos = j;
                let mut k = j - 1;
                if bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'$' {
                    while k > 0 && (bytes[k-1].is_ascii_alphanumeric() || bytes[k-1] == b'_' || bytes[k-1] == b'$') {
                        k -= 1;
                    }
                    let candidate = &trimmed[k..end_pos];
                    if candidate.len() > 1
                        && !keywords.contains(&candidate)
                        && candidate.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false)
                        && !calls.contains(&candidate.to_string())
                    {
                        calls.push(candidate.to_string());
                    }
                }
            }
            // <ComponentName — JSX usage (uppercase first letter = component, not HTML tag)
            if bytes[j] == b'<' && j + 1 < len {
                let rest = &trimmed[j+1..];
                // Skip closing tags </Foo> and fragments <>
                if rest.starts_with('/') || rest.starts_with('>') {
                    j += 1;
                    continue;
                }
                let name_end = rest.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(rest.len());
                let component = &rest[..name_end];
                if !component.is_empty()
                    && component.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && !calls.contains(&component.to_string())
                {
                    calls.push(component.to_string());
                }
            }
            j += 1;
        }
    }
    calls
}

fn find_block_end_python(lines: &[&str], start: usize) -> usize {
    if start >= lines.len() {
        return start;
    }
    let indent = lines[start].len() - lines[start].trim_start().len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
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
    for (i, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
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
                    || (trimmed.starts_with("const ") && trimmed.contains("require(")) =>
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_python_file() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_parse.py");
        std::fs::write(
            &tmp,
            "def hello():\n    \"\"\"Say hello.\"\"\"\n    print(\"hello\")\n\nclass Foo:\n    pass\n",
        )
        .unwrap();

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
    fn test_js_arrow_functions() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_parse_arrow.ts");
        std::fs::write(
            &tmp,
            r#"export const fetchUser = async (id: string) => {
    const result = await db.query(id);
    return transform(result);
};

export function processData(data: any) {
    return validate(data);
}

class UserService {
    async getUser(id: string) {
        return fetchUser(id);
    }
}
"#,
        )
        .unwrap();

        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"fetchUser"), "Missing arrow fn: {:?}", names);
        assert!(names.contains(&"processData"), "Missing function: {:?}", names);
        assert!(names.contains(&"UserService"), "Missing class: {:?}", names);
        assert!(names.contains(&"getUser"), "Missing method: {:?}", names);

        // Check calls are extracted
        let fetch_sym = result.symbols.iter().find(|s| s.name == "fetchUser").unwrap();
        assert!(!fetch_sym.calls.is_empty(), "fetchUser should have calls");
        assert!(fetch_sym.calls.contains(&"transform".to_string()));

        // Check code_snippet is populated
        assert!(!fetch_sym.code_snippet.is_empty(), "code_snippet should be populated");

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_rust_impl_methods() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_parse_impl.rs");
        std::fs::write(
            &tmp,
            r#"/// A user store.
pub struct UserStore {
    db: Database,
}

impl UserStore {
    /// Create a new store.
    pub fn new(db: Database) -> Self {
        Self { db: validate(db) }
    }

    pub async fn get(&self, id: &str) -> Result<User> {
        let raw = self.db.query(id).await?;
        deserialize(raw)
    }
}

pub fn standalone() {
    helper();
}
"#,
        )
        .unwrap();

        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"UserStore"), "Missing struct: {:?}", names);
        assert!(names.contains(&"new"), "Missing method new: {:?}", names);
        assert!(names.contains(&"get"), "Missing method get: {:?}", names);
        assert!(names.contains(&"standalone"), "Missing fn: {:?}", names);

        // Check calls
        let new_sym = result.symbols.iter().find(|s| s.name == "new").unwrap();
        assert!(new_sym.calls.contains(&"validate".to_string()), "new should call validate: {:?}", new_sym.calls);

        let get_sym = result.symbols.iter().find(|s| s.name == "get").unwrap();
        assert!(get_sym.calls.contains(&"deserialize".to_string()), "get should call deserialize: {:?}", get_sym.calls);

        // Check docstring
        let store_sym = result.symbols.iter().find(|s| s.name == "UserStore").unwrap();
        assert!(store_sym.docstring.contains("user store"), "Missing docstring: {:?}", store_sym.docstring);

        // Check parent
        assert_eq!(new_sym.parent, Some("UserStore".to_string()));

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_can_parse() {
        let parser = TreeSitterParser::new();
        assert!(parser.can_parse("main.py"));
        assert!(parser.can_parse("app.ts"));
        assert!(!parser.can_parse("readme.md"));
    }

    #[test]
    fn test_extract_calls_from_body() {
        let lines = vec![
            "    const x = foo(bar());",
            "    if (condition) {",
            "        await fetchData(id);",
            "    }",
        ];
        let calls = extract_calls_from_body(&lines, 0, 3);
        assert!(calls.contains(&"foo".to_string()));
        assert!(calls.contains(&"bar".to_string()));
        assert!(calls.contains(&"fetchData".to_string()));
        // 'condition' should NOT be in calls (no parens after it)
    }
}
