use super::shared::*;
use super::*;

pub(super) fn parse_ts_js(content: &str, filepath: &str, language: &str) -> Vec<Symbol> {
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

pub(super) fn extract_ts_symbols(
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

pub(super) fn extract_ts_function(
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

pub(super) fn extract_ts_variable_decls(
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
