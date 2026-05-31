use super::*;

#[test]
fn test_handle_diff_preview_summarizes_working_tree_changes() {
    let repo_dir = temp_file("diff-preview-worktree");
    let src_dir = repo_dir.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();

    let tracked = src_dir.join("lib.rs");
    std::fs::write(&tracked, "pub fn version() -> u32 { 1 }\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("src/lib.rs")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "initial import",
        &tree,
        &[],
    )
    .unwrap();

    std::fs::write(&tracked, "pub fn version() -> u32 { 2 }\n").unwrap();
    let added = repo_dir.join("src/new.rs");
    std::fs::write(&added, "pub fn added() {}\n").unwrap();

    let result = handle_diff_preview(
        &json!({"limit": 10, "preview_lines": 2}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["mode"], "worktree");
    assert_eq!(result["base"], "HEAD");
    assert_eq!(result["head"], "WORKTREE");
    assert!(result["diffstat"]["files"].as_u64().unwrap() >= 2);
    assert!(result["branch"].as_str().unwrap_or("").len() >= 3);

    let files = result["files"].as_array().expect("files array");
    assert!(files.iter().any(|entry| {
        entry["path"] == "src/lib.rs"
            && entry["status"] == "modified"
            && entry["preview"]
                .as_array()
                .map(|preview| {
                    preview.iter().any(|line| {
                        line.as_str()
                            .unwrap_or("")
                            .contains("version() -> u32 { 2 }")
                    })
                })
                .unwrap_or(false)
    }));
    assert!(files
        .iter()
        .any(|entry| entry["path"] == "src/new.rs" && entry["status"] == "untracked"));

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn test_handle_diff_preview_supports_revision_range_and_path_filter() {
    let repo_dir = temp_file("diff-preview-range");
    let src_dir = repo_dir.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    let repo = git2::Repository::init(&repo_dir).unwrap();
    let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();

    let tracked = src_dir.join("lib.rs");
    let readme = repo_dir.join("README.md");
    std::fs::write(&tracked, "pub fn bench_fixture() {}\n").unwrap();
    std::fs::write(&readme, "# Contextro\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("src/lib.rs")).unwrap();
    index.add_path(Path::new("README.md")).unwrap();
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

    std::fs::write(
        &tracked,
        "pub fn bench_fixture() { println!(\"updated\"); }\n",
    )
    .unwrap();
    std::fs::write(&readme, "# Contextro\n\nUpdated\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("src/lib.rs")).unwrap();
    index.add_path(Path::new("README.md")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let parent = repo.find_commit(base).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "update fixture",
        &tree,
        &[&parent],
    )
    .unwrap();

    let result = handle_diff_preview(
        &json!({"base":"HEAD~1","head":"HEAD","path":"src","limit":10,"preview_lines":1}),
        Some(repo_dir.to_string_lossy().as_ref()),
    );

    assert_eq!(result["mode"], "range");
    assert_eq!(result["base"], "HEAD~1");
    assert_eq!(result["head"], "HEAD");
    assert_eq!(result["candidate_total"], 1);
    let files = result["files"].as_array().expect("files array");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "src/lib.rs");
    assert_eq!(files[0]["status"], "modified");
    assert_eq!(files[0]["preview"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(repo_dir);
}
