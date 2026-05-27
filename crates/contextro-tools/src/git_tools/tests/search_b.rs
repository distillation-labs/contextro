use super::*;

#[test]
fn test_handle_commit_search_falls_back_beyond_initial_scan_limit() {
    let repo_dir = temp_file("commit-search-fallback-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
    let mut parent: Option<git2::Oid> = None;

    for (idx, message) in [
        "fix persistence regression in knowledge store",
        "knowledge persistence retrospective",
    ]
    .iter()
    .enumerate()
    {
        std::fs::write(repo_dir.join("tracked.txt"), format!("seed-{idx}\n")).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parents = parent
            .map(|oid| vec![repo.find_commit(oid).unwrap()])
            .unwrap_or_default();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        let oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .unwrap();
        parent = Some(oid);
    }

    for idx in 0..505 {
        std::fs::write(repo_dir.join("tracked.txt"), format!("filler-{idx}\n")).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parents = parent
            .map(|oid| vec![repo.find_commit(oid).unwrap()])
            .unwrap_or_default();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        let oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &format!("chore filler commit {idx}"),
                &tree,
                &parent_refs,
            )
            .unwrap();
        parent = Some(oid);
    }

    let fix_result = handle_commit_search(
        &json!({"query":"fix","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );
    let knowledge_result = handle_commit_search(
        &json!({"query":"knowledge","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(fix_result["total"], 1);
    assert_eq!(
        fix_result["commits"][0]["message"],
        "fix persistence regression in knowledge store"
    );
    assert_eq!(knowledge_result["total"], 2);

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_handle_commit_search_matches_terse_update_commits_via_diff_context() {
    let repo_dir = temp_file("commit-search-diff-context-repo");
    let bm25_path = repo_dir.join("crates/contextro-engines/src");
    std::fs::create_dir_all(&bm25_path).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();

    let file = bm25_path.join("bm25.rs");
    std::fs::write(&file, "pub fn build_query() { parse bm25 query terms }\n").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(Path::new("crates/contextro-engines/src/bm25.rs"))
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let base_commit = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "initial import",
            &tree,
            &[],
        )
        .unwrap();

    std::fs::write(
        &file,
        "pub fn build_query() { query aware confidence scoring for bm25 tokens }\n",
    )
    .unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(Path::new("crates/contextro-engines/src/bm25.rs"))
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let parent = repo.find_commit(base_commit).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Update crates/contextro-engines/src/bm25.rs",
        &tree,
        &[&parent],
    )
    .unwrap();

    let result = handle_commit_search(
        &json!({"query":"query aware confidence scoring","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 1, "unexpected result: {result}");
    assert_eq!(
        result["commits"][0]["message"],
        "Update crates/contextro-engines/src/bm25.rs"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_handle_commit_history_applies_author_and_since_filters() {
    let repo_dir = temp_file("commit-history-filters-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let mut parent: Option<git2::Oid> = None;

    for (idx, (author, email, message)) in [
        ("Alice Example", "alice@example.com", "first commit"),
        ("Bob Example", "bob@example.com", "second commit"),
    ]
    .iter()
    .enumerate()
    {
        let signature = git2::Signature::now(author, email).unwrap();
        std::fs::write(repo_dir.join("tracked.txt"), format!("commit-{idx}\n")).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parents = parent
            .map(|oid| vec![repo.find_commit(oid).unwrap()])
            .unwrap_or_default();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        let oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .unwrap();
        parent = Some(oid);
    }

    let author_result = handle_commit_history(
        &json!({"author":"Bob Example","limit":10}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );
    assert_eq!(author_result["total"], 1);
    assert_eq!(author_result["commits"][0]["author"], "Bob Example");

    let future_result = handle_commit_history(
        &json!({"since":"2999-01-01","limit":10}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );
    assert_eq!(future_result["total"], 0);

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_commit_search_filters_nonsense_queries_below_threshold() {
    let repo_dir = temp_file("commit-search-nonsense-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
    let mut parent: Option<git2::Oid> = None;

    for (idx, message) in ["Update LICENSE", "Update Dockerfile", "Update .gitignore"]
        .iter()
        .enumerate()
    {
        std::fs::write(repo_dir.join("tracked.txt"), format!("commit-{idx}\n")).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parents = parent
            .map(|oid| vec![repo.find_commit(oid).unwrap()])
            .unwrap_or_default();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        let oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .unwrap();
        parent = Some(oid);
    }

    let result = handle_commit_search(
        &json!({"query":"___definitely_not_a_real_commit_phrase___","limit":3}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 0);
    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_repo_add_reports_non_git_directory() {
    let path = temp_file("repo-add-non-git.json");
    let repo_dir = temp_file("repo-add-non-git-dir");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let registry = RepoRegistry::with_path(&path);
    let result = handle_repo_add(
        &json!({"path": repo_dir.to_string_lossy().to_string()}),
        &registry,
    );

    assert_eq!(result["registered"], true);
    assert_eq!(result["is_git"], false);
    assert!(result["hint"]
        .as_str()
        .unwrap_or("")
        .contains("non-git directory"));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir_all(repo_dir);
}
