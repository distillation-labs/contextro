use super::{
    format_response, normalize_repo_dir, resolve_refactor_targets, strip_response_paths,
    take_chars, ContextroServer,
};
use contextro_config::Settings;
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::NodeType;
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_can_skip_reindex_only_for_same_loaded_repo() {
    assert!(ContextroServer::can_skip_reindex(
        "/tmp/repo-a",
        Some("/tmp/repo-a"),
        true,
        true,
        0,
    ));

    assert!(!ContextroServer::can_skip_reindex(
        "/tmp/repo-a",
        Some("/tmp/repo-b"),
        true,
        true,
        0,
    ));

    assert!(!ContextroServer::can_skip_reindex(
        "/tmp/repo-a",
        None,
        true,
        true,
        0,
    ));
}

#[test]
fn test_format_response_truncation_stays_valid_json() {
    let value = json!({
        "symbol": "BrowserSession.close",
        "callers": (0..120)
            .map(|i| format!("caller_{i} (tests/file_{i}.py:{})", i + 1))
            .collect::<Vec<_>>(),
        "total": 120,
    });

    let rendered = format_response(&value, 200);
    let parsed: Value =
        serde_json::from_str(&rendered).expect("truncated output should stay valid JSON");

    assert_eq!(parsed["symbol"], "BrowserSession.close");
    assert_eq!(parsed["total"], 120);
    assert_eq!(parsed["truncated"], true);
    assert!(parsed["callers"].as_array().unwrap().len() < 120);
    assert!(parsed["hint"]
        .as_str()
        .unwrap()
        .contains("Response truncated"));
}

#[test]
fn test_resolve_refactor_targets_supports_qualified_method_names() {
    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "class-browser-session".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Class,
        location: UniversalLocation {
            file_path: "/tmp/repo/session.py".into(),
            start_line: 1,
            end_line: 20,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "method-close".into(),
        name: "close".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "/tmp/repo/session.py".into(),
            start_line: 22,
            end_line: 30,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        parent: Some("BrowserSession".into()),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "caller".into(),
        name: "shutdown".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "/tmp/repo/main.py".into(),
            start_line: 5,
            end_line: 12,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_relationship(UniversalRelationship {
        id: "calls-close".into(),
        source_id: "caller".into(),
        target_id: "method-close".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let matches = resolve_refactor_targets("BrowserSession.close", &graph);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].name, "close");
    assert_eq!(matches[0].parent.as_deref(), Some("BrowserSession"));
}

#[test]
fn test_strip_response_paths_preserves_root_identity_fields() {
    let base = "/tmp/contextro-repo";
    let stripped = strip_response_paths(
        json!({
            "codebase_path": base,
            "path": base,
            "file": format!("{base}/src/lib.rs"),
            "repos": [{"path": base, "name": "repo-a"}],
            "nested": {"file": format!("{base}/src/main.rs")},
        }),
        base,
    );

    assert_eq!(stripped["codebase_path"], base);
    assert_eq!(stripped["path"], base);
    assert_eq!(stripped["repos"][0]["path"], base);
    assert_eq!(stripped["file"], "src/lib.rs");
    assert_eq!(stripped["nested"]["file"], "src/main.rs");
}

#[test]
fn test_take_chars_handles_unicode_boundaries() {
    assert_eq!(take_chars("─alpha", 1), "─");
    assert_eq!(take_chars("hello", 4), "hell");
}

#[test]
fn test_all_tool_definitions_expose_object_input_schema() {
    for tool in ContextroServer::tool_definitions() {
        let schema = tool.schema_as_json_value();
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("object"),
            "tool '{}' must expose an object input schema: {}",
            tool.name,
            schema
        );
    }
}

#[test]
fn test_find_symbol_missing_exact_match_suggests_fuzzy_lookup() {
    let server = ContextroServer::new();
    server
        .state
        .graph
        .add_node(contextro_core::graph::UniversalNode {
            id: "browser-session".into(),
            name: "BrowserSession".into(),
            node_type: contextro_core::NodeType::Class,
            location: contextro_core::graph::UniversalLocation {
                file_path: "/tmp/repo/src/browser/session.py".into(),
                start_line: 1,
                end_line: 20,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
    *server.state.indexed.write() = true;
    let result = server.handle_find_symbol(&json!({"symbol_name":"Browser","exact":true}));

    assert_eq!(result["error"], "Symbol 'Browser' not found.");
    assert!(result["hint"]
        .as_str()
        .unwrap_or("")
        .contains("exact=false"));
    assert!(result["did_you_mean"].is_array());
}

#[test]
fn test_find_symbol_omits_total_for_untruncated_exact_match() {
    let server = ContextroServer::new();
    server
        .state
        .graph
        .add_node(contextro_core::graph::UniversalNode {
            id: "unique-query-cache".into(),
            name: "UniqueQueryCacheTest".into(),
            node_type: contextro_core::NodeType::Class,
            location: contextro_core::graph::UniversalLocation {
                file_path: "/tmp/repo/crates/contextro-engines/src/cache.rs".into(),
                start_line: 10,
                end_line: 80,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    *server.state.indexed.write() = true;

    let result =
        server.handle_find_symbol(&json!({"symbol_name":"UniqueQueryCacheTest","exact":true}));

    assert!(result.get("total").is_none(), "unexpected result: {result}");
    assert!(
        result.get("truncated").is_none(),
        "unexpected result: {result}"
    );
    assert_eq!(result["symbols"][0]["name"], "UniqueQueryCacheTest");
}

fn temp_repo_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("contextro-server-{unique}-{name}"))
}

fn write_indexable_repo(root: &Path, symbol_name: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        format!("pub fn {symbol_name}() {{}}\n"),
    )
    .unwrap();
}

fn temp_storage_dir(name: &str) -> PathBuf {
    temp_repo_dir(&format!("storage-{name}"))
}

fn test_settings(storage_dir: &Path) -> Settings {
    Settings {
        storage_dir: storage_dir.to_string_lossy().to_string(),
        ..Settings::default()
    }
}

fn test_server(storage_dir: &Path) -> ContextroServer {
    fs::create_dir_all(storage_dir).unwrap();
    ContextroServer::with_settings(test_settings(storage_dir))
}

#[path = "main_tests/repo_scope.rs"]
mod repo_scope;
#[path = "main_tests/runtime.rs"]
mod runtime;
