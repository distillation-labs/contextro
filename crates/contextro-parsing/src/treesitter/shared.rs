pub(super) fn collect_calls(node: tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut calls = Vec::new();
    collect_calls_recursive(node, source, &mut calls);
    calls
}

pub(super) fn collect_calls_recursive(
    node: tree_sitter::Node,
    source: &[u8],
    calls: &mut Vec<String>,
) {
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

pub(super) fn is_keyword(s: &str) -> bool {
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

pub(super) fn child_text_by_kind(
    node: tree_sitter::Node,
    kind: &str,
    source: &[u8],
) -> Option<String> {
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

pub(super) fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()])
        .unwrap_or("")
        .to_string()
}

pub(super) fn line_at(source: &[u8], row: usize) -> String {
    let s = std::str::from_utf8(source).unwrap_or("");
    s.lines().nth(row).unwrap_or("").to_string()
}

pub(super) fn snippet_from(source: &[u8], start_row: usize, end_row: usize) -> String {
    let s = std::str::from_utf8(source).unwrap_or("");
    let lines: Vec<&str> = s.lines().collect();
    snippet_from_lines(&lines, start_row, end_row)
}

pub(super) fn snippet_from_lines(lines: &[&str], start_row: usize, end_row: usize) -> String {
    if start_row >= lines.len() {
        return String::new();
    }
    let end = (end_row + 1).min(lines.len()).min(start_row + 50);
    lines[start_row..end].join("\n")
}

pub(super) fn extract_jsdoc_above(source: &[u8], row: usize) -> String {
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
