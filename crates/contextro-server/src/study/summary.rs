use super::util::{round1, round3};
use super::*;

pub(super) fn summarize(
    timestamp_unix: u64,
    codebase: &str,
    indexed: &IndexedRepo,
    tasks: &[StudyTask],
    results: &[TaskResult],
) -> StudySummary {
    let tokenizer = TokenizerMeta {
        library: "tiktoken-rs".into(),
        encoding: "cl100k_base".into(),
    };

    let mut arms = BTreeMap::new();
    for arm in ["stronger_local", "contextro"] {
        let arm_results = results
            .iter()
            .filter(|result| result.arm == arm)
            .cloned()
            .collect::<Vec<_>>();
        arms.insert(arm.to_string(), summarize_arm(&arm_results));
    }

    let mut categories = BTreeMap::new();
    for task in tasks {
        *categories.entry(task.category.clone()).or_insert(0) += 1;
    }

    let stronger_total = arms
        .get("stronger_local")
        .map(|summary| summary.tokens.sum)
        .unwrap_or(0.0);
    let contextro_total = arms
        .get("contextro")
        .map(|summary| summary.tokens.sum)
        .unwrap_or(0.0);
    let overall_token_reduction_pct = if stronger_total > 0.0 {
        round1((1.0 - contextro_total / stronger_total) * 100.0)
    } else {
        0.0
    };

    let mut by_category = BTreeMap::new();
    let category_names = categories.keys().cloned().collect::<Vec<_>>();
    for category in category_names {
        let stronger = results
            .iter()
            .filter(|result| result.arm == "stronger_local" && result.category == category)
            .cloned()
            .collect::<Vec<_>>();
        let contextro = results
            .iter()
            .filter(|result| result.arm == "contextro" && result.category == category)
            .cloned()
            .collect::<Vec<_>>();
        let stronger_tokens = stronger
            .iter()
            .map(|result| result.tokens_estimate)
            .sum::<usize>();
        let contextro_tokens = contextro
            .iter()
            .map(|result| result.tokens_estimate)
            .sum::<usize>();
        let reduction = if stronger_tokens > 0 {
            round1((1.0 - contextro_tokens as f64 / stronger_tokens as f64) * 100.0)
        } else {
            0.0
        };
        by_category.insert(
            category.clone(),
            CategorySummary {
                tasks: stronger.len(),
                stronger_local_success_rate: success_rate(&stronger),
                contextro_success_rate: success_rate(&contextro),
                stronger_local_tokens: stronger_tokens,
                contextro_tokens,
                token_reduction_pct: reduction,
            },
        );
    }

    StudySummary {
        timestamp_unix,
        codebase: codebase.to_string(),
        tokenizer,
        index: indexed.index_snapshot.clone(),
        tasks: tasks.len(),
        categories,
        excluded_capabilities: excluded_capabilities(indexed),
        arms,
        overall_token_reduction_pct,
        by_category,
        notes: vec![
            "This study uses scripted deterministic retrieval tasks, not autonomous coding loops."
                .into(),
            "Call-graph-dependent tasks were excluded because the current TypeScript/Javascript parser path emits zero calls on this codebase.".into(),
        ],
    }
}

pub(super) fn build_config(
    timestamp_unix: u64,
    codebase: &str,
    indexed: &IndexedRepo,
    tasks: &[StudyTask],
    tasks_requested: usize,
) -> StudyConfig {
    let mut categories = BTreeMap::new();
    for task in tasks {
        *categories.entry(task.category.clone()).or_insert(0) += 1;
    }

    StudyConfig {
        timestamp_unix,
        codebase: codebase.to_string(),
        tracked_files: indexed.tracked_files,
        tasks_requested,
        tasks_generated: tasks.len(),
        tokenizer: TokenizerMeta {
            library: "tiktoken-rs".into(),
            encoding: "cl100k_base".into(),
        },
        index: indexed.index_snapshot.clone(),
        categories,
        excluded_capabilities: excluded_capabilities(indexed),
        limitations: vec![
            "TypeScript/Javascript graph edges are zero on this repo because the current parser path does not populate calls for those languages.".into(),
            "The no-MCP arm is a stronger local baseline built from exact grep and bounded file reads, not an autonomous agent.".into(),
        ],
    }
}

fn summarize_arm(results: &[TaskResult]) -> ArmSummary {
    ArmSummary {
        completed: results.iter().filter(|result| result.completed).count(),
        successful: results.iter().filter(|result| result.success).count(),
        success_rate: success_rate(results),
        tokens: stats(
            results
                .iter()
                .map(|result| result.tokens_estimate as f64)
                .collect(),
        ),
        latency_ms: stats(results.iter().map(|result| result.wall_clock_ms).collect()),
        tool_calls: stats(
            results
                .iter()
                .map(|result| result.tool_calls as f64)
                .collect(),
        ),
        files_read: stats(
            results
                .iter()
                .map(|result| result.files_read as f64)
                .collect(),
        ),
    }
}

fn stats(mut values: Vec<f64>) -> StatSummary {
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let count = values.len();
    let sum = values.iter().sum::<f64>();
    let mean = if count > 0 { sum / count as f64 } else { 0.0 };
    let median = percentile(&values, 0.50);
    let p95 = percentile(&values, 0.95);
    StatSummary {
        count,
        sum: round3(sum),
        mean: round3(mean),
        median: round3(median),
        p95: round3(p95),
    }
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() - 1) as f64 * p).round() as usize;
    values[index]
}

fn success_rate(results: &[TaskResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    results.iter().filter(|result| result.success).count() as f64 / results.len() as f64
}

fn excluded_capabilities(indexed: &IndexedRepo) -> Vec<String> {
    let mut capabilities = Vec::new();
    if indexed.index_snapshot.graph_relationships == 0 {
        capabilities.push("find_callers".into());
        capabilities.push("find_callees".into());
        capabilities.push("impact".into());
        capabilities.push("relationship-rich explain".into());
    }
    capabilities
}
