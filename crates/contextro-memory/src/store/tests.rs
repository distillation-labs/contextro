use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn make_memory(content: &str) -> Memory {
    Memory {
        id: String::new(),
        content: content.into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["rust".into()],
        created_at: Utc::now().to_rfc3339(),
        accessed_at: Utc::now().to_rfc3339(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    }
}

fn temp_db(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("contextro-memory-store-{unique}-{name}.db"))
}

#[test]
fn test_remember_recall_forget() {
    let store = MemoryStore::in_memory().unwrap();
    let mem = make_memory("JWT tokens expire after 24h");
    let id = store.remember(&mem).unwrap();
    assert!(id.starts_with("mem_"));

    let results = store.recall("JWT", 10, None, None, None).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].content.contains("JWT"));

    let deleted = store.forget(Some(&id), None, None).unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(store.count(), 0);
}

#[test]
fn test_recall_with_filters() {
    let store = MemoryStore::in_memory().unwrap();
    let mut mem = make_memory("Use Redis for caching");
    mem.memory_type = MemoryType::Decision;
    store.remember(&mem).unwrap();

    let results = store
        .recall("Redis", 10, Some("decision"), None, None)
        .unwrap();
    assert_eq!(results.len(), 1);

    let results = store.recall("Redis", 10, Some("note"), None, None).unwrap();
    assert_eq!(results.len(), 0);
}

#[test]
fn test_recall_prefers_bug_tag_matches_over_old_testing_notes() {
    let store = MemoryStore::in_memory().unwrap();

    let old_testing_note = Memory {
        id: String::new(),
        content: "Scenario testing notes for the release flow and regression checks".into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["testing".into()],
        created_at: "2026-01-01T00:00:00Z".into(),
        accessed_at: "2026-01-01T00:00:00Z".into(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    };
    store.remember(&old_testing_note).unwrap();

    let fresh_bug_note = Memory {
        id: String::new(),
        content: "Checkout submission crashes after the final confirmation step".into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["bug".into(), "scenario".into()],
        created_at: "2026-05-01T00:00:00Z".into(),
        accessed_at: "2026-05-01T00:00:00Z".into(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    };
    store.remember(&fresh_bug_note).unwrap();

    let results = store
        .recall("bugs found during scenario testing", 10, None, None, None)
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].content, fresh_bug_note.content);
    assert_eq!(results[1].content, old_testing_note.content);
}

