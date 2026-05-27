use super::*;

#[test]
fn test_circular_dependencies_detects_rust_use_cycles() {
    let dir = temp_dir("circular-rust");
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let alpha = src.join("alpha.rs");
    let beta = src.join("beta.rs");
    std::fs::write(&alpha, "use crate::beta;\n").unwrap();
    std::fs::write(&beta, "use crate::alpha;\n").unwrap();

    let graph = CodeGraph::new();
    for (id, name, file_path) in [
        ("alpha", "alpha", alpha.to_string_lossy().to_string()),
        ("beta", "beta", beta.to_string_lossy().to_string()),
    ] {
        graph.add_node(UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path,
                start_line: 1,
                end_line: 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    }

    let result = handle_circular_dependencies(&graph, Some(dir.to_string_lossy().as_ref()));

    assert_eq!(result["total"], 1);
    let cycle_files = result["circular_dependencies"][0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    assert!(cycle_files.contains(&"src/alpha.rs"));
    assert!(cycle_files.contains(&"src/beta.rs"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_circular_dependencies_detects_relative_typescript_cycles() {
    let dir = temp_dir("circular-ts");
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let alpha = src.join("alpha.ts");
    let beta = src.join("beta.ts");
    std::fs::write(&alpha, "import { beta } from './beta';\n").unwrap();
    std::fs::write(&beta, "import { alpha } from './alpha';\n").unwrap();

    let graph = CodeGraph::new();
    for (id, name, file_path) in [
        ("alpha", "alpha", alpha.to_string_lossy().to_string()),
        ("beta", "beta", beta.to_string_lossy().to_string()),
    ] {
        graph.add_node(UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path,
                start_line: 1,
                end_line: 1,
                start_column: 0,
                end_column: 0,
                language: "typescript".into(),
            },
            language: "typescript".into(),
            ..Default::default()
        });
    }

    let result = handle_circular_dependencies(&graph, Some(dir.to_string_lossy().as_ref()));

    assert_eq!(result["total"], 1);
    let cycle_files = result["circular_dependencies"][0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    assert!(cycle_files.contains(&"src/alpha.ts"));
    assert!(cycle_files.contains(&"src/beta.ts"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_coverage_map_uses_test_symbol_overlap_for_probable_matches() {
    let graph = CodeGraph::new();
    let source = "/tmp/repo/traverse/browser/session.py";
    let test = "/tmp/repo/tests/ci/test_cross_origin_click.py";

    graph.add_node(UniversalNode {
        id: "browser-session".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Class,
        location: UniversalLocation {
            file_path: source.into(),
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
        id: "test-browser-session".into(),
        name: "browser_session".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: test.into(),
            start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let result = handle_test_coverage_map(&graph, Some("/tmp/repo"));
    assert_eq!(result["covered_files"], 1);
    assert_eq!(result["conservative_covered_files"], 0);
    assert_eq!(result["likely_covered_files"], 1);
    assert_eq!(result["likely_covered"][0], "traverse/browser/session.py");
    assert_eq!(result["coverage_range_percent"]["lower_bound"], 0.0);
    assert_eq!(result["coverage_range_percent"]["upper_bound"], 100.0);
    assert!(result["interpretation"]
        .as_str()
        .unwrap_or("")
        .contains("lower bound"));
}
