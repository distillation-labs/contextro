use super::*;

#[test]
fn test_repo_registry_persists_to_disk() {
    let path = temp_file("repos.json");
    let repo_dir = std::env::temp_dir().join("contextro-repo-registry-test");
    let _ = std::fs::create_dir_all(&repo_dir);

    let registry = RepoRegistry::with_path(&path);
    assert!(registry.add(repo_dir.to_string_lossy().as_ref(), Some("repo")));

    let reloaded = RepoRegistry::with_path(&path);
    let repos = reloaded.list();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].1, "repo");

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_repo_remove_accepts_name() {
    let path = temp_file("repos-remove.json");
    let repo_dir = std::env::temp_dir().join("contextro-repo-remove-name-test");
    let _ = std::fs::create_dir_all(&repo_dir);

    let registry = RepoRegistry::with_path(&path);
    assert!(registry.add(repo_dir.to_string_lossy().as_ref(), Some("repo-by-name")));

    let result = handle_repo_remove(&json!({"name":"repo-by-name"}), &registry);
    assert_eq!(result["removed"], true);
    assert_eq!(result["name"], "repo-by-name");
    assert!(registry.list().is_empty());

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_token_overlap_score_rewards_exact_phrase_and_density() {
    let exact = token_overlap_score(
        "fix reliability",
        &tokenize("fix reliability"),
        "fix reliability bug in session tracker",
        &tokenize("fix reliability bug in session tracker"),
    );
    let partial = token_overlap_score(
        "fix reliability",
        &tokenize("fix reliability"),
        "fix session tracker bug",
        &tokenize("fix session tracker bug"),
    );
    let diluted = token_overlap_score(
        "fix reliability",
        &tokenize("fix reliability"),
        "fix the repo registry and update changelog entries for release housekeeping",
        &tokenize("fix the repo registry and update changelog entries for release housekeeping"),
    );

    assert!(
        exact > partial,
        "exact phrase should outrank partial overlap"
    );
    assert!(
        partial > diluted,
        "denser partial match should outrank diluted overlap"
    );
}

#[test]
fn test_token_overlap_score_prefers_prefix_subtoken_matches() {
    let bug_prefix = token_overlap_score(
        "fix bug",
        &tokenize("fix bug"),
        "add issue template bug_report",
        &tokenize("add issue template bug_report"),
    );
    let bug_suffix = token_overlap_score(
        "fix bug",
        &tokenize("fix bug"),
        "add issue template element_detection_bug",
        &tokenize("add issue template element_detection_bug"),
    );

    assert!(
        bug_prefix > bug_suffix,
        "prefix matches should outrank looser suffix-only matches for short queries"
    );
}
