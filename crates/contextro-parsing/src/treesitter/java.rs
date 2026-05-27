use super::shared::*;
use super::*;

pub(super) fn parse_java(content: &str, filepath: &str) -> Vec<Symbol> {
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

pub(super) fn extract_java_symbols(
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
