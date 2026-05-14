//! Real tree-sitter based multi-language code parser.
//!
//! Uses tree-sitter grammars to parse source files into ASTs, then extracts
//! symbols (functions, classes, methods, interfaces, enums, types) and call
//! relationships (function calls + JSX component usage).

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

        let symbols = match language {
            "typescript" | "javascript" => parse_ts_js(&content, filepath, language),
            "rust" => parse_rust(&content, filepath),
            "python" => parse_python(&content, filepath),
            "go" => parse_go(&content, filepath),
            "java" => parse_java(&content, filepath),
            _ => parse_heuristic(&content, filepath, language),
        };

        let imports = extract_imports(&content, language);
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

// ─── TypeScript / JavaScript (real tree-sitter) ──────────────────────────────

fn parse_ts_js(content: &str, filepath: &str, language: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    let ts_lang = if filepath.ends_with(".tsx") || filepath.ends_with(".jsx") {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else if language == "typescript" {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    } else {
        tree_sitter_javascript::LANGUAGE.into()
    };
    if parser.set_language(&ts_lang).is_err() {
        return parse_heuristic(content, filepath, language);
    }

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return parse_heuristic(content, filepath, language),
    };

    let root = tree.root_node();
    let mut symbols = Vec::new();
    let source = content.as_bytes();

    extract_ts_symbols(root, source, filepath, language, None, &mut symbols);
    symbols
}

fn extract_ts_symbols(
    node: tree_sitter::Node,
    source: &[u8],
    filepath: &str,
    language: &str,
    parent_name: Option<&str>,
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_declaration" => {
                if let Some(sym) =
                    extract_ts_function(child, source, filepath, language, parent_name)
                {
                    symbols.push(sym);
                }
            }
            "export_statement" => {
                // Recurse into export to find the declaration inside
                extract_ts_symbols(child, source, filepath, language, parent_name, symbols);
            }
            "lexical_declaration" => {
                // const foo = () => {} or const Foo: React.FC = ...
                extract_ts_variable_decls(child, source, filepath, language, parent_name, symbols);
            }
            "class_declaration" => {
                if let Some(name) = child_text_by_kind(child, "type_identifier", source) {
                    let (start, end) = (
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                    );
                    let sig = line_at(source, child.start_position().row);
                    let doc = extract_jsdoc_above(source, child.start_position().row);
                    let snippet =
                        snippet_from(source, child.start_position().row, child.end_position().row);

                    symbols.push(Symbol {
                        name: name.clone(),
                        symbol_type: SymbolType::Class,
                        filepath: filepath.to_string(),
                        line_start: start,
                        line_end: end,
                        language: language.to_string(),
                        signature: sig,
                        docstring: doc,
                        parent: parent_name.map(String::from),
                        code_snippet: snippet,
                        imports: vec![],
                        calls: vec![],
                    });

                    // Extract methods inside class body
                    if let Some(body) = child.child_by_field_name("body") {
                        extract_ts_symbols(body, source, filepath, language, Some(&name), symbols);
                    }
                }
            }
            "method_definition" => {
                if let Some(name) = child_text_by_kind(child, "property_identifier", source) {
                    let calls = collect_calls(child, source);
                    let (start, end) = (
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                    );
                    let sig = line_at(source, child.start_position().row);
                    let doc = extract_jsdoc_above(source, child.start_position().row);
                    let snippet =
                        snippet_from(source, child.start_position().row, child.end_position().row);

                    symbols.push(Symbol {
                        name,
                        symbol_type: SymbolType::Method,
                        filepath: filepath.to_string(),
                        line_start: start,
                        line_end: end,
                        language: language.to_string(),
                        signature: sig,
                        docstring: doc,
                        parent: parent_name.map(String::from),
                        code_snippet: snippet,
                        imports: vec![],
                        calls,
                    });
                }
            }
            "interface_declaration" | "type_alias_declaration" => {
                let name = child_text_by_kind(child, "type_identifier", source)
                    .or_else(|| child_text_by_kind(child, "identifier", source));
                if let Some(name) = name {
                    let (start, end) = (
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                    );
                    symbols.push(Symbol {
                        name,
                        symbol_type: SymbolType::Class,
                        filepath: filepath.to_string(),
                        line_start: start,
                        line_end: end,
                        language: language.to_string(),
                        signature: line_at(source, child.start_position().row),
                        docstring: extract_jsdoc_above(source, child.start_position().row),
                        parent: None,
                        code_snippet: String::new(),
                        imports: vec![],
                        calls: vec![],
                    });
                }
            }
            "enum_declaration" => {
                if let Some(name) = child_text_by_kind(child, "identifier", source) {
                    let (start, end) = (
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                    );
                    symbols.push(Symbol {
                        name,
                        symbol_type: SymbolType::Class,
                        filepath: filepath.to_string(),
                        line_start: start,
                        line_end: end,
                        language: language.to_string(),
                        signature: line_at(source, child.start_position().row),
                        docstring: String::new(),
                        parent: None,
                        code_snippet: String::new(),
                        imports: vec![],
                        calls: vec![],
                    });
                }
            }
            _ => {}
        }
    }
}

