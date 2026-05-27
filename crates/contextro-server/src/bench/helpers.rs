use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rmcp::model::{CallToolResult, RawContent};
use serde_json::Value;

use super::cases::ToolCase;
use super::ContextroServer;

#[derive(Clone)]
pub(crate) struct ToolBenchmark {
    pub(crate) display_name: &'static str,
    pub(crate) tool_name: &'static str,
    pub(crate) avg_ms: f64,
    pub(crate) p50_ms: f64,
    pub(crate) p95_ms: f64,
    pub(crate) notes: &'static str,
}

pub(crate) fn make_tool_benchmark(
    display_name: &'static str,
    tool_name: &'static str,
    notes: &'static str,
    times: &[Duration],
) -> ToolBenchmark {
    let avg_ms = times
        .iter()
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .sum::<f64>()
        / times.len() as f64;
    let p50_ms = percentile_ms(times, 0.50);
    let p95_ms = percentile_ms(times, 0.95);

    ToolBenchmark {
        display_name,
        tool_name,
        avg_ms,
        p50_ms,
        p95_ms,
        notes,
    }
}

pub(crate) fn print_tool_benchmark(benchmark: &ToolBenchmark) {
    println!(
        "║  {:15} avg:{:>7.2}ms  p50:{:>6.2}ms  p95:{:>7.2}ms  ║",
        benchmark.display_name, benchmark.avg_ms, benchmark.p50_ms, benchmark.p95_ms
    );
}

pub(crate) fn parse_tool_json(result: CallToolResult) -> Value {
    let Some(content) = result.content.first() else {
        panic!("tool returned no content");
    };
    let text = match &content.raw {
        RawContent::Text(text) => text.text.clone(),
        other => panic!("unexpected non-text tool content: {other:?}"),
    };
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse tool json: {error}; payload={text}"))
}

pub(crate) fn ensure_success(tool_name: &str, result: &Value) {
    if result.get("error").is_some() {
        panic!("tool '{tool_name}' failed: {result}");
    }
}

pub(crate) fn ensure_case_result(case: &ToolCase, result: &Value) {
    if result.get("error").is_some() && !case.allow_error {
        panic!(
            "tool '{}' failed during benchmark: {}",
            case.tool_name, result
        );
    }
}

fn percentile_ms(times: &[Duration], percentile: f64) -> f64 {
    let index = ((times.len().saturating_sub(1)) as f64 * percentile).round() as usize;
    times[index].as_secs_f64() * 1000.0
}

pub(crate) fn wrap_list(items: &[String], width: usize) -> Vec<String> {
    if items.is_empty() {
        return vec!["none".into()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for item in items {
        let candidate = if current.is_empty() {
            item.clone()
        } else {
            format!("{current}, {item}")
        };
        if candidate.chars().count() > width && !current.is_empty() {
            lines.push(current);
            current = item.clone();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub(crate) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub(crate) fn new_bench_server(storage_dir: &Path) -> ContextroServer {
    std::fs::create_dir_all(storage_dir).expect("create bench storage dir");

    let settings = contextro_config::Settings {
        storage_dir: storage_dir.to_string_lossy().to_string(),
        ..contextro_config::Settings::default()
    };
    ContextroServer::with_settings(settings)
}

pub(crate) fn temp_storage_dir(name: &str) -> PathBuf {
    temp_path(name)
}

pub(crate) fn temp_output_dir(name: &str) -> String {
    temp_path(name).to_string_lossy().to_string()
}

pub(crate) fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("contextro-bench-{unique}-{name}"))
}
