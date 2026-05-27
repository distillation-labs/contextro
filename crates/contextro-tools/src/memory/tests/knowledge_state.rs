use super::*;

#[test]
fn test_knowledge_search_truncates_unicode_safely() {
    let root = temp_dir("unicode");
    let note = root.join("guide.md");
    std::fs::create_dir_all(&root).unwrap();
    let unicode_text = format!("{}match token", "─".repeat(600));
    std::fs::write(&note, &unicode_text).unwrap();

    let store_path = temp_file("unicode");
    let knowledge = KnowledgeStore::with_path(&store_path);
    knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
    handle_knowledge(
        &json!({"command":"add","name":"guide","value": note.to_string_lossy()}),
        &knowledge,
    );

    let search_result = handle_knowledge(
        &json!({"command":"search","query":"match token","limit":5}),
        &knowledge,
    );

    assert_eq!(search_result["total"], 1);
    let content = search_result["results"][0]["content"].as_str().unwrap();
    assert!(content.ends_with("..."));
    assert!(content.chars().count() <= 503);

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_persists_active_scope_across_reloads() {
    let store_path = temp_file("persist");
    let root = temp_dir("persist-root");
    let roadmap = root.join("ROADMAP.md");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        &roadmap,
        "Roadmap priorities: developer trust and milestone discipline.",
    )
    .unwrap();

    let knowledge = KnowledgeStore::with_path(&store_path);
    knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
    let add_result = handle_knowledge(
        &json!({"command":"add","name":"ROADMAP.md","value": roadmap.to_string_lossy()}),
        &knowledge,
    );
    assert_eq!(add_result["status"], "indexed");
    drop(knowledge);

    let reloaded = KnowledgeStore::with_path(&store_path);
    let list_result = handle_knowledge(&json!({"command":"list"}), &reloaded);
    let search_result = handle_knowledge(
        &json!({"command":"search","query":"roadmap priorities","limit":5}),
        &reloaded,
    );

    assert_eq!(list_result["total"], 1);
    assert_eq!(list_result["knowledge_bases"][0]["name"], "ROADMAP.md");
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "ROADMAP.md");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_restart_restores_sole_repo_scope_when_active_scope_missing() {
    let store_path = temp_file("restart-scope-missing");
    let root = temp_dir("restart-scope-root");
    let note = root.join("evaluation.md");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&note, "release blocker persistence note").unwrap();

    let repo_scope = std::fs::canonicalize(&root)
        .unwrap_or_else(|_| root.clone())
        .to_string_lossy()
        .to_string();
    let source_path = std::fs::canonicalize(&note)
        .unwrap_or_else(|_| note.clone())
        .to_string_lossy()
        .to_string();

    let persisted = json!({
        "scopes": {
            "": {
                "README.md": {
                    "chunks": [{"content": "global default docs"}],
                    "metadata_text": "README.md",
                    "source_path": serde_json::Value::Null,
                }
            },
            repo_scope: {
                "qa-evaluation": {
                    "chunks": [{"content": "release blocker persistence note"}],
                    "metadata_text": "qa-evaluation\nevaluation.md",
                    "source_path": source_path,
                }
            }
        }
    });
    std::fs::write(&store_path, serde_json::to_vec_pretty(&persisted).unwrap()).unwrap();

    let reloaded = KnowledgeStore::with_path(&store_path);
    let list_result = handle_knowledge(&json!({"command":"list"}), &reloaded);
    let remove_result = handle_knowledge(
        &json!({"command":"remove", "name":"qa-evaluation"}),
        &reloaded,
    );
    let after_remove = handle_knowledge(&json!({"command":"list"}), &reloaded);

    assert_eq!(list_result["total"], 1);
    assert_eq!(list_result["knowledge_bases"][0]["name"], "qa-evaluation");
    assert_eq!(remove_result["removed"], true);
    assert_eq!(after_remove["total"], 0);

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_restart_does_not_guess_between_multiple_repo_scopes() {
    let store_path = temp_file("restart-ambiguous-scope");
    let persisted = json!({
        "scopes": {
            "": {
                "README.md": {
                    "chunks": [{"content": "global default docs"}],
                    "metadata_text": "README.md",
                    "source_path": serde_json::Value::Null,
                }
            },
            "/tmp/repo-a": {
                "doc-a": {
                    "chunks": [{"content": "repo a doc"}],
                    "metadata_text": "doc-a",
                    "source_path": serde_json::Value::Null,
                }
            },
            "/tmp/repo-b": {
                "doc-b": {
                    "chunks": [{"content": "repo b doc"}],
                    "metadata_text": "doc-b",
                    "source_path": serde_json::Value::Null,
                }
            }
        }
    });
    std::fs::write(&store_path, serde_json::to_vec_pretty(&persisted).unwrap()).unwrap();

    let reloaded = KnowledgeStore::with_path(&store_path);
    let list_result = handle_knowledge(&json!({"command":"list"}), &reloaded);

    assert_eq!(list_result["total"], 1);
    assert_eq!(list_result["knowledge_bases"][0]["name"], "README.md");

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_scopes_isolate_same_named_docs() {
    let store_path = temp_file("scopes");
    let root_a = temp_dir("scope-a");
    let root_b = temp_dir("scope-b");
    std::fs::create_dir_all(&root_a).unwrap();
    std::fs::create_dir_all(&root_b).unwrap();

    let knowledge = KnowledgeStore::with_path(&store_path);
    knowledge.set_active_scope(Some(root_a.to_string_lossy().as_ref()));
    assert_eq!(
        knowledge.add("README.md", "alpha release checklist", None),
        1
    );

    knowledge.set_active_scope(Some(root_b.to_string_lossy().as_ref()));
    assert_eq!(knowledge.add("README.md", "beta browser workflow", None), 1);

    let scope_b = handle_knowledge(
        &json!({"command":"search","query":"browser workflow","limit":5}),
        &knowledge,
    );
    assert_eq!(scope_b["total"], 1);

    knowledge.set_active_scope(Some(root_a.to_string_lossy().as_ref()));
    let scope_a = handle_knowledge(
        &json!({"command":"search","query":"release checklist","limit":5}),
        &knowledge,
    );
    let scope_a_miss = handle_knowledge(
        &json!({"command":"search","query":"browser workflow","limit":5}),
        &knowledge,
    );
    assert_eq!(scope_a["total"], 1);
    assert_eq!(scope_a_miss["total"], 0);

    let _ = std::fs::remove_dir_all(root_a);
    let _ = std::fs::remove_dir_all(root_b);
    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_add_nonexistent_path_like_value_is_treated_as_inline_text() {
    let store_path = temp_file("missing-path-inline");
    let knowledge = KnowledgeStore::with_path(&store_path);
    let value = "/nonexistent/path/fake.md";

    let add_result = handle_knowledge(
        &json!({"command":"add","name":"fake","value": value}),
        &knowledge,
    );
    let search_result = handle_knowledge(
        &json!({"command":"search","query":"fake md","limit":5}),
        &knowledge,
    );

    assert_eq!(add_result["status"], "indexed");
    assert_eq!(search_result["total"], 1);
    assert_eq!(search_result["results"][0]["source"], "fake");

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_update_still_errors_for_nonexistent_path() {
    let store_path = temp_file("update-missing-path");
    let knowledge = KnowledgeStore::with_path(&store_path);

    let result = handle_knowledge(
        &json!({"command":"update","name":"fake","path":"/nonexistent/path/fake.md"}),
        &knowledge,
    );

    assert!(result["error"].as_str().unwrap().contains("Path not found"));

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_add_reports_overwrite_signal() {
    let store_path = temp_file("overwrite");
    let knowledge = KnowledgeStore::with_path(&store_path);

    let first = handle_knowledge(
        &json!({"command":"add","name":"guide","value":"first version"}),
        &knowledge,
    );
    let second = handle_knowledge(
        &json!({"command":"add","name":"guide","value":"second version"}),
        &knowledge,
    );

    assert_eq!(first["overwritten"], false);
    assert_eq!(second["overwritten"], true);

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_clear_removes_active_scope_documents() {
    let store_path = temp_file("clear");
    let knowledge = KnowledgeStore::with_path(&store_path);
    let _ = handle_knowledge(
        &json!({"command":"add","name":"guide","value":"clear me"}),
        &knowledge,
    );

    let result = handle_knowledge(&json!({"command":"clear"}), &knowledge);

    assert_eq!(result["status"], "cleared");
    assert_eq!(result["removed"], 1);
    assert_eq!(
        handle_knowledge(&json!({"command":"list"}), &knowledge)["total"],
        0
    );

    let _ = std::fs::remove_file(store_path);
}

#[test]
fn test_knowledge_search_returns_zero_for_nonsense_query() {
    let store_path = temp_file("nonsense-search");
    let knowledge = KnowledgeStore::with_path(&store_path);
    let _ = handle_knowledge(
        &json!({"command":"add","name":"guide","value":"developer trust and release automation"}),
        &knowledge,
    );

    let result = handle_knowledge(
        &json!({"command":"search","query":"xyznonexistent999","limit":5}),
        &knowledge,
    );

    assert_eq!(result["total"], 0);
    let _ = std::fs::remove_file(store_path);
}
