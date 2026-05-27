use super::*;

#[test]
fn test_ts_arrow_functions() {
    let parser = TreeSitterParser::new();
    let tmp = std::env::temp_dir().join("test_ts_real.tsx");
    std::fs::write(
        &tmp,
        r#"
export const fetchUser = async (id: string) => {
    const result = await db.query(id);
    return transform(result);
};

export function processData(data: any) {
    return validate(data);
}

class UserService {
    async getUser(id: string) {
        return fetchUser(id);
    }
}

interface UserProps { id: string; }
type UserId = string;
enum Status { Active, Inactive }

const App = () => {
    return <UserService />;
};
"#,
    )
    .unwrap();

    let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();

    assert!(
        names.contains(&"fetchUser"),
        "Missing arrow fn: {:?}",
        names
    );
    assert!(
        names.contains(&"processData"),
        "Missing function: {:?}",
        names
    );
    assert!(names.contains(&"UserService"), "Missing class: {:?}", names);
    assert!(names.contains(&"getUser"), "Missing method: {:?}", names);
    assert!(
        names.contains(&"UserProps"),
        "Missing interface: {:?}",
        names
    );
    assert!(names.contains(&"UserId"), "Missing type alias: {:?}", names);
    assert!(names.contains(&"Status"), "Missing enum: {:?}", names);
    assert!(
        names.contains(&"App"),
        "Missing arrow component: {:?}",
        names
    );

    // Check calls
    let fetch = result
        .symbols
        .iter()
        .find(|s| s.name == "fetchUser")
        .unwrap();
    assert!(
        fetch.calls.contains(&"transform".to_string()),
        "fetchUser calls: {:?}",
        fetch.calls
    );

    // Check JSX call detection
    let app = result.symbols.iter().find(|s| s.name == "App").unwrap();
    assert!(
        app.calls.contains(&"UserService".to_string()),
        "App JSX calls: {:?}",
        app.calls
    );

    // Check method parent
    let get_user = result.symbols.iter().find(|s| s.name == "getUser").unwrap();
    assert_eq!(get_user.parent, Some("UserService".to_string()));

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_rust_real_parser() {
    let parser = TreeSitterParser::new();
    let tmp = std::env::temp_dir().join("test_rs_real.rs");
    std::fs::write(
        &tmp,
        r#"
/// A user store.
pub struct UserStore {
    db: Database,
}

impl UserStore {
    /// Create a new store.
    pub fn new(db: Database) -> Self {
        Self { db: validate(db) }
    }

    pub async fn get(&self, id: &str) -> Result<User> {
        let raw = self.db.query(id).await?;
        deserialize(raw)
    }
}

pub fn standalone() {
    helper();
}

enum Color { Red, Blue }
trait Drawable { fn draw(&self); }
"#,
    )
    .unwrap();

    let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();

    assert!(names.contains(&"UserStore"), "Missing struct: {:?}", names);
    assert!(names.contains(&"new"), "Missing method: {:?}", names);
    assert!(names.contains(&"get"), "Missing method: {:?}", names);
    assert!(names.contains(&"standalone"), "Missing fn: {:?}", names);
    assert!(names.contains(&"Color"), "Missing enum: {:?}", names);
    assert!(names.contains(&"Drawable"), "Missing trait: {:?}", names);

    let new_sym = result.symbols.iter().find(|s| s.name == "new").unwrap();
    assert!(
        new_sym.calls.contains(&"validate".to_string()),
        "new calls: {:?}",
        new_sym.calls
    );
    assert_eq!(new_sym.parent, Some("UserStore".to_string()));

    let get_sym = result.symbols.iter().find(|s| s.name == "get").unwrap();
    assert!(
        get_sym.calls.contains(&"deserialize".to_string()),
        "get calls: {:?}",
        get_sym.calls
    );

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_python_fallback() {
    let parser = TreeSitterParser::new();
    let tmp = std::env::temp_dir().join("test_py_fallback.py");
    std::fs::write(
        &tmp,
        "def hello():\n    \"\"\"Say hello.\"\"\"\n    print(\"hello\")\n\nclass Foo:\n    pass\n",
    )
    .unwrap();
    let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"hello"));
    assert!(names.contains(&"Foo"));
    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_rust_heuristic_handles_multiline_function_signatures() {
    let parser = TreeSitterParser::new();
    let tmp = std::env::temp_dir().join("test_rust_multiline_signature.rs");
    std::fs::write(
        &tmp,
        r#"
fn execute_search() {}
fn vector_search() {}

pub fn handle_search(
    query: &str,
    limit: usize,
) -> usize {
    let results = execute_search();
    if query.is_empty() {
        return limit;
    }
    vector_search();
    results.len()
}
"#,
    )
    .unwrap();
    let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
    let handle_search = result
        .symbols
        .iter()
        .find(|symbol| symbol.name == "handle_search")
        .unwrap();

    assert!(handle_search.line_end > handle_search.line_start);
    assert!(
        handle_search.calls.contains(&"execute_search".to_string()),
        "calls: {:?}",
        handle_search.calls
    );
    assert!(
        handle_search.calls.contains(&"vector_search".to_string()),
        "calls: {:?}",
        handle_search.calls
    );

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_rust_heuristic_resets_impl_scope_after_skipped_method_bodies() {
    let parser = TreeSitterParser::new();
    let tmp = std::env::temp_dir().join("test_rust_impl_scope_reset.rs");
    std::fs::write(
        &tmp,
        r#"
struct SearchOptions;

impl SearchOptions {
    fn helper(&self) {
        nested();
    }
}

fn nested() {}

pub fn execute_search() {
    nested();
}
"#,
    )
    .unwrap();
    let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
    let names: Vec<&str> = result
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    let helper = result
        .symbols
        .iter()
        .find(|symbol| symbol.name == "helper")
        .unwrap_or_else(|| panic!("missing helper symbol in {:?}", names));
    let execute_search = result
        .symbols
        .iter()
        .find(|symbol| symbol.name == "execute_search")
        .unwrap_or_else(|| panic!("missing execute_search symbol in {:?}", names));

    assert_eq!(helper.parent.as_deref(), Some("SearchOptions"));
    assert_eq!(execute_search.parent, None);

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_rust_heuristic_extracts_item_and_module_doc_context() {
    let parser = TreeSitterParser::new();
    let tmp = std::env::temp_dir().join("test_rust_doc_context.rs");
    std::fs::write(
        &tmp,
        r#"
//! Session archive persistence across restart.
//!
//! Retrieve archived session content after restart.

/// Retrieves archived session content by reference id.
#[instrument(skip(store))]
pub fn retrieve_archived_session() {}

/**
 * Repository registry for multi-repo indexing.
 */
#[derive(Debug)]
pub struct RepoRegistry;
"#,
    )
    .unwrap();

    let result = parser.parse_file(tmp.to_str().unwrap()).unwrap();
    let retrieve = result
        .symbols
        .iter()
        .find(|symbol| symbol.name == "retrieve_archived_session")
        .unwrap();
    let repo_registry = result
        .symbols
        .iter()
        .find(|symbol| symbol.name == "RepoRegistry")
        .unwrap();

    assert!(
        retrieve
            .docstring
            .contains("Retrieves archived session content by reference id"),
        "retrieve docstring: {}",
        retrieve.docstring
    );
    assert!(
        retrieve
            .docstring
            .contains("Session archive persistence across restart"),
        "retrieve docstring: {}",
        retrieve.docstring
    );
    assert!(
        repo_registry
            .docstring
            .contains("Repository registry for multi-repo indexing"),
        "RepoRegistry docstring: {}",
        repo_registry.docstring
    );

    std::fs::remove_file(tmp).ok();
}

#[test]
fn test_can_parse() {
    let parser = TreeSitterParser::new();
    assert!(parser.can_parse("main.py"));
    assert!(parser.can_parse("app.ts"));
    assert!(parser.can_parse("component.tsx"));
    assert!(!parser.can_parse("readme.md"));
}
