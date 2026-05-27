use serde_json::{json, Value};

pub(super) fn format_response(value: &Value, max_tokens: usize) -> String {
    let cleaned = strip_empty_nested(value, true);
    let output = cleaned.to_string();

    // #4: Smart max_tokens default — auto-truncate at 8000 chars (~2000 tokens) if no explicit budget
    let effective_budget = if max_tokens > 0 {
        max_tokens * 4
    } else if output.len() > 8000 {
        8000 // ~2000 tokens default cap
    } else {
        0 // no truncation needed
    };

    if effective_budget > 0 && output.len() > effective_budget {
        let budget_tokens = effective_budget / 4;
        return truncate_response_json(cleaned, effective_budget, budget_tokens).to_string();
    }
    output
}

fn truncate_response_json(value: Value, budget_chars: usize, budget_tokens: usize) -> Value {
    let mut candidate = value;
    for _ in 0..12 {
        let with_meta = add_truncation_metadata(candidate.clone(), budget_tokens);
        if with_meta.to_string().len() <= budget_chars {
            return with_meta;
        }
        candidate = shrink_json_value(candidate);
    }

    json!({
        "truncated": true,
        "hint": truncation_hint(budget_tokens),
        "summary": "Response exceeded the token budget. Narrow your query or request a larger max_tokens budget.",
    })
}

fn add_truncation_metadata(value: Value, budget_tokens: usize) -> Value {
    match value {
        Value::Object(mut map) => {
            map.insert("truncated".into(), Value::Bool(true));
            map.insert("hint".into(), Value::String(truncation_hint(budget_tokens)));
            Value::Object(map)
        }
        other => json!({
            "truncated": true,
            "hint": truncation_hint(budget_tokens),
            "value": other,
        }),
    }
}

fn truncation_hint(budget_tokens: usize) -> String {
    format!(
        "Response truncated to ~{} tokens. Use max_tokens for a different budget, or narrow your query.",
        budget_tokens
    )
}

fn shrink_json_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(shrink_text(text)),
        Value::Array(items) => {
            let target_len = if items.len() > 1 {
                items.len().div_ceil(2)
            } else {
                1
            };
            Value::Array(
                items
                    .into_iter()
                    .take(target_len)
                    .map(shrink_json_value)
                    .collect(),
            )
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, shrink_json_value(value)))
                .collect(),
        ),
        other => other,
    }
}

fn shrink_text(mut text: String) -> String {
    let len = text.chars().count();
    if len <= 80 {
        return text;
    }

    let target = (len * 2 / 3).max(80);
    text = text.chars().take(target.saturating_sub(1)).collect();
    text.push('…');
    text
}

pub(super) fn take_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn strip_empty_nested(value: &Value, is_top_level: bool) -> Value {
    match value {
        Value::Object(map) => {
            let cleaned: serde_json::Map<String, Value> = map
                .iter()
                .filter(|(_, v)| is_top_level || !is_empty_value(v))
                .map(|(k, v)| (k.clone(), strip_empty_nested(v, false)))
                .collect();
            Value::Object(cleaned)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| strip_empty_nested(v, false)).collect())
        }
        _ => value.clone(),
    }
}

fn is_empty_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(m) => m.is_empty(),
        _ => false,
    }
}

/// Strip absolute codebase prefixes from display-oriented file paths in responses,
/// while preserving identity-bearing root fields like `codebase_path` and repo `path`.
pub(super) fn strip_response_paths(value: Value, base: &str) -> Value {
    let prefix = format!("{base}/");
    strip_absolute_paths_impl(value, base, &prefix, true, None)
}

pub(super) fn response_needs_path_stripping(value: &Value, base: &str) -> bool {
    let prefix = format!("{base}/");
    response_contains_path_prefix(value, base, &prefix, true, None)
}

fn response_contains_path_prefix(
    value: &Value,
    base: &str,
    prefix: &str,
    preserve_identity_paths: bool,
    parent_key: Option<&str>,
) -> bool {
    match value {
        Value::String(s) => {
            if preserve_identity_paths && matches!(parent_key, Some("codebase_path" | "path")) {
                return false;
            }
            s == base || s.starts_with(prefix)
        }
        Value::Object(map) => map.iter().any(|(key, value)| {
            response_contains_path_prefix(
                value,
                base,
                prefix,
                preserve_identity_paths,
                Some(key.as_str()),
            )
        }),
        Value::Array(items) => items.iter().any(|value| {
            response_contains_path_prefix(value, base, prefix, preserve_identity_paths, parent_key)
        }),
        _ => false,
    }
}

/// Recursively replace any string value that starts with `base/` with the relative path.
fn strip_absolute_paths(value: Value, base: &str) -> Value {
    let prefix = format!("{base}/");
    strip_absolute_paths_impl(value, base, &prefix, false, None)
}

fn strip_absolute_paths_impl(
    value: Value,
    base: &str,
    prefix: &str,
    preserve_identity_paths: bool,
    parent_key: Option<&str>,
) -> Value {
    match value {
        Value::String(s) => {
            if preserve_identity_paths && matches!(parent_key, Some("codebase_path" | "path")) {
                return Value::String(s);
            }
            if let Some(stripped) = s.strip_prefix(prefix) {
                Value::String(stripped.to_string())
            } else if s == base {
                Value::String(".".to_string())
            } else {
                Value::String(s)
            }
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let stripped = strip_absolute_paths_impl(
                        v,
                        base,
                        prefix,
                        preserve_identity_paths,
                        Some(k.as_str()),
                    );
                    (k, stripped)
                })
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.into_iter()
                .map(|v| {
                    strip_absolute_paths_impl(v, base, prefix, preserve_identity_paths, parent_key)
                })
                .collect(),
        ),
        other => other,
    }
}

pub(super) fn strip_codebase(path: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|b| std::path::Path::new(path).strip_prefix(b).ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

pub(super) fn summarize_tool_call(name: &str, args: &Value, codebase: Option<&str>) -> String {
    if name == "compact" {
        let chars = args
            .get("content")
            .and_then(|value| value.as_str())
            .map(|content| content.len())
            .unwrap_or(0);
        return format!("compact(chars={chars})");
    }

    let sanitized = sanitize_tool_args(args, codebase);
    let Some(map) = sanitized.as_object() else {
        return format!("{name}()");
    };
    if map.is_empty() {
        return format!("{name}()");
    }

    let mut parts: Vec<String> = map
        .iter()
        .take(3)
        .map(|(key, value)| format!("{key}={}", summarize_value(value)))
        .collect();
    if map.len() > 3 {
        parts.push("…".into());
    }
    format!("{name}({})", parts.join(", "))
}

pub(super) fn sanitize_tool_args(args: &Value, codebase: Option<&str>) -> Value {
    let stripped = if let Some(base) = codebase {
        strip_absolute_paths(args.clone(), base)
    } else {
        args.clone()
    };
    truncate_json_strings(stripped, 160)
}

fn truncate_json_strings(value: Value, max_len: usize) -> Value {
    match value {
        Value::String(mut text) => {
            if text.len() > max_len {
                text.truncate(max_len);
                text.push('…');
            }
            Value::String(text)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| truncate_json_strings(item, max_len))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, truncate_json_strings(value, max_len)))
                .collect(),
        ),
        other => other,
    }
}

fn summarize_value(value: &Value) -> String {
    match value {
        Value::String(text) => format!("{text:?}"),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Null => "null".into(),
        Value::Array(items) => format!("[{} item(s)]", items.len()),
        Value::Object(map) => format!("{{{} key(s)}}", map.len()),
    }
}
