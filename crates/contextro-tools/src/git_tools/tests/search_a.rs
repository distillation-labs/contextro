use super::*;
use contextro_config::get_settings;

#[test]
fn test_handle_commit_search_returns_differentiated_scores() {
    let repo_dir = temp_file("commit-search-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
    let mut parent: Option<git2::Oid> = None;

    for (idx, message) in [
        "chore release housekeeping",
        "fix session tracker bug",
        "fix reliability regression in session tracker",
    ]
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
        &json!({"query":"fix reliability","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );
    let commits = result["commits"].as_array().expect("commits array");

    assert_eq!(
        commits[0]["message"],
        "fix reliability regression in session tracker"
    );
    assert!(commits[0]["score"].as_f64().unwrap() > commits[1]["score"].as_f64().unwrap());

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_handle_commit_search_differentiates_single_token_release_queries() {
    let repo_dir = temp_file("commit-search-release-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
    let mut parent: Option<git2::Oid> = None;

    for (idx, message) in [
        "Release v1.6.3",
        "Update publication and release artifacts",
        "ci: add cargo publish job to release workflow",
    ]
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
        &json!({"query":"release","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );
    let commits = result["commits"].as_array().expect("commits array");

    assert_eq!(commits[0]["message"], "Release v1.6.3");
    assert!(commits[0]["score"].as_f64().unwrap() > commits[1]["score"].as_f64().unwrap());
    assert!(commits[1]["score"].as_f64().unwrap() > commits[2]["score"].as_f64().unwrap());

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_handle_commit_search_does_not_eagerly_expand_cache_on_initial_hit() {
    let repo_dir = temp_file("commit-search-cache-hit-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
    let mut parent: Option<git2::Oid> = None;

    for (idx, message) in [
        "fix reliability regression in session tracker",
        "chore release housekeeping",
        "docs update benchmark notes",
    ]
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
        &json!({"query":"docs","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["total"], 1);

    let initial_scan_limit = get_settings().read().commit_history_limit.max(500);
    let repo = git2::Repository::discover(&repo_dir).unwrap();
    let repo_key = commit_search_repo_key(&repo);
    let entry = commit_search_cache()
        .read()
        .get(&repo_key)
        .cloned()
        .expect("cache entry for repo");

    assert_eq!(entry.head_hash, commit_search_head_hash(&repo));
    assert_eq!(entry.scan_limit, initial_scan_limit);

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_handle_commit_search_returns_cached_final_response() {
    let repo_dir = temp_file("commit-search-result-cache-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
    std::fs::write(repo_dir.join("tracked.txt"), "seed\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("tracked.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "initial commit",
        &tree,
        &[],
    )
    .unwrap();

    let repo_key = commit_search_repo_key(&repo);
    let head_hash = commit_search_head_hash(&repo);
    let cached = json!({
        "query": "release",
        "commits": [{
            "hash": "deadbeef1234",
            "message": "Release v9.9.9",
            "author": "Cache",
            "score": 1.0
        }],
        "total": 1
    });
    commit_search_result_cache().write().insert(
        commit_search_result_cache_key(&repo_key, &head_hash, "release", None, 5),
        cached.clone(),
    );

    let result = handle_commit_search(
        &json!({"query":"release","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result, cached);

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_handle_commit_search_result_cache_invalidates_on_head_change() {
    let repo_dir = temp_file("commit-search-result-cache-head-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();

    std::fs::write(repo_dir.join("tracked.txt"), "seed\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("tracked.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let base = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "initial import",
            &tree,
            &[],
        )
        .unwrap();

    let before = handle_commit_search(
        &json!({"query":"release","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );
    assert_eq!(before["total"], 0);

    std::fs::write(repo_dir.join("tracked.txt"), "release\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("tracked.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let parent = repo.find_commit(base).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "release benchmark cache",
        &tree,
        &[&parent],
    )
    .unwrap();

    let after = handle_commit_search(
        &json!({"query":"release","limit":5}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(after["total"], 1, "unexpected result: {after}");
    assert_eq!(after["commits"][0]["message"], "release benchmark cache");

    let _ = std::fs::remove_dir_all(repo_dir);
}