fn extract_ts_function(
    node: tree_sitter::Node,
    source: &[u8],
    filepath: &str,
    language: &str,
    parent: Option<&str>,
) -> Option<Symbol> {
    let name = child_text_by_kind(node, "identifier", source)?;
    let calls = collect_calls(node, source);
    let (start, end) = (
        node.start_position().row as u32 + 1,
        node.end_position().row as u32 + 1,
    );
    let sig = line_at(source, node.start_position().row);
    let doc = extract_jsdoc_above(source, node.start_position().row);
    let snippet = snippet_from(source, node.start_position().row, node.end_position().row);

    Some(Symbol {
        name,
        symbol_type: SymbolType::Function,
        filepath: filepath.to_string(),
        line_start: start,
        line_end: end,
        language: language.to_string(),
        signature: sig,
        docstring: doc,
        parent: parent.map(String::from),
        code_snippet: snippet,
        imports: vec![],
        calls,
    })
}

fn extract_ts_variable_decls(
    node: tree_sitter::Node,
    source: &[u8],
    filepath: &str,
    language: &str,
    parent: Option<&str>,
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = child_text_by_kind(child, "identifier", source);
            // Check if the value is an arrow_function or function_expression
            let has_fn = child
                .named_children(&mut child.walk())
                .any(|c| c.kind() == "arrow_function" || c.kind() == "function");
            if let Some(name) = name {
                if has_fn {
                    let calls = collect_calls(child, source);
                    let (start, end) = (
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                    );
                    let sig = line_at(source, child.start_position().row);
                    let doc = extract_jsdoc_above(source, node.start_position().row);
                    let snippet =
                        snippet_from(source, child.start_position().row, child.end_position().row);

                    symbols.push(Symbol {
                        name,
                        symbol_type: SymbolType::Function,
                        filepath: filepath.to_string(),
                        line_start: start,
                        line_end: end,
                        language: language.to_string(),
                        signature: sig,
                        docstring: doc,
                        parent: parent.map(String::from),
                        code_snippet: snippet,
                        imports: vec![],
                        calls,
                    });
                }
            }
        }
    }
}

// ─── Python (real tree-sitter) ────────────────────────────────────────────────

fn parse_python(content: &str, filepath: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return parse_heuristic(content, filepath, "python");
    }
    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return parse_heuristic(content, filepath, "python"),
    };
    let source = content.as_bytes();
    let mut symbols = Vec::new();
    extract_py_symbols(tree.root_node(), source, filepath, None, &mut symbols);
    symbols
}

