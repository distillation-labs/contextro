use super::*;
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::NodeType;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("contextro-code-{unique}-{name}"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn test_node(
    id: &str,
    name: &str,
    file_path: &str,
    start_line: u32,
    content: &str,
) -> UniversalNode {
    UniversalNode {
        id: id.into(),
        name: name.into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file_path.into(),
            start_line,
            end_line: start_line + 1,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        content: content.into(),
        language: "rust".into(),
        ..Default::default()
    }
}

fn add_call(graph: &CodeGraph, source_id: &str, target_id: &str) {
    graph.add_relationship(UniversalRelationship {
        id: format!("{source_id}->{target_id}"),
        source_id: source_id.into(),
        target_id: target_id.into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
}

#[test]
fn test_get_document_symbols_accepts_path_alias() {
    let dir = temp_dir("symbols");
    let file = dir.join("main.py");
    std::fs::write(&file, "def hello():\n    return 1\n").unwrap();

    let result = get_document_symbols(
        &json!({"path": file.to_string_lossy().to_string()}),
        None,
        Some(dir.to_string_lossy().as_ref()),
    );
    assert!(result.get("total").is_none(), "unexpected result: {result}");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_search_codebase_map_errors_on_missing_path() {
    let graph = CodeGraph::new();
    let result = search_codebase_map(&json!({"path":"missing"}), &graph, None);
    assert!(result.get("error").is_some());
}

#[test]
fn test_search_codebase_map_handles_absolute_path_filter() {
    let dir = temp_dir("map");
    let file = dir.join("lib.rs");
    std::fs::write(&file, "fn alpha() {}\n").unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "alpha".into(),
        name: "alpha".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        ..Default::default()
    });

    let result = search_codebase_map(
        &json!({"path": dir.to_string_lossy().to_string()}),
        &graph,
        Some(dir.to_string_lossy().as_ref()),
    );
    assert_eq!(result["total_files"], 1);
    assert_eq!(result["total_symbols"], 1);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_search_codebase_map_matches_natural_language_queries() {
    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "repo-add".into(),
        name: "handle_repo_add".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "/tmp/contextro/crates/contextro-tools/src/git_tools.rs".into(),
            start_line: 10,
            end_line: 20,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        content: "pub fn handle_repo_add(path: &str) { registry.add(path); }".into(),
        language: "rust".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "repo-registry-add".into(),
        name: "add".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "/tmp/contextro/crates/contextro-tools/src/git_tools.rs".into(),
            start_line: 30,
            end_line: 45,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        content: "pub fn add(&self, path: &str) { self.persist_repos(); }".into(),
        language: "rust".into(),
        parent: Some("RepoRegistry".into()),
        ..Default::default()
    });

    let result = search_codebase_map(
        &json!({"query":"repo add persistence"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["total_files"], 1);
    assert!(result["total_symbols"].as_u64().unwrap_or(0) >= 1);
    let names: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert!(
        names.contains(&"handle_repo_add") || names.contains(&"add"),
        "unexpected matches: {:?}",
        names
    );
}

#[test]
fn test_pattern_search_errors_on_missing_path() {
    let result = pattern_search(
        &json!({"pattern":"truncate_chars","path":"does/not/exist.rs"}),
        None,
    );

    assert_eq!(result["error"], "Path not found: does/not/exist.rs");
}

#[test]
fn test_pattern_rewrite_errors_on_missing_path() {
    let result = pattern_rewrite(
        &json!({
            "pattern":"truncate_chars",
            "replacement":"truncate_text",
            "path":"does/not/exist.rs",
            "dry_run":true
        }),
        None,
    );

    assert_eq!(result["error"], "Path not found: does/not/exist.rs");
}

#[test]
fn test_pattern_search_supports_metavariables() {
    let dir = temp_dir("pattern-search-metavars");
    let file = dir.join("lib.rs");
    std::fs::write(
        &file,
        "fn truncate_chars(text: &str, max_chars: usize) -> String {\n    text.to_string()\n}\n",
    )
    .unwrap();

    let result = pattern_search(
        &json!({
            "pattern":"fn $NAME($$$ARGS)",
            "language":"rust",
            "path": file.to_string_lossy().to_string()
        }),
        None,
    );

    assert_eq!(result["total"], 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_pattern_search_accepts_repo_relative_directory_path() {
    let dir = temp_dir("pattern-search-relative-dir");
    let nested = dir.join("src");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        nested.join("lib.rs"),
        "fn handle_alpha() {}\nfn beta() {}\n",
    )
    .unwrap();

    let result = pattern_search(
        &json!({
            "pattern":"fn handle_",
            "path":"src",
            "language":"rust"
        }),
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 1);
    assert_eq!(result["matches"][0]["file"], "src/lib.rs");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_pattern_rewrite_accepts_repo_relative_file_path() {
    let dir = temp_dir("pattern-rewrite-relative-file");
    let nested = dir.join("src");
    std::fs::create_dir_all(&nested).unwrap();
    let file = nested.join("lib.rs");
    std::fs::write(&file, "fn handle_search() {}\n").unwrap();

    let result = pattern_rewrite(
        &json!({
            "pattern":"handle_search",
            "replacement":"handle_lookup",
            "path":"src/lib.rs",
            "dry_run":true
        }),
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total_files"], 1);
    assert_eq!(result["changes"][0]["file"], "src/lib.rs");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_lookup_symbols_rejects_empty_array_explicitly() {
    let graph = CodeGraph::new();
    let result = lookup_symbols(&json!({"symbols":[]}), &graph, None);

    assert_eq!(
        result["error"],
        "Parameter 'symbols' must contain at least one symbol name."
    );
}

#[test]
fn test_lookup_symbols_omits_type_for_unique_exact_match() {
    let dir = temp_dir("lookup-symbols-omit-type");
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

    let result = lookup_symbols(
        &json!({"symbols":["hello"]}),
        &graph,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["columns"], json!(["name", "file"]));
    assert_eq!(result["symbols"][0], json!(["hello", "module.py:1"]));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_lookup_symbols_keeps_type_for_ambiguous_matches() {
    let dir = temp_dir("lookup-symbols-keep-type-ambiguous");
    let file_a = dir.join("a.py");
    let file_b = dir.join("b.py");
    std::fs::write(&file_a, "def hello(name):\n    return name\n").unwrap();
    std::fs::write(&file_b, "class hello:\n    pass\n").unwrap();

    let graph = CodeGraph::new();
    for (id, node_type, file) in [
        ("hello-fn", NodeType::Function, &file_a),
        ("hello-class", NodeType::Class, &file_b),
    ] {
        graph.add_node(UniversalNode {
            id: id.into(),
            name: "hello".into(),
            node_type,
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
    }

    let result = lookup_symbols(
        &json!({"symbols":["hello"]}),
        &graph,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert!(
        result.get("columns").is_none(),
        "unexpected result: {result}"
    );
    let symbols = result["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 2, "unexpected result: {result}");
    assert!(symbols.iter().all(|entry| entry.get("type").is_some()));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_lookup_symbols_keeps_type_when_including_source() {
    let dir = temp_dir("lookup-symbols-keep-type-source");
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

    let result = lookup_symbols(
        &json!({"symbols":["hello"],"include_source":true}),
        &graph,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert!(
        result.get("columns").is_none(),
        "unexpected result: {result}"
    );
    assert_eq!(result["symbols"][0]["type"], json!("function"));
    assert!(result["symbols"][0]["source"]
        .as_str()
        .unwrap_or("")
        .contains("def hello(name):"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_search_symbols_missing_input_mentions_query_alias() {
    let graph = CodeGraph::new();
    let result = search_symbols(&json!({}), &graph, None);

    assert!(result["hint"].as_str().unwrap_or("").contains("query"));
}

#[path = "tests/codebase_map_core.rs"]
mod codebase_map_core;
#[path = "tests/codebase_map_queries.rs"]
mod codebase_map_queries;
#[path = "tests/document_symbols.rs"]
mod document_symbols;
#[path = "tests/edit_plan_core.rs"]
mod edit_plan_core;
#[path = "tests/edit_plan_regressions.rs"]
mod edit_plan_regressions;
