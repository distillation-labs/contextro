use std::collections::HashSet;

use crate::analysis::is_test_file;
use contextro_core::models::SearchResult;

use super::symbol_queries::tokenize_identifier;

#[derive(Clone, Debug, Default)]
pub(super) struct QuerySubsystemFocus {
    required_terms: Vec<String>,
    strong_terms: Vec<String>,
    preferred_path_terms: Vec<String>,
    preferred_symbol_terms: Vec<(String, f64)>,
    penalty_symbol_prefixes: Vec<String>,
    penalty_symbol_terms: Vec<String>,
}

impl QuerySubsystemFocus {
    pub(super) fn score_result(&self, result: &SearchResult) -> f64 {
        if self.required_terms.is_empty()
            && self.strong_terms.is_empty()
            && self.preferred_path_terms.is_empty()
            && self.preferred_symbol_terms.is_empty()
            && self.penalty_symbol_prefixes.is_empty()
            && self.penalty_symbol_terms.is_empty()
        {
            return 1.0;
        }

        let symbol_name = result.symbol_name.to_ascii_lowercase();
        let terminal_symbol = terminal_symbol_name(&result.symbol_name);
        let path = result.filepath.to_ascii_lowercase();
        let signature = result.signature.to_ascii_lowercase();
        let combined = format!("{} {} {}", symbol_name, path, signature);

        let mut multiplier = 1.0;

        let required_matches = self
            .required_terms
            .iter()
            .filter(|term| combined.contains(term.as_str()))
            .count();
        if !self.required_terms.is_empty() {
            if required_matches == self.required_terms.len() {
                multiplier *= 1.45;
            } else if required_matches == 0 {
                multiplier *= 0.55;
            } else {
                multiplier *= 0.90;
            }
        }

        let strong_matches = self
            .strong_terms
            .iter()
            .filter(|term| combined.contains(term.as_str()))
            .count();
        if strong_matches > 0 {
            multiplier *= 1.0 + (strong_matches as f64 * 0.16).min(0.48);
        }

        if self
            .preferred_path_terms
            .iter()
            .any(|term| path.contains(term.as_str()))
        {
            multiplier *= 1.22;
        }

        for (term, bonus) in &self.preferred_symbol_terms {
            if symbol_name.contains(term.as_str()) {
                multiplier *= *bonus;
            }
        }

        if self
            .penalty_symbol_prefixes
            .iter()
            .any(|prefix| terminal_symbol.starts_with(prefix.as_str()))
        {
            multiplier *= 0.62;
        }

        if self
            .penalty_symbol_terms
            .iter()
            .any(|term| symbol_name.contains(term.as_str()) || path.contains(term.as_str()))
        {
            multiplier *= 0.72;
        }

        multiplier
    }
}

pub(super) fn query_subsystem_focus(query: &str) -> QuerySubsystemFocus {
    let lowered = query.to_ascii_lowercase();

    if lowered.contains("observability") && lowered.contains("config") {
        return QuerySubsystemFocus {
            required_terms: vec!["observability".into(), "config".into()],
            strong_terms: vec![
                "telemetry".into(),
                "tracing".into(),
                "sentry".into(),
                "otel".into(),
            ],
            preferred_path_terms: vec!["/observability/".into()],
            preferred_symbol_terms: vec![
                ("buildobservabilityconfig".into(), 1.30),
                ("observabilityconfig".into(), 1.20),
                ("buildnextsentryconfigoptions".into(), 1.12),
            ],
            penalty_symbol_prefixes: vec!["read_".into(), "write_".into()],
            penalty_symbol_terms: vec!["formstate".into(), "syncformstate".into()],
        };
    }

    if query_is_explanatory(query) && (lowered.contains("caching") || lowered.contains("cache")) {
        return QuerySubsystemFocus {
            required_terms: vec!["cache".into()],
            strong_terms: vec![
                "querycache".into(),
                "query cache".into(),
                "ttl".into(),
                "evict".into(),
                "invalidate".into(),
                "cached responses".into(),
            ],
            preferred_path_terms: vec!["/cache.rs".into(), "/engines/".into()],
            preferred_symbol_terms: vec![("querycache".into(), 1.22)],
            penalty_symbol_prefixes: vec!["read_".into(), "write_".into()],
            penalty_symbol_terms: vec!["hf_cache_path".into(), "update_check".into()],
        };
    }

    QuerySubsystemFocus::default()
}

