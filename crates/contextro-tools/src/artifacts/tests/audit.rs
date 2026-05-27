use super::*;

#[test]
fn test_audit_reports_capped_actionable_complexity_evidence() {
    let graph = CodeGraph::new();

    for idx in 0..4 {
        let hub_id = format!("hub-{idx}");
        let hub_name = format!("HubSymbol{idx}");
        graph.add_node(UniversalNode {
            id: hub_id.clone(),
            name: hub_name,
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: format!("src/hub_{idx}.rs"),
                start_line: 1,
                end_line: 20,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });

        for leaf in 0..11 {
            let leaf_id = format!("leaf-{idx}-{leaf}");
            graph.add_node(UniversalNode {
                id: leaf_id.clone(),
                name: format!("Leaf{idx}_{leaf}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: format!("src/leaf_{idx}_{leaf}.rs"),
                    start_line: 1,
                    end_line: 5,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
            graph.add_relationship(UniversalRelationship {
                id: format!("rel-{idx}-{leaf}"),
                source_id: hub_id.clone(),
                target_id: leaf_id,
                relationship_type: RelationshipType::Calls,
                strength: 1.0,
            });
        }
    }

    graph.add_node(UniversalNode {
        id: "test-hub".into(),
        name: "IgnoredTestHub".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "tests/audit_noise.rs".into(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        ..Default::default()
    });
    for idx in 0..11 {
        let leaf_id = format!("test-leaf-{idx}");
        graph.add_node(UniversalNode {
            id: leaf_id.clone(),
            name: format!("IgnoredLeaf{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: format!("tests/leaf_{idx}.rs"),
                start_line: 1,
                end_line: 5,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
        graph.add_relationship(UniversalRelationship {
            id: format!("test-rel-{idx}"),
            source_id: "test-hub".into(),
            target_id: leaf_id,
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
    }

    let result = handle_audit(&graph, None);
    assert_eq!(result["quality_score"], 80);
    let complexity = result["recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["category"] == "complexity")
        .cloned()
        .expect("complexity recommendation");

    assert_eq!(complexity["threshold"], AUDIT_CONNECTION_THRESHOLD);
    assert_eq!(complexity["affected_count"], 4);
    assert!(complexity["message"]
        .as_str()
        .unwrap_or("")
        .contains("top offenders"));

    let evidence = complexity["evidence"].as_array().expect("evidence array");
    assert_eq!(evidence.len(), AUDIT_EVIDENCE_LIMIT);
    assert!(evidence.iter().all(|item| item["connections"] == 11));
    assert!(evidence.iter().all(|item| {
        item["follow_up"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|step| step.as_str().unwrap_or("").contains("explain"))
    }));
    assert!(evidence
        .iter()
        .all(|item| { !item["file"].as_str().unwrap_or("").starts_with("tests/") }));
}

#[test]
fn test_audit_reports_large_file_evidence_without_test_noise() {
    let graph = CodeGraph::new();

    for idx in 0..31 {
        graph.add_node(UniversalNode {
            id: format!("prod-{idx}"),
            name: format!("ProdSymbol{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "src/big.rs".into(),
                start_line: idx + 1,
                end_line: idx + 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    }

    for idx in 0..40 {
        graph.add_node(UniversalNode {
            id: format!("test-{idx}"),
            name: format!("TestSymbol{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "tests/big_test.rs".into(),
                start_line: idx + 1,
                end_line: idx + 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    }

    let result = handle_audit(&graph, None);
    assert_eq!(result["quality_score"], 80);
    let structure = result["recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["category"] == "structure")
        .cloned()
        .expect("structure recommendation");

    assert_eq!(structure["threshold"], AUDIT_FILE_SYMBOL_THRESHOLD);
    assert_eq!(structure["affected_count"], 1);

    let evidence = structure["evidence"].as_array().expect("evidence array");
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0]["file"], "src/big.rs");
    assert_eq!(evidence[0]["symbols"], 31);
    assert!(evidence[0]["follow_up"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| step.as_str().unwrap_or("").contains("analyze")));
}

#[test]
fn test_audit_quality_score_drops_with_worse_hotspots_even_when_categories_are_unchanged() {
    let baseline_graph = CodeGraph::new();
    baseline_graph.add_node(UniversalNode {
        id: "hub".into(),
        name: "Hub".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "src/core.rs".into(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        ..Default::default()
    });
    for idx in 0..11 {
        let leaf_id = format!("baseline-leaf-{idx}");
        baseline_graph.add_node(UniversalNode {
            id: leaf_id.clone(),
            name: format!("BaselineLeaf{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: format!("src/leaf_{idx}.rs"),
                start_line: 1,
                end_line: 5,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
        baseline_graph.add_relationship(UniversalRelationship {
            id: format!("baseline-rel-{idx}"),
            source_id: "hub".into(),
            target_id: leaf_id,
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
    }
    for idx in 0..31 {
        baseline_graph.add_node(UniversalNode {
            id: format!("baseline-big-{idx}"),
            name: format!("BaselineBig{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "src/big.rs".into(),
                start_line: idx + 1,
                end_line: idx + 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    }

    let baseline = handle_audit(&baseline_graph, None);
    assert_eq!(baseline["quality_score"], 75);

    let worse_graph = CodeGraph::new();
    worse_graph.add_node(UniversalNode {
        id: "hub".into(),
        name: "Hub".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "src/core.rs".into(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        ..Default::default()
    });
    for idx in 0..31 {
        let leaf_id = format!("worse-leaf-{idx}");
        worse_graph.add_node(UniversalNode {
            id: leaf_id.clone(),
            name: format!("WorseLeaf{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: format!("src/worse_leaf_{idx}.rs"),
                start_line: 1,
                end_line: 5,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
        worse_graph.add_relationship(UniversalRelationship {
            id: format!("worse-rel-{idx}"),
            source_id: "hub".into(),
            target_id: leaf_id,
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
    }
    for idx in 0..61 {
        worse_graph.add_node(UniversalNode {
            id: format!("worse-big-{idx}"),
            name: format!("WorseBig{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "src/big.rs".into(),
                start_line: idx + 1,
                end_line: idx + 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    }

    let worse = handle_audit(&worse_graph, None);
    assert_eq!(worse["quality_score"], 72);
    let recommendations = worse["recommendations"].as_array().unwrap();
    assert!(recommendations
        .iter()
        .any(|item| item["category"] == "complexity"));
    assert!(recommendations
        .iter()
        .any(|item| item["category"] == "structure"));
    assert!(recommendations
        .iter()
        .all(|item| item["evidence"].is_array()));
}
