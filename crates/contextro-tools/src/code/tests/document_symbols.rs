use super::*;

#[test]
fn test_get_document_symbols_omits_signatures_by_default() {
    let dir = temp_dir("default-signature");
    let file = dir.join("main.py");
    std::fs::write(&file, "def hello(name):\n    return name\n").unwrap();

    let result = get_document_symbols(
        &json!({"path": file.to_string_lossy().to_string()}),
        None,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 1);
    assert_eq!(result["columns"], json!(["name", "type", "line"]));
    assert_eq!(result["symbols"][0], json!(["hello", "function", 1]));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_get_document_symbols_truncates_unicode_signatures_when_requested() {
    let dir = temp_dir("unicode-signature");
    let file = dir.join("main.py");
    let signature = format!("def hello({}) -> str:", "─".repeat(80));
    std::fs::write(&file, format!("{signature}\n    return 'ok'\n")).unwrap();

    let result = get_document_symbols(
        &json!({
            "path": file.to_string_lossy().to_string(),
            "include_signature": true
        }),
        None,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 1);
    assert_eq!(
        result["columns"],
        json!(["name", "type", "line", "signature"])
    );
    let rendered = result["symbols"][0][3].as_str().unwrap();
    assert!(rendered.ends_with('…'));
    assert!(rendered.chars().count() <= 58);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_get_document_symbols_uses_shared_columns_for_multiline_symbols() {
    let dir = temp_dir("shared-columns");
    let file = dir.join("main.py");
    std::fs::write(
        &file,
        "class Hello:\n    def first(self):\n        return 1\n\ndef second():\n    return 2\n",
    )
    .unwrap();

    let result = get_document_symbols(
        &json!({"path": file.to_string_lossy().to_string()}),
        None,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(
        result["columns"],
        json!(["name", "type", "line", "end_line"])
    );
    assert_eq!(result["symbols"][0], json!(["Hello", "class", 1, 3]));
    assert_eq!(result["symbols"][1], json!(["first", "method", 2, null]));
    assert_eq!(result["symbols"][2], json!(["second", "function", 5, null]));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_get_document_symbols_truncates_large_default_payloads() {
    let dir = temp_dir("document-symbols-truncated-default");
    let file = dir.join("main.py");
    let mut content = String::new();
    for idx in 0..30 {
        content.push_str(&format!("def fn_{idx}():\n    return {idx}\n\n"));
    }
    std::fs::write(&file, content).unwrap();

    let result = get_document_symbols(
        &json!({"path": file.to_string_lossy().to_string()}),
        None,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 30);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["symbols"].as_array().unwrap().len(), 3);
    assert_eq!(result["symbols"][0], json!(["fn_0", "function", 1]));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_get_document_symbols_limit_override_returns_full_payload() {
    let dir = temp_dir("document-symbols-limit-override");
    let file = dir.join("main.py");
    let mut content = String::new();
    for idx in 0..30 {
        content.push_str(&format!("def fn_{idx}():\n    return {idx}\n\n"));
    }
    std::fs::write(&file, content).unwrap();

    let result = get_document_symbols(
        &json!({"path": file.to_string_lossy().to_string(), "limit": 30}),
        None,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 30);
    assert!(
        result.get("truncated").is_none(),
        "unexpected result: {result}"
    );
    assert_eq!(result["symbols"].as_array().unwrap().len(), 30);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_get_document_symbols_uses_indexed_graph_for_file_queries() {
    let dir = temp_dir("indexed-document-symbols");
    let nested = dir.join("src");
    std::fs::create_dir_all(&nested).unwrap();
    let file = nested.join("main.py");
    std::fs::write(
        &file,
        "class Hello:\n    def first(self):\n        return 1\n\ndef second():\n    return 2\n",
    )
    .unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "hello-class".into(),
        name: "Hello".into(),
        node_type: NodeType::Class,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 1,
            end_line: 3,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "first-method".into(),
        name: "first".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 2,
            end_line: 3,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        parent: Some("Hello".into()),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "second-function".into(),
        name: "second".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 5,
            end_line: 6,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let result = get_document_symbols(
        &json!({"path":"src/main.py"}),
        Some(&graph),
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(
        result["columns"],
        json!(["name", "type", "line", "end_line"])
    );
    assert_eq!(result["symbols"][0], json!(["Hello", "class", 1, 3]));
    assert_eq!(result["symbols"][1], json!(["first", "method", 2, null]));
    assert_eq!(result["symbols"][2], json!(["second", "function", 5, null]));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_get_document_symbols_truncates_large_indexed_graph_payloads() {
    let dir = temp_dir("indexed-document-symbols-truncated");
    let nested = dir.join("src");
    std::fs::create_dir_all(&nested).unwrap();
    let file = nested.join("main.py");
    let mut content = String::new();
    for idx in 0..30 {
        content.push_str(&format!("def fn_{idx}():\n    return {idx}\n\n"));
    }
    std::fs::write(&file, content).unwrap();
    let canonical_file = std::fs::canonicalize(&file).unwrap();

    let graph = CodeGraph::new();
    for idx in 0..30u32 {
        graph.add_node(UniversalNode {
            id: format!("node-{idx}"),
            name: format!("fn_{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: canonical_file.to_string_lossy().to_string(),
                start_line: idx + 1,
                end_line: idx + 1,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
    }

    let result = get_document_symbols(
        &json!({"path":"src/main.py"}),
        Some(&graph),
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 30);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["symbols"].as_array().unwrap().len(), 3);
    assert_eq!(result["symbols"][0], json!(["fn_0", "function", 1]));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_get_document_symbols_falls_back_to_parser_when_signatures_requested() {
    let dir = temp_dir("document-symbols-signature-fallback");
    let file = dir.join("main.py");
    std::fs::write(&file, "def hello(name):\n    return name\n").unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "hello".into(),
        name: "hello".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 1,
            end_line: 2,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let result = get_document_symbols(
        &json!({
            "path": file.to_string_lossy().to_string(),
            "include_signature": true
        }),
        Some(&graph),
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(
        result["columns"],
        json!(["name", "type", "line", "signature"])
    );
    assert_eq!(result["symbols"][0][3], json!("def hello(name):"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_list_symbols_uses_columnar_file_contract() {
    let dir = temp_dir("list-symbols-file-contract");
    let file = dir.join("main.py");
    std::fs::write(&file, "def hello(name):\n    return name\n").unwrap();

    let result = handle_code(
        &json!({"operation":"list_symbols","path": file.to_string_lossy().to_string()}),
        &CodeGraph::new(),
        None,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["columns"], json!(["name", "type", "line"]));
    assert_eq!(result["symbols"][0], json!(["hello", "function", 1]));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_list_symbols_directory_contract_remains_object_based() {
    let dir = temp_dir("list-symbols-dir-contract");
    let file = dir.join("module.py");
    std::fs::write(&file, "def hello(name):\n    return name\n").unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "hello".into(),
        name: "hello".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 1,
            end_line: 2,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let result = handle_code(
        &json!({"operation":"list_symbols","path": dir.to_string_lossy().to_string()}),
        &graph,
        None,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert!(
        result.get("columns").is_none(),
        "unexpected result: {result}"
    );
    assert_eq!(result["symbols"][0]["name"], json!("hello"));
    assert_eq!(result["symbols"][0]["file"], json!("module.py"));
    assert!(result["symbols"][0].get("callers").is_some());
    assert!(result["symbols"][0].get("callees").is_some());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_handle_code_caches_relative_document_symbols() {
    let dir = temp_dir("cached-relative-document-symbols");
    let file = dir.join("notes.txt");
    std::fs::write(&file, "hello\n").unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "hello".into(),
        name: "hello".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "notes.txt".into(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
            language: "text".into(),
        },
        language: "text".into(),
        ..Default::default()
    });

    let cache = contextro_engines::cache::QueryCache::new(8, 60.0);
    let args = json!({"operation":"get_document_symbols","path":"notes.txt"});
    let first = handle_code(
        &args,
        &graph,
        Some(&cache),
        Some(dir.to_string_lossy().as_ref()),
    );
    assert_eq!(cache.size(), 1);

    let second = handle_code(
        &args,
        &CodeGraph::new(),
        Some(&cache),
        Some(dir.to_string_lossy().as_ref()),
    );
    assert_eq!(second, first);

    let _ = std::fs::remove_dir_all(dir);
}
