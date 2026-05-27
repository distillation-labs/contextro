use super::{find_tool_doc, tool_docs_for_tier, ToolTier};

#[test]
fn standard_tier_excludes_full_only_tools() {
    let names: Vec<&str> = tool_docs_for_tier(ToolTier::Standard)
        .into_iter()
        .map(|doc| doc.name)
        .collect();

    assert!(names.contains(&"search"));
    assert!(names.contains(&"introspect"));
    assert!(!names.contains(&"audit"));
    assert!(!names.contains(&"docs_bundle"));
}

#[test]
fn find_tool_doc_is_case_insensitive() {
    let search = find_tool_doc("SeArCh").expect("search tool manifest entry");
    assert_eq!(search.name, "search");
}
