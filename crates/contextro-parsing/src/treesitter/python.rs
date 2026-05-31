use super::shared::*;
use super::*;

pub(super) fn parse_python(content: &str, filepath: &str) -> Vec<Symbol> {
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
    let lines: Vec<&str> = content.lines().collect();
    let mut symbols = Vec::new();
    extract_py_symbols(
        tree.root_node(),
        source,
        &lines,
        filepath,
        None,
        &mut symbols,
    );
    symbols
}

pub(super) fn extract_py_symbols(
    node: tree_sitter::Node,
    source: &[u8],
    lines: &[&str],
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
                    signature: line_from_lines(lines, child.start_position().row),
                    docstring: doc,
                    parent: parent.map(String::from),
                    code_snippet: snippet_from_lines(
                        lines,
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
                    signature: line_from_lines(lines, child.start_position().row),
                    docstring: doc,
                    parent: parent.map(String::from),
                    code_snippet: snippet_from_lines(
                        lines,
                        child.start_position().row,
                        child.end_position().row,
                    ),
                    imports: vec![],
                    calls: vec![],
                });
                if let Some(body) = child.child_by_field_name("body") {
                    extract_py_symbols(body, source, lines, filepath, Some(&name), symbols);
                }
            }
            "decorated_definition" => {
                extract_py_symbols(child, source, lines, filepath, parent, symbols);
            }
            _ => {}
        }
    }
}

pub(super) fn py_docstring(node: tree_sitter::Node, source: &[u8]) -> String {
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

pub(super) fn py_calls(node: tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut calls = Vec::new();
    py_calls_walk(node, source, &mut calls);
    calls
}

pub(super) fn py_calls_walk(node: tree_sitter::Node, source: &[u8], calls: &mut Vec<String>) {
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
    for child in node.named_children(&mut cursor) {
        py_calls_walk(child, source, calls);
    }
}

// ─── Go (real tree-sitter) ────────────────────────────────────────────────────
