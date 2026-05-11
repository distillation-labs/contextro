//! Language registry: maps file extensions to languages and parser configs.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Map of file extension → language name.
static EXTENSION_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

fn extension_map() -> &'static HashMap<&'static str, &'static str> {
    EXTENSION_MAP.get_or_init(|| {
        let mut m = HashMap::new();
        // Python
        m.insert("py", "python");
        m.insert("pyi", "python");
        // JavaScript
        m.insert("js", "javascript");
        m.insert("mjs", "javascript");
        m.insert("cjs", "javascript");
        m.insert("jsx", "javascript");
        // TypeScript
        m.insert("ts", "typescript");
        m.insert("tsx", "typescript");
        m.insert("mts", "typescript");
        // Rust
        m.insert("rs", "rust");
        // Go
        m.insert("go", "go");
        // Java
        m.insert("java", "java");
        // C
        m.insert("c", "c");
        m.insert("h", "c");
        // C++
        m.insert("cpp", "cpp");
        m.insert("cc", "cpp");
        m.insert("cxx", "cpp");
        m.insert("hpp", "cpp");
        m.insert("hh", "cpp");
        // Ruby
        m.insert("rb", "ruby");
        // PHP
        m.insert("php", "php");
        // C#
        m.insert("cs", "c_sharp");
        // Kotlin
        m.insert("kt", "kotlin");
        m.insert("kts", "kotlin");
        // Swift
        m.insert("swift", "swift");
        // Scala
        m.insert("scala", "scala");
        // Lua
        m.insert("lua", "lua");
        m
    })
}

/// Get the language name for a file path, or None if unsupported.
pub fn get_language_for_file(filepath: &str) -> Option<&'static str> {
    let ext = Path::new(filepath).extension()?.to_str()?;
    extension_map().get(ext).copied()
}

/// Get all supported file extensions (without leading dot).
pub fn get_supported_extensions() -> Vec<&'static str> {
    extension_map().keys().copied().collect()
}

/// Check if a file extension is supported.
pub fn is_supported(filepath: &str) -> bool {
    get_language_for_file(filepath).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_detection() {
        assert_eq!(get_language_for_file("src/main.py"), Some("python"));
        assert_eq!(get_language_for_file("types.pyi"), Some("python"));
    }

    #[test]
    fn test_typescript_detection() {
        assert_eq!(get_language_for_file("app.tsx"), Some("typescript"));
    }

    #[test]
    fn test_unsupported() {
        assert_eq!(get_language_for_file("readme.md"), None);
        assert_eq!(get_language_for_file("data.json"), None);
    }
}