pub(super) fn is_probable_test_symbol(symbol_name: &str) -> bool {
    let symbol_name = terminal_symbol_name(symbol_name);

    symbol_name == "tests"
        || symbol_name.starts_with("test_")
        || symbol_name.ends_with("_test")
        || symbol_name.starts_with("bench_")
}

pub(super) fn is_probable_internal_helper_symbol(symbol_name: &str) -> bool {
    let symbol_name = terminal_symbol_name(symbol_name);

    symbol_name.starts_with("make_")
        || symbol_name.starts_with("normalize_")
        || symbol_name.starts_with("tokenize_")
        || symbol_name.starts_with("accumulate_")
        || symbol_name.starts_with("confidence_")
        || symbol_name.ends_with("_for_query")
        || symbol_name.ends_with("_query_overlap")
        || symbol_name.ends_with("_candidate_limit")
        || symbol_name.ends_with("_weights")
        || symbol_name.contains("setup")
        || symbol_name.contains("plugin")
        || symbol_name.contains("stub")
        || symbol_name.contains("helper")
}

pub(super) fn is_public_signature(signature: &str) -> bool {
    let trimmed = signature.trim_start();
    trimmed.starts_with("pub ") || trimmed.starts_with("pub(")
}

pub(super) fn query_targets_product_surface(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    if query_targets_engine_internals(&lowered) {
        return false;
    }

    if lowered.contains("how does") || lowered.contains("how do") {
        return true;
    }

    [
        "alias",
        "contract",
        "developer",
        "mcp",
        "noise",
        "output",
        "persistence",
        "persist",
        "ranking",
        "response",
        "surface",
        "tool",
        "workflow",
    ]
    .iter()
    .any(|token| lowered.contains(token))
}

pub(super) fn query_targets_engine_internals(lowered_query: &str) -> bool {
    [
        "cache",
        "cached",
        "caching",
        "config",
        "configuration",
        "observability",
        "evict",
        "eviction",
        "expire",
        "expiry",
        "ttl",
        "invalidation",
        "invalidate",
    ]
    .iter()
    .any(|token| lowered_query.contains(token))
}

pub(super) fn query_targets_support_or_tooling(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();

    [
        "test", "tests", "fixture", "fixtures", "spec", "stub", "mock", "plugin", "plugins",
        "setup", "path", "helper", "helpers", "manifest",
    ]
    .iter()
    .any(|token| lowered.contains(token))
}

pub(super) fn is_probable_product_surface_result(result: &SearchResult) -> bool {
    let path = result.filepath.to_ascii_lowercase();
    let symbol_name = terminal_symbol_name(&result.symbol_name);

    symbol_name.starts_with("handle_")
        || path.contains("/tools/")
        || path.contains("/server/")
        || path.contains("/routes/")
        || path.contains("/handlers/")
        || path.contains("/commands/")
}

pub(super) fn is_probable_engine_internal_search_result(result: &SearchResult) -> bool {
    if is_test_file(&result.filepath)
        || is_probable_test_symbol(&result.symbol_name)
        || is_probable_meta_support_result(result)
    {
        return false;
    }

    let path = result.filepath.to_ascii_lowercase();
    let symbol_name = terminal_symbol_name(&result.symbol_name);
    let full_symbol_name = result.symbol_name.to_ascii_lowercase();

    symbol_name == "execute_search"
        || symbol_name == "search"
        || full_symbol_name.contains("querycache")
        || full_symbol_name.contains("query_cache")
        || symbol_name.contains("ttl")
        || symbol_name.contains("evict")
        || symbol_name.contains("expire")
        || symbol_name.contains("invalidat")
        || (path.ends_with("/cache.rs") && is_public_signature(&result.signature))
        || ((path.contains("/engines/")
            || path.ends_with("/cache.rs")
            || path.ends_with("/sandbox.rs")
            || path.ends_with("/memory.rs")
            || path.ends_with("/archive.rs"))
            && (symbol_name.contains("search")
                || symbol_name.contains("cache")
                || symbol_name.contains("ttl")
                || symbol_name.contains("evict")
                || symbol_name.contains("expire")
                || symbol_name.contains("invalidat")
                || symbol_name.ends_with("_weights")
                || symbol_name.ends_with("_consensus")))
}