fn extract_py_symbols(
    node: tree_sitter::Node,
    source: &[u8],
    filepath: &str,
    parent: Option<&str>,
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source))
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let st = if parent.is_some() {
                    SymbolType::Method
                } else {
                    SymbolType::Function
                };
                let doc = py_docstring(child, source);
                let calls = py_calls(child, source);
                let (start, end) = (
                    child.start_position().row as u32 + 1,
                    child.end_position().row as u32 + 1,
                );
                symbols.push(Symbol {
                    name,
                    symbol_type: st,
                    filepath: filepath.to_string(),
                    line_start: start,
                    line_end: end,
                    language: "python".to_string(),
                    signature: line_at(source, child.start_position().row),
                    docstring: doc,
                    parent: parent.map(String::from),
                    code_snippet: snippet_from(
                        source,
                        child.start_position().row,
                        child.end_position().row,
                    ),
                    imports: vec![],
                    calls,
                });
            }
            "class_definition" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source))
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let doc = py_docstring(child, source);
                let (start, end) = (
                    child.start_position().row as u32 + 1,
                    child.end_position().row as u32 + 1,
                );
                symbols.push(Symbol {
                    name: name.clone(),
                    symbol_type: SymbolType::Class,
                    filepath: filepath.to_string(),
                    line_start: start,
                    line_end: end,
                    language: "python".to_string(),
                    signature: line_at(source, child.start_position().row),
                    docstring: doc,
                    parent: parent.map(String::from),
                    code_snippet: snippet_from(
                        source,
                        child.start_position().row,
                        child.end_position().row,
                    ),
                    imports: vec![],
                    calls: vec![],
                });
                if let Some(body) = child.child_by_field_name("body") {
                    extract_py_symbols(body, source, filepath, Some(&name), symbols);
                }
            }
            "decorated_definition" => {
                extract_py_symbols(child, source, filepath, parent, symbols);
            }
            _ => {}
        }
    }
}

fn py_docstring(node: tree_sitter::Node, source: &[u8]) -> String {
    let body = match node.child_by_field_name("body") {
        Some(b) => b,
        None => return String::new(),
    };
    // Docstring is the first expression_statement containing a string literal
    let first = body.named_child(0);
    if let Some(child) = first {
        if child.kind() == "expression_statement" {
            if let Some(expr) = child.named_child(0) {
                if expr.kind() == "string" || expr.kind() == "concatenated_string" {
                    let text = node_text(expr, source);
                    return text.trim_matches('"').trim_matches('\'').trim().to_string();
                }
            }
        }
    }
    String::new()
}

fn py_calls(node: tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut calls = Vec::new();
    py_calls_walk(node, source, &mut calls);
    calls
}

