use super::*;

pub(crate) fn tokenize_codebase_map_text(text: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(text.len() * 2);
    let mut prev_was_lower_or_digit = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_was_lower_or_digit {
                normalized.push(' ');
            }
            normalized.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            normalized.push(' ');
            prev_was_lower_or_digit = false;
        }
    }

    let mut tokens = Vec::new();
    let mut seen = HashSet::new();

    for token in normalized.split_whitespace() {
        for variant in codebase_map_token_variants(token) {
            if seen.insert(variant.clone()) {
                tokens.push(variant);
            }
        }
    }

    tokens
}

pub(crate) fn codebase_map_query_targets_tests(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    ["test", "tests", "pytest", "spec", "fixture"]
        .iter()
        .any(|token| lowered.contains(token))
}

pub(crate) fn is_probable_codebase_map_test_symbol(symbol_name: &str) -> bool {
    let symbol_name = symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase();

    symbol_name == "tests"
        || symbol_name.starts_with("test_")
        || symbol_name.ends_with("_test")
        || symbol_name.starts_with("bench_")
}

pub(crate) fn codebase_map_token_variants(token: &str) -> Vec<String> {
    let token = token.trim().to_ascii_lowercase();
    if token.len() < 3 || is_codebase_map_stopword(&token) {
        return Vec::new();
    }

    let mut variants = vec![token.clone()];
    if let Some(stemmed) = stem_codebase_map_token(&token) {
        if stemmed != token {
            variants.push(stemmed);
        }
    }
    variants
}

pub(crate) fn stem_codebase_map_token(token: &str) -> Option<String> {
    let stemmed = if token.ends_with("ing") && token.len() > 5 {
        restore_codebase_map_stemmed_root(&token[..token.len() - 3])
    } else if token.ends_with("ers") && token.len() > 5 {
        token[..token.len() - 3].to_string()
    } else if token.ends_with("er") && token.len() > 4 {
        token[..token.len() - 2].to_string()
    } else if token.ends_with("ed") && token.len() > 4 {
        restore_codebase_map_stemmed_root(&token[..token.len() - 2])
    } else if token.ends_with("es") && token.len() > 4 {
        token[..token.len() - 2].to_string()
    } else if token.ends_with('s') && token.len() > 4 {
        token[..token.len() - 1].to_string()
    } else {
        token.to_string()
    };

    (stemmed.len() >= 3).then_some(stemmed)
}

pub(crate) fn restore_codebase_map_stemmed_root(base: &str) -> String {
    if base.ends_with("ch") || base.ends_with("sh") || base.ends_with('v') || base.ends_with('c') {
        format!("{base}e")
    } else {
        base.to_string()
    }
}

pub(crate) fn is_codebase_map_stopword(token: &str) -> bool {
    matches!(
        token,
        "and"
            | "are"
            | "does"
            | "for"
            | "from"
            | "how"
            | "into"
            | "the"
            | "this"
            | "that"
            | "what"
            | "when"
            | "where"
            | "which"
            | "with"
            | "work"
            | "works"
    )
}
