use super::*;

#[test]
fn test_forget_accepts_id_alias_from_remember() {
    let db_path = temp_db("forget-id");
    let store = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();

    let remember_result = handle_remember(&json!({"content":"remember forget id alias"}), &store);
    let id = remember_result["id"].as_str().unwrap().to_string();

    let forget_result = handle_forget(&json!({"id": id}), &store);
    assert_eq!(forget_result["deleted"], 1);

    let recall_result = handle_recall(&json!({"query":"remember forget id alias"}), &store);
    assert_eq!(recall_result["total"], 0);

    let _ = fs::remove_file(db_path);
}

#[test]
fn test_remember_rejects_invalid_memory_type() {
    let db_path = temp_db("remember-invalid-type");
    let store = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();

    let result = handle_remember(
        &json!({"content":"invalid type","memory_type":"bogustype"}),
        &store,
    );

    assert!(result["error"]
        .as_str()
        .unwrap()
        .contains("Invalid memory_type"));
    let _ = fs::remove_file(db_path);
}

#[test]
fn test_remember_reports_ttl_and_expiry() {
    let db_path = temp_db("remember-ttl");
    let store = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();

    let result = handle_remember(&json!({"content":"ttl memory","ttl":"day"}), &store);

    assert_eq!(result["ttl"], "day");
    assert!(result["expires_at"].as_str().is_some());
    let _ = fs::remove_file(db_path);
}

#[test]
fn test_recall_supports_empty_query_with_tag_filter() {
    let db_path = temp_db("recall-empty");
    let store = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();

    let _ = handle_remember(
        &json!({"content":"architecture decision","tags":["architecture"]}),
        &store,
    );
    let _ = handle_remember(&json!({"content":"other note","tags":["other"]}), &store);

    let result = handle_recall(&json!({"query":"","tags":["architecture"]}), &store);

    assert_eq!(result["total"], 1);
    assert_eq!(result["memories"][0]["content"], "architecture decision");
    let _ = fs::remove_file(db_path);
}

#[test]
fn test_forget_errors_for_missing_specific_memory_id() {
    let db_path = temp_db("forget-missing");
    let store = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();

    let result = handle_forget(&json!({"memory_id":"mem_missing"}), &store);

    assert!(result["error"].as_str().unwrap().contains("not found"));
    let _ = fs::remove_file(db_path);
}