fn py_calls_walk(node: tree_sitter::Node, source: &[u8], calls: &mut Vec<String>) {
    if node.kind() == "call" {
        if let Some(func) = node.child_by_field_name("function") {
            let name = match func.kind() {
                "identifier" => node_text(func, source),
                "attribute" => func
                    .child_by_field_name("attribute")
                    .map(|a| node_text(a, source))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            if !name.is_empty() && !calls.contains(&name) && !is_keyword(&name) {
                calls.push(name);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        py_calls_walk(child, source, calls);
    }
}

// ─── Go (real tree-sitter) ────────────────────────────────────────────────────

fn parse_go(content: &str, filepath: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .is_err()
    {
        return parse_heuristic(content, filepath, "go");
    }
    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return parse_heuristic(content, filepath, "go"),
    };
    let source = content.as_bytes();
    let mut symbols = Vec::new();
    let mut cursor = tree.root_node().walk();
    for child in tree.root_node().named_children(&mut cursor) {
        match child.kind() {
            "function_declaration" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source))
                    .unwrap_or_default();
                if !name.is_empty() {
                    let calls = collect_calls(child, source);
                    let (start, end) = (
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                    );
                    symbols.push(Symbol {
                        name,
                        symbol_type: SymbolType::Function,
                        filepath: filepath.to_string(),
                        line_start: start,
                        line_end: end,
                        language: "go".to_string(),
                        signature: line_at(source, child.start_position().row),
                        docstring: extract_go_doc(source, child.start_position().row),
                        parent: None,
                        code_snippet: snippet_from(
                            source,
                            child.start_position().row,
                            child.end_position().row,
                        ),
                        imports: vec![],
                        calls,
                    });
                }
            }
            "method_declaration" => {
                let name =
                    child_text_by_kind(child, "field_identifier", source).unwrap_or_default();
                if !name.is_empty() {
                    let calls = collect_calls(child, source);
                    let (start, end) = (
                        child.start_position().row as u32 + 1,
                        child.end_position().row as u32 + 1,
                    );
                    // Extract receiver type as parent
                    let parent = child
                        .child_by_field_name("receiver")
                        .and_then(|r| child_text_by_kind(r, "type_identifier", source));
                    symbols.push(Symbol {
                        name,
                        symbol_type: SymbolType::Method,
                        filepath: filepath.to_string(),
                        line_start: start,
                        line_end: end,
                        language: "go".to_string(),
                        signature: line_at(source, child.start_position().row),
                        docstring: extract_go_doc(source, child.start_position().row),
                        parent,
                        code_snippet: snippet_from(
                            source,
                            child.start_position().row,
                            child.end_position().row,
                        ),
                        imports: vec![],
                        calls,
                    });
                }
            }
            "type_declaration" => {
                // type Foo struct { ... } or type Bar interface { ... }
                let mut tc = child.walk();
                for spec in child.named_children(&mut tc) {
                    if spec.kind() == "type_spec" {
                        let name =
                            child_text_by_kind(spec, "type_identifier", source).unwrap_or_default();
                        if !name.is_empty() {
                            let (start, end) = (
                                spec.start_position().row as u32 + 1,
                                spec.end_position().row as u32 + 1,
                            );
                            symbols.push(Symbol {
                                name,
                                symbol_type: SymbolType::Class,
                                filepath: filepath.to_string(),
                                line_start: start,
                                line_end: end,
                                language: "go".to_string(),
                                signature: line_at(source, spec.start_position().row),
                                docstring: extract_go_doc(source, child.start_position().row),
                                parent: None,
                                code_snippet: snippet_from(
                                    source,
                                    spec.start_position().row,
                                    spec.end_position().row,
                                ),
                                imports: vec![],
                                calls: vec![],
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    symbols
}

fn extract_go_doc(source: &[u8], row: usize) -> String {
    if row == 0 {
        return String::new();
    }
    let s = std::str::from_utf8(source).unwrap_or("");
    let lines: Vec<&str> = s.lines().collect();
    let mut doc_lines = Vec::new();
    let mut j = row;
    while j > 0 {
        j -= 1;
        let l = lines.get(j).unwrap_or(&"").trim();
        if l.starts_with("//") {
            doc_lines.push(l.trim_start_matches('/').trim());
        } else {
            break;
        }
    }
    doc_lines.reverse();
    doc_lines.join(" ")
}

// ─── Java (real tree-sitter) ─────────────────────────────────────────────────

fn parse_java(content: &str, filepath: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .is_err()
    {
        return parse_heuristic(content, filepath, "java");
    }
    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return parse_heuristic(content, filepath, "java"),
    };
    let source = content.as_bytes();
    let mut symbols = Vec::new();
    extract_java_symbols(tree.root_node(), source, filepath, None, &mut symbols);
    symbols
}

fn extract_java_symbols(
    node: tree_sitter::Node,
    source: &[u8],
    filepath: &str,
    parent: Option<&str>,
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "class_declaration" | "interface_declaration" | "enum_declaration" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source))
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let (start, end) = (
                    child.start_position().row as u32 + 1,
                    child.end_position().row as u32 + 1,
                );
                symbols.push(Symbol {
                    name: name.clone(),
                    symbol_type: SymbolType::Class,
                    filepath: filepath.to_string(),
                    line_start: start,
                    line_end: end,
                    language: "java".to_string(),
                    signature: line_at(source, child.start_position().row),
                    docstring: extract_jsdoc_above(source, child.start_position().row),
                    parent: parent.map(String::from),
                    code_snippet: snippet_from(
                        source,
                        child.start_position().row,
                        child.end_position().row,
                    ),
                    imports: vec![],
                    calls: vec![],
                });
                // Recurse into class body
                if let Some(body) = child.child_by_field_name("body") {
                    extract_java_symbols(body, source, filepath, Some(&name), symbols);
                }
            }
            "method_declaration" | "constructor_declaration" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source))
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let calls = collect_calls(child, source);
                let (start, end) = (
                    child.start_position().row as u32 + 1,
                    child.end_position().row as u32 + 1,
                );
                symbols.push(Symbol {
                    name,
                    symbol_type: SymbolType::Method,
                    filepath: filepath.to_string(),
                    line_start: start,
                    line_end: end,
                    language: "java".to_string(),
                    signature: line_at(source, child.start_position().row),
                    docstring: extract_jsdoc_above(source, child.start_position().row),
                    parent: parent.map(String::from),
                    code_snippet: snippet_from(
                        source,
                        child.start_position().row,
                        child.end_position().row,
                    ),
                    imports: vec![],
                    calls,
                });
            }
            _ => {}
        }
    }
}