#[test]
fn test_recall_query_ignores_newer_crowded_distractors_before_rerank() {
    let store = MemoryStore::in_memory().unwrap();

    let relevant_bug = Memory {
        id: String::new(),
        content: "Scenario testing found a checkout bug that duplicates the final order".into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["bug".into(), "scenario".into()],
        created_at: "2026-05-01T00:00:00Z".into(),
        accessed_at: "2026-05-01T00:00:00Z".into(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    };
    store.remember(&relevant_bug).unwrap();

    for i in 0..40 {
        let distractor = Memory {
            id: format!("distractor_{i}"),
            content: format!(
                "Scenario testing notes {i}: generic release checklist review with no bug details"
            ),
            memory_type: MemoryType::Note,
            project: "test".into(),
            tags: vec!["testing".into()],
            created_at: format!("2026-05-{:02}T00:00:00Z", (i % 28) + 2),
            accessed_at: format!("2026-05-{:02}T00:00:00Z", (i % 28) + 2),
            ttl: MemoryTtl::Permanent,
            source: "user".into(),
        };
        store.remember(&distractor).unwrap();
    }

    let results = store
        .recall("bugs found during scenario testing", 5, None, None, None)
        .unwrap();

    assert_eq!(results[0].content, relevant_bug.content);
    assert!(results[1..]
        .iter()
        .all(|memory| memory.content != relevant_bug.content));
}

#[test]
fn test_recall_prefers_all_bug_tagged_memories_over_generic_scenario_testing_distractor() {
    let store = MemoryStore::in_memory().unwrap();

    let bug_memories = [
        Memory {
            id: String::new(),
            content: "Checkout confirmation crashes after the final submit".into(),
            memory_type: MemoryType::Note,
            project: "test".into(),
            tags: vec!["bug".into()],
            created_at: "2026-05-01T00:00:00Z".into(),
            accessed_at: "2026-05-01T00:00:00Z".into(),
            ttl: MemoryTtl::Permanent,
            source: "user".into(),
        },
        Memory {
            id: String::new(),
            content: "Scenario test found duplicate orders when retrying payment".into(),
            memory_type: MemoryType::Note,
            project: "test".into(),
            tags: vec!["bug".into()],
            created_at: "2026-05-02T00:00:00Z".into(),
            accessed_at: "2026-05-02T00:00:00Z".into(),
            ttl: MemoryTtl::Permanent,
            source: "user".into(),
        },
        Memory {
            id: String::new(),
            content: "Receipt email is missing after the tested checkout flow completes".into(),
            memory_type: MemoryType::Note,
            project: "test".into(),
            tags: vec!["bug".into()],
            created_at: "2026-05-03T00:00:00Z".into(),
            accessed_at: "2026-05-03T00:00:00Z".into(),
            ttl: MemoryTtl::Permanent,
            source: "user".into(),
        },
    ];
    for memory in &bug_memories {
        store.remember(memory).unwrap();
    }

    let distractor = Memory {
        id: String::new(),
        content: "Generic scenario testing note for release checklist coverage".into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["scenario".into(), "testing".into()],
        created_at: "2026-05-10T00:00:00Z".into(),
        accessed_at: "2026-05-10T00:00:00Z".into(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    };
    store.remember(&distractor).unwrap();

    let results = store
        .recall("bugs found during scenario testing", 10, None, None, None)
        .unwrap();

    assert_eq!(results.len(), 4);
    assert!(results[..3]
        .iter()
        .all(|memory| memory.tags.iter().any(|tag| tag == "bug")));
    assert_eq!(results[3].content, distractor.content);
}

#[test]
fn test_recall_tag_filter_behavior_is_preserved_with_distractors() {
    let store = MemoryStore::in_memory().unwrap();

    let bug_memory = Memory {
        id: String::new(),
        content: "Scenario testing found a checkout bug in confirmation".into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["bug".into()],
        created_at: "2026-05-01T00:00:00Z".into(),
        accessed_at: "2026-05-01T00:00:00Z".into(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    };
    store.remember(&bug_memory).unwrap();

    for i in 0..8 {
        let distractor = Memory {
            id: format!("non_bug_{i}"),
            content: format!("Generic scenario testing note {i}"),
            memory_type: MemoryType::Note,
            project: "test".into(),
            tags: vec!["testing".into()],
            created_at: format!("2026-05-{:02}T00:00:00Z", i + 2),
            accessed_at: format!("2026-05-{:02}T00:00:00Z", i + 2),
            ttl: MemoryTtl::Permanent,
            source: "user".into(),
        };
        store.remember(&distractor).unwrap();
    }

    let results = store.recall("", 5, None, Some("bug"), None).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, bug_memory.content);
}

#[test]
fn test_recall_matches_plural_bug_query_against_bug_tag() {
    let store = MemoryStore::in_memory().unwrap();

    let memory = Memory {
        id: String::new(),
        content: "Crash in checkout confirmation flow".into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["bug".into()],
        created_at: "2026-05-01T00:00:00Z".into(),
        accessed_at: "2026-05-01T00:00:00Z".into(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    };
    store.remember(&memory).unwrap();

    let results = store.recall("bugs", 10, None, None, None).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, memory.content);
}

#[test]
fn test_file_backed_store_reopen_recalls_remembered_memory() {
    let db_path = temp_db("reopen-recall");
    let memory = Memory {
        id: String::new(),
        content: "Checkout confirmation bug found during scenario testing".into(),
        memory_type: MemoryType::Note,
        project: "test".into(),
        tags: vec!["bug".into(), "scenario".into()],
        created_at: "2026-05-01T00:00:00Z".into(),
        accessed_at: "2026-05-01T00:00:00Z".into(),
        ttl: MemoryTtl::Permanent,
        source: "user".into(),
    };

    {
        let store = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();
        store.remember(&memory).unwrap();
        assert_eq!(store.count(), 1);
    }

    let reopened = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();
    let results = reopened
        .recall("bugs found during scenario testing", 10, None, None, None)
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, memory.content);
    assert!(results[0].tags.iter().any(|tag| tag == "bug"));

    let _ = fs::remove_file(db_path);
}
