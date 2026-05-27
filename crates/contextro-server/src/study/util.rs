use super::*;

pub(super) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let rendered = serde_json::to_string_pretty(value)?;
    fs::write(path, rendered)
        .with_context(|| format!("failed to write {}", path.to_string_lossy()))?;
    Ok(())
}

pub(super) fn relativize_path(root: &Path, absolute: &str) -> String {
    strip_base(absolute, Some(root.to_string_lossy().as_ref()))
}

pub(super) fn strip_base(file: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|base| Path::new(file).strip_prefix(base).ok())
        .map(|path| normalize_relative(&path.to_string_lossy()))
        .unwrap_or_else(|| normalize_relative(file))
}

pub(super) fn normalize_relative(path: &str) -> String {
    path.trim_start_matches("./").replace('\\', "/")
}

pub(super) fn is_reasonable_symbol_name(name: &str) -> bool {
    let len_ok = (3..=100).contains(&name.len());
    let starts_ok = name
        .chars()
        .next()
        .map(|ch| ch.is_ascii_alphabetic() || ch == '_')
        .unwrap_or(false);
    let chars_ok = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'));
    len_ok && starts_ok && chars_ok
}

pub(super) fn is_source_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SOURCE_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

pub(super) fn token_count(tokenizer: &CoreBPE, text: &str) -> usize {
    tokenizer.encode_with_special_tokens(text).len()
}

pub(super) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

pub(super) fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub(super) fn truncate_pad(value: &str, width: usize) -> String {
    let shortened = if value.len() > width {
        let keep = width.saturating_sub(1);
        format!("{}…", &value[..keep])
    } else {
        value.to_string()
    };
    format!("{shortened:<width$}")
}
