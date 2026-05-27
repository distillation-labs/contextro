use super::*;

#[test]
fn test_knowledge_list_alias_returns_indexed_bases() {
    let store_path = temp_file("list");
    let knowledge = KnowledgeStore::with_path(&store_path);
    assert_eq!(knowledge.add("docs", "chunk one\nchunk two", None), 1);

    let result = handle_knowledge(&json!({"command":"list"}), &knowledge);
    assert_eq!(result["total"], 1);
    assert_eq!(result["knowledge_bases"][0]["name"], "docs");
    assert_eq!(result["knowledge_bases"][0]["chunks"], 1);

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_add_indexes_nested_directory_contents() {
    let root = temp_dir("nested");
    let nested = root.join("docs/guides");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        nested.join("manual.md"),
        "Nested manual token: unique_nested_knowledge_token",
    )
    .unwrap();

    let store_path = temp_file("nested");
    let knowledge = KnowledgeStore::with_path(&store_path);
    knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
    let add_result = handle_knowledge(
        &json!({"command":"add","name":"nested-docs","value": root.to_string_lossy()}),
        &knowledge,
    );
    assert_eq!(add_result["status"], "indexed");
    assert_eq!(add_result["overwritten"], false);

    let search_result = handle_knowledge(
        &json!({"command":"search","query":"unique_nested_knowledge_token","limit":5}),
        &knowledge,
    );
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "nested-docs");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_add_documents_batches_multiple_sources() {
    let store_path = temp_file("batch");
    let knowledge = KnowledgeStore::with_path(&store_path);

    let count = knowledge.add_documents([
        (
            "README.md".to_string(),
            "alpha unique_batch_alpha".to_string(),
            Some(PathBuf::from("README.md")),
        ),
        (
            "AGENTS.md".to_string(),
            "beta unique_batch_beta".to_string(),
            Some(PathBuf::from("AGENTS.md")),
        ),
    ]);

    assert_eq!(count, 2);
    let results = knowledge.search("unique_batch_beta", 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "AGENTS.md");

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_add_inline_text_succeeds() {
    let store_path = temp_file("inline-text");
    let knowledge = KnowledgeStore::with_path(&store_path);

    let add_result = handle_knowledge(
        &json!({"command":"add","name":"inline","value":"developer trust release checklist"}),
        &knowledge,
    );
    let search_result = handle_knowledge(
        &json!({"command":"search","query":"release checklist","limit":5}),
        &knowledge,
    );

    assert_eq!(add_result["status"], "indexed");
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "inline");

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_add_path_like_inline_text_succeeds() {
    let store_path = temp_file("inline-path-like");
    let knowledge = KnowledgeStore::with_path(&store_path);

    let inline_value = "See docs/api/v1 and ../notes/todo before release";
    let add_result = handle_knowledge(
        &json!({"command":"add","name":"path-like-inline","value": inline_value}),
        &knowledge,
    );
    let search_result = handle_knowledge(
        &json!({"command":"search","query":"docs api v1","limit":5}),
        &knowledge,
    );

    assert_eq!(add_result["status"], "indexed");
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "path-like-inline");

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_add_existing_file_path_reads_from_disk() {
    let root = temp_dir("existing-file-path");
    let note = root.join("guide.md");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&note, "disk-backed knowledge token").unwrap();

    let store_path = temp_file("existing-file-path");
    let knowledge = KnowledgeStore::with_path(&store_path);
    let add_result = handle_knowledge(
        &json!({"command":"add","name":"guide","value": note.to_string_lossy()}),
        &knowledge,
    );
    let show_result = handle_knowledge(&json!({"command":"show"}), &knowledge);
    let search_result = handle_knowledge(
        &json!({"command":"search","query":"disk-backed knowledge token","limit":5}),
        &knowledge,
    );

    assert_eq!(add_result["status"], "indexed");
    assert!(show_result["knowledge_bases"][0]["source_path"]
        .as_str()
        .unwrap()
        .ends_with("guide.md"));
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "guide");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_search_matches_manual_doc_name_for_high_level_queries() {
    let root = temp_dir("roadmap");
    let roadmap = root.join("ROADMAP.md");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        &roadmap,
        "Prioritize developer trust, launch quality, and release automation.",
    )
    .unwrap();

    let store_path = temp_file("roadmap");
    let knowledge = KnowledgeStore::with_path(&store_path);
    knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
    let add_result = handle_knowledge(
        &json!({"command":"add","name":"ROADMAP.md","value": roadmap.to_string_lossy()}),
        &knowledge,
    );
    assert_eq!(add_result["status"], "indexed");

    let search_result = handle_knowledge(
        &json!({"command":"search","query":"roadmap priorities","limit":5}),
        &knowledge,
    );
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "ROADMAP.md");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_search_matches_plural_variants() {
    let root = temp_dir("milestones");
    let roadmap = root.join("ROADMAP.md");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        &roadmap,
        "Roadmap priorities: developer trust and milestone discipline.",
    )
    .unwrap();

    let store_path = temp_file("milestones");
    let knowledge = KnowledgeStore::with_path(&store_path);
    knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
    let add_result = handle_knowledge(
        &json!({"command":"add","name":"ROADMAP.md","value": roadmap.to_string_lossy()}),
        &knowledge,
    );
    assert_eq!(add_result["status"], "indexed");

    let search_result = handle_knowledge(
        &json!({"command":"search","query":"milestones","limit":5}),
        &knowledge,
    );
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "ROADMAP.md");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_show_returns_more_detail_than_list() {
    let root = temp_dir("show");
    let note = root.join("guide.md");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&note, "Guide preview text for knowledge show details.").unwrap();

    let store_path = temp_file("show");
    let knowledge = KnowledgeStore::with_path(&store_path);
    knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
    handle_knowledge(
        &json!({"command":"add","name":"guide","value": note.to_string_lossy()}),
        &knowledge,
    );

    let show_result = handle_knowledge(&json!({"command":"show"}), &knowledge);
    let list_result = handle_knowledge(&json!({"command":"list"}), &knowledge);

    assert_eq!(show_result["knowledge_bases"][0]["name"], "guide");
    assert!(show_result["knowledge_bases"][0]["preview"]
        .as_str()
        .unwrap()
        .contains("Guide preview text"));
    assert!(show_result["knowledge_bases"][0]["source_path"]
        .as_str()
        .unwrap()
        .ends_with("guide.md"));
    assert!(list_result["knowledge_bases"][0].get("preview").is_none());
    assert!(list_result["knowledge_bases"][0]
        .get("source_path")
        .is_none());

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}
