use super::shared::*;
use super::*;

pub(super) fn parse_go(content: &str, filepath: &str) -> Vec<Symbol> {
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

pub(super) fn extract_go_doc(source: &[u8], row: usize) -> String {
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
