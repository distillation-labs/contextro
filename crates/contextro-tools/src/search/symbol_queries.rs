use std::collections::HashSet;

use contextro_core::models::SearchResult;

pub(super) fn drop_low_confidence_noise(
    query: &str,
    mode: &str,
    results: Vec<SearchResult>,
) -> Vec<SearchResult> {
    let mut results = results;

    if results.is_empty() {
        return results;
    }

    if mode == "vector" && vector_query_requires_literal_grounding(query) {
        results.retain(|result| result_has_literal_query_grounding(query, result));
        if results.is_empty() {
            return results;
        }
    }

    let min_score = if mode == "vector" {
        if is_symbol_lookup_query(query) {
            0.15
        } else {
            0.18
        }
    } else if is_symbol_lookup_query(query) {
        0.12
    } else {
        0.18
    };

    let relative_floor = if mode == "vector" {
        let top_score = results[0].score.max(0.0);
        if top_score >= min_score {
            let ratio = if is_symbol_lookup_query(query) {
                0.72
            } else {
                0.70
            };
            top_score * ratio
        } else {
            min_score
        }
    } else {
        min_score
    };

    results
        .into_iter()
        .filter(|result| result.score >= min_score && result.score >= relative_floor)
        .collect()
}

fn vector_query_requires_literal_grounding(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().count() == 1
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
}

fn result_has_literal_query_grounding(query: &str, result: &SearchResult) -> bool {
    let normalized_query = normalize_identifier(query);
    if normalized_query.len() < 3 {
        return true;
    }

    [
        result.symbol_name.as_str(),
        result.filepath.as_str(),
        result.signature.as_str(),
    ]
    .iter()
    .any(|field| normalize_identifier(field).contains(&normalized_query))
}

pub(super) fn apply_symbol_query_guard(
    query: &str,
    results: Vec<SearchResult>,
) -> Vec<SearchResult> {
    if !is_symbol_lookup_query(query) {
        return results;
    }

    let normalized_query = normalize_identifier(query);
    if normalized_query.len() < 3 {
        return results;
    }

    results
        .into_iter()
        .filter(|result| result_matches_symbol_query(query, &normalized_query, result))
        .collect()
}

pub(super) fn is_symbol_lookup_query(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty() && trimmed.split_whitespace().count() == 1
}

pub(super) fn is_exact_symbol_lookup_query(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().count() == 1
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
        && (trimmed.contains('_')
            || trimmed.contains('.')
            || trimmed.chars().any(|ch| ch.is_ascii_uppercase()))
}

pub(super) fn is_bm25_identifier_exact_match_query(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().count() == 1
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | ':'))
}

pub(super) fn query_explicitly_targets_tests(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    ["test", "tests", "pytest", "spec", "fixture"]
        .iter()
        .any(|token| lowered.contains(token))
}

fn result_matches_symbol_query(query: &str, normalized_query: &str, result: &SearchResult) -> bool {
    let query_tokens = tokenize_identifier(query);
    if query_tokens.is_empty() {
        return true;
    }

    let matched = [result.symbol_name.as_str(), result.filepath.as_str()]
        .iter()
        .map(|field| {
            let normalized_field = normalize_identifier(field);
            if !normalized_query.is_empty() && normalized_field.contains(normalized_query) {
                return query_tokens.len();
            }

            let field_tokens: HashSet<String> = tokenize_identifier(field).into_iter().collect();
            query_tokens
                .iter()
                .filter(|token| {
                    field_tokens.iter().any(|candidate| {
                        candidate.contains(token.as_str()) || token.contains(candidate)
                    })
                })
                .count()
        })
        .max()
        .unwrap_or(0);

    match query_tokens.len() {
        0 => true,
        1 => matched == 1,
        2 => matched == 2,
        _ => matched * 2 >= query_tokens.len(),
    }
}

pub(super) fn normalize_identifier(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

pub(super) fn tokenize_identifier(text: &str) -> Vec<String> {
    let mut spaced = String::with_capacity(text.len() * 2);
    let mut prev_was_lower_or_digit = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_was_lower_or_digit {
                spaced.push(' ');
            }
            spaced.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            spaced.push(' ');
            prev_was_lower_or_digit = false;
        }
    }

    spaced
        .split_whitespace()
        .filter(|token| token.len() >= 3)
        .map(String::from)
        .collect()
}