// ─── Rust (real tree-sitter) ─────────────────────────────────────────────────

fn parse_rust(content: &str, filepath: &str) -> Vec<Symbol> {
    // tree-sitter-rust 0.24 requires ABI 15 which is incompatible with tree-sitter 0.24.7.
    // Use the heuristic parser which already handles impl blocks, methods, calls, and docstrings.
    parse_rust_heuristic(content, filepath)
}

fn parse_rust_heuristic(content: &str, filepath: &str) -> Vec<Symbol> {
    let lines: Vec<&str> = content.lines().collect();
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
                        docstring: extract_rust_doc(content.as_bytes(), i),
                        parent: current_impl.clone(),
                        code_snippet: snippet_from(content.as_bytes(), i, end_line),
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
                        docstring: extract_rust_doc(content.as_bytes(), i),
                        parent: None,
                        code_snippet: snippet_from(content.as_bytes(), i, end_line),
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

fn extract_rust_impl_name(line: &str) -> Option<String> {
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

fn find_block_end_braces(lines: &[&str], start: usize) -> usize {
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

fn apply_brace_depth_delta(lines: &[&str], start: usize, end: usize, depth: &mut i32) {
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

/// Recursively collect all call_expression and JSX component usages from a subtree.
fn collect_calls(node: tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut calls = Vec::new();
    collect_calls_recursive(node, source, &mut calls);
    calls
}

fn collect_calls_recursive(node: tree_sitter::Node, source: &[u8], calls: &mut Vec<String>) {
    match node.kind() {
        "call_expression" => {
            // First named child is the function being called
            if let Some(func) = node.named_child(0) {
                let name = match func.kind() {
                    "identifier" => node_text(func, source),
                    "member_expression" => {
                        // foo.bar() → extract "bar"
                        if let Some(prop) = func.child_by_field_name("property") {
                            node_text(prop, source)
                        } else {
                            node_text(func, source)
                        }
                    }
                    _ => node_text(func, source),
                };
                if !name.is_empty() && !calls.contains(&name) && !is_keyword(&name) {
                    calls.push(name);
                }
            }
        }
        "jsx_self_closing_element" | "jsx_opening_element" => {
            // <ComponentName ... /> → extract component name (uppercase = component)
            if let Some(name_node) = node.named_child(0) {
                let name = node_text(name_node, source);
                if !name.is_empty()
                    && name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    && !calls.contains(&name)
                {
                    calls.push(name);
                }
            }
        }
        "jsx_attribute" => {
            // onClick={handleClick} → extract "handleClick" as a call edge
            // The value is a jsx_expression containing an identifier
            if let Some(value) = node.child_by_field_name("value") {
                // jsx_expression: { identifier } or { obj.method }
                let mut vc = value.walk();
                for child in value.named_children(&mut vc) {
                    if child.kind() == "identifier" {
                        let name = node_text(child, source);
                        if !name.is_empty()
                            && name.len() > 1
                            && !calls.contains(&name)
                            && !is_keyword(&name)
                            && name
                                .chars()
                                .next()
                                .map(|c| c.is_alphabetic())
                                .unwrap_or(false)
                        {
                            calls.push(name);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls_recursive(child, source, calls);
    }
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "if" | "for"
            | "while"
            | "return"
            | "switch"
            | "case"
            | "else"
            | "new"
            | "typeof"
            | "instanceof"
            | "delete"
            | "void"
            | "throw"
            | "catch"
            | "try"
            | "finally"
            | "yield"
            | "await"
            | "match"
            | "let"
            | "const"
            | "var"
            | "fn"
            | "use"
            | "mod"
            | "impl"
            | "struct"
            | "enum"
            | "trait"
            | "type"
            | "where"
            | "as"
            | "super"
            | "require"
            | "import"
            | "export"
    )
}

fn child_text_by_kind(node: tree_sitter::Node, kind: &str, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == kind {
            let text = node_text(child, source);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()])
        .unwrap_or("")
        .to_string()
}

fn line_at(source: &[u8], row: usize) -> String {
    let s = std::str::from_utf8(source).unwrap_or("");
    s.lines().nth(row).unwrap_or("").to_string()
}

fn snippet_from(source: &[u8], start_row: usize, end_row: usize) -> String {
    let s = std::str::from_utf8(source).unwrap_or("");
    let lines: Vec<&str> = s.lines().collect();
    let end = (end_row + 1).min(lines.len()).min(start_row + 50);
    lines[start_row..end].join("\n")
}

fn extract_jsdoc_above(source: &[u8], row: usize) -> String {
    if row == 0 {
        return String::new();
    }
    let s = std::str::from_utf8(source).unwrap_or("");
    let lines: Vec<&str> = s.lines().collect();
    let prev = lines.get(row.wrapping_sub(1)).unwrap_or(&"").trim();
    if prev.ends_with("*/") {
        let mut doc_lines = Vec::new();
        let mut j = row - 1;
        loop {
            let l = lines.get(j).unwrap_or(&"").trim();
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
        return doc_lines
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    if prev.starts_with("//") {
        return prev.trim_start_matches('/').trim().to_string();
    }
    String::new()
}

fn extract_rust_doc(source: &[u8], row: usize) -> String {
    if row == 0 {
        return String::new();
    }
    let s = std::str::from_utf8(source).unwrap_or("");
    let lines: Vec<&str> = s.lines().collect();
    let mut doc_lines = Vec::new();
    let mut j = row;
    while j > 0 {
        j -= 1;
        let l = lines.get(j).unwrap_or(&"").trim();
        if l.starts_with("///") {
            doc_lines.push(l.trim_start_matches('/').trim());
        } else if l.starts_with("#[") || l.is_empty() {
            // skip attributes and blank lines
        } else {
            break;
        }
    }
    doc_lines.reverse();
    doc_lines.join(" ")
}

// ─── Heuristic fallback (for Python, Go, etc.) ──────────────────────────────

fn parse_heuristic(content: &str, filepath: &str, language: &str) -> Vec<Symbol> {
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

fn parse_generic_def(line: &str, filepath: &str, language: &str, line_num: u32) -> Option<Symbol> {
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

fn collect_calls_heuristic(lines: &[&str], start: usize, end: usize) -> Vec<String> {
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

fn find_block_end_python(lines: &[&str], start: usize) -> usize {
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

fn extract_imports(content: &str, language: &str) -> Vec<String> {
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ts_arrow_functions() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_ts_real.tsx");
        std::fs::write(
            &tmp,
            r#"
export const fetchUser = async (id: string) => {
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

interface UserProps { id: string; }
type UserId = string;
enum Status { Active, Inactive }

const App = () => {
    return <UserService />;
};
"#,
        )
        .unwrap();

        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"fetchUser"),
            "Missing arrow fn: {:?}",
            names
        );
        assert!(
            names.contains(&"processData"),
            "Missing function: {:?}",
            names
        );
        assert!(names.contains(&"UserService"), "Missing class: {:?}", names);
        assert!(names.contains(&"getUser"), "Missing method: {:?}", names);
        assert!(
            names.contains(&"UserProps"),
            "Missing interface: {:?}",
            names
        );
        assert!(names.contains(&"UserId"), "Missing type alias: {:?}", names);
        assert!(names.contains(&"Status"), "Missing enum: {:?}", names);
        assert!(
            names.contains(&"App"),
            "Missing arrow component: {:?}",
            names
        );

        // Check calls
        let fetch = result
            .symbols
            .iter()
            .find(|s| s.name == "fetchUser")
            .unwrap();
        assert!(
            fetch.calls.contains(&"transform".to_string()),
            "fetchUser calls: {:?}",
            fetch.calls
        );

        // Check JSX call detection
        let app = result.symbols.iter().find(|s| s.name == "App").unwrap();
        assert!(
            app.calls.contains(&"UserService".to_string()),
            "App JSX calls: {:?}",
            app.calls
        );

        // Check method parent
        let get_user = result.symbols.iter().find(|s| s.name == "getUser").unwrap();
        assert_eq!(get_user.parent, Some("UserService".to_string()));

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_rust_real_parser() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_rs_real.rs");
        std::fs::write(
            &tmp,
            r#"
/// A user store.
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

enum Color { Red, Blue }
trait Drawable { fn draw(&self); }
"#,
        )
        .unwrap();

        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"UserStore"), "Missing struct: {:?}", names);
        assert!(names.contains(&"new"), "Missing method: {:?}", names);
        assert!(names.contains(&"get"), "Missing method: {:?}", names);
        assert!(names.contains(&"standalone"), "Missing fn: {:?}", names);
        assert!(names.contains(&"Color"), "Missing enum: {:?}", names);
        assert!(names.contains(&"Drawable"), "Missing trait: {:?}", names);

        let new_sym = result.symbols.iter().find(|s| s.name == "new").unwrap();
        assert!(
            new_sym.calls.contains(&"validate".to_string()),
            "new calls: {:?}",
            new_sym.calls
        );
        assert_eq!(new_sym.parent, Some("UserStore".to_string()));

        let get_sym = result.symbols.iter().find(|s| s.name == "get").unwrap();
        assert!(
            get_sym.calls.contains(&"deserialize".to_string()),
            "get calls: {:?}",
            get_sym.calls
        );

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_python_fallback() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_py_fallback.py");
        std::fs::write(&tmp, "def hello():\n    \"\"\"Say hello.\"\"\"\n    print(\"hello\")\n\nclass Foo:\n    pass\n").unwrap();
        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"Foo"));
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_rust_heuristic_handles_multiline_function_signatures() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_rust_multiline_signature.rs");
        std::fs::write(
            &tmp,
            r#"
fn execute_search() {}
fn vector_search() {}

pub fn handle_search(
    query: &str,
    limit: usize,
) -> usize {
    let results = execute_search();
    if query.is_empty() {
        return limit;
    }
    vector_search();
    results.len()
}
"#,
        )
        .unwrap();
        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        let handle_search = result
            .symbols
            .iter()
            .find(|symbol| symbol.name == "handle_search")
            .unwrap();

        assert!(handle_search.line_end > handle_search.line_start);
        assert!(
            handle_search.calls.contains(&"execute_search".to_string()),
            "calls: {:?}",
            handle_search.calls
        );
        assert!(
            handle_search.calls.contains(&"vector_search".to_string()),
            "calls: {:?}",
            handle_search.calls
        );

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_rust_heuristic_resets_impl_scope_after_skipped_method_bodies() {
        let parser = TreeSitterParser::new();
        let tmp = std::env::temp_dir().join("test_rust_impl_scope_reset.rs");
        std::fs::write(
            &tmp,
            r#"
struct SearchOptions;

impl SearchOptions {
    fn helper(&self) {
        nested();
    }
}

fn nested() {}

pub fn execute_search() {
    nested();
}
"#,
        )
        .unwrap();
        let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
        let names: Vec<&str> = result
            .symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect();
        let helper = result
            .symbols
            .iter()
            .find(|symbol| symbol.name == "helper")
            .unwrap_or_else(|| panic!("missing helper symbol in {:?}", names));
        let execute_search = result
            .symbols
            .iter()
            .find(|symbol| symbol.name == "execute_search")
            .unwrap_or_else(|| panic!("missing execute_search symbol in {:?}", names));

        assert_eq!(helper.parent.as_deref(), Some("SearchOptions"));
        assert_eq!(execute_search.parent, None);

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_can_parse() {
        let parser = TreeSitterParser::new();
        assert!(parser.can_parse("main.py"));
        assert!(parser.can_parse("app.ts"));
        assert!(parser.can_parse("component.tsx"));
        assert!(!parser.can_parse("readme.md"));
    }
}