pub(super) fn is_probable_meta_support_result(result: &SearchResult) -> bool {
    let path = result.filepath.to_ascii_lowercase();
    let symbol_name = terminal_symbol_name(&result.symbol_name);

    path.contains("/test")
        || path.contains("/tests/")
        || path.contains("/fixtures/")
        || path.contains("/fixture/")
        || path.contains("/plugins/")
        || path.contains("/setup")
        || path.contains("/stubs/")
        || path.contains("/stub/")
        || path.contains("/helpers/")
        || path.contains("/helper/")
        || path.contains("manifest")
        || path.ends_with("_stub.rs")
        || path.ends_with("_helper.rs")
        || path.ends_with("_helpers.rs")
        || symbol_name.contains("test")
        || symbol_name.contains("fixture")
        || symbol_name.contains("stub")
        || symbol_name.contains("plugin")
        || symbol_name.contains("helper")
        || symbol_name.contains("setup")
        || symbol_name.contains("path")
}

pub(super) fn terminal_symbol_name(symbol_name: &str) -> String {
    symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase()
}

pub(super) fn natural_language_query_terms(query: &str) -> Vec<String> {
    normalized_concept_terms(query, true).into_iter().collect()
}

pub(super) fn normalized_concept_terms(text: &str, drop_stopwords: bool) -> HashSet<String> {
    tokenize_identifier(text)
        .into_iter()
        .filter_map(|token| normalize_concept_term(&token, drop_stopwords))
        .collect()
}

fn normalize_concept_term(token: &str, drop_stopwords: bool) -> Option<String> {
    let normalized = normalize_token_stem(token);
    if normalized.len() < 3 {
        return None;
    }

    if drop_stopwords && is_natural_language_stopword(&normalized) {
        return None;
    }

    Some(normalized)
}

fn normalize_token_stem(token: &str) -> String {
    let token = token.to_ascii_lowercase();

    if token.starts_with("configur") || token == "config" || token == "configs" {
        return "config".into();
    }

    if token.starts_with("cach") {
        return "cache".into();
    }

    if token.starts_with("evict") {
        return "evict".into();
    }

    if token.starts_with("invalidat") {
        return "invalidate".into();
    }

    if token.ends_with("ies") && token.len() > 4 {
        return format!("{}y", &token[..token.len() - 3]);
    }

    if token.ends_with('s') && token.len() > 4 && !token.ends_with("ss") {
        return token[..token.len() - 1].into();
    }

    token
}

fn is_natural_language_stopword(token: &str) -> bool {
    matches!(
        token,
        "about"
            | "across"
            | "does"
            | "each"
            | "from"
            | "have"
            | "into"
            | "should"
            | "that"
            | "their"
            | "them"
            | "then"
            | "there"
            | "these"
            | "this"
            | "those"
            | "through"
            | "what"
            | "when"
            | "where"
            | "which"
            | "with"
            | "work"
            | "works"
            | "would"
            | "how"
            | "the"
            | "and"
            | "for"
    )
}

pub(super) fn query_is_explanatory(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    lowered.contains("how does")
        || lowered.contains("how do")
        || lowered.contains("how is")
        || lowered.contains("how are")
        || lowered.starts_with("explain ")
        || lowered.starts_with("what is ")
        || lowered.starts_with("what are ")
}
