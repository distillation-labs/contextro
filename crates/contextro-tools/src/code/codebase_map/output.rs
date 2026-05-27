use super::*;

pub(crate) fn build_codebase_map_response(
    mut hits: Vec<CodebaseMapHit>,
    graph: &CodeGraph,
    normalized_query: &str,
    query_tokens: &[String],
    narrow_explanatory_query: bool,
    targets_product_surface: bool,
    raw_query: &str,
    resolved_filter: Option<&Path>,
    codebase: Option<&str>,
) -> Value {
    if !normalized_query.is_empty() {
        let top_score = hits.first().map(|hit| hit.score).unwrap_or(0.0);
        let min_score = (top_score * 0.45).max(0.5);
        hits.retain(|hit| hit.score >= min_score);
    }

    let mut grouped: Vec<(String, Vec<CodebaseMapHit>, f64)> = Vec::new();
    for hit in hits {
        if let Some((_, symbols, top_score)) =
            grouped.iter_mut().find(|(file, _, _)| *file == hit.file)
        {
            *top_score = top_score.max(hit.score);
            symbols.push(hit);
        } else {
            let top_score = hit.score;
            grouped.push((hit.file.clone(), vec![hit], top_score));
        }
    }

    if narrow_explanatory_query {
        for (_, symbols, file_score) in &mut grouped {
            *file_score = codebase_map_narrow_file_relevance_score(
                symbols,
                graph,
                normalized_query,
                query_tokens,
                targets_product_surface,
            );
        }
    }

    if normalized_query.is_empty() {
        grouped.sort_by(|a, b| a.0.cmp(&b.0));
    } else {
        grouped.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        if narrow_explanatory_query && grouped.len() > 1 {
            let lead_score = grouped[0].2;
            let second_score = grouped[1].2;
            let max_files = if lead_score >= second_score + 0.35 {
                1
            } else {
                2
            };
            let retain_floor = if max_files == 1 {
                lead_score - 0.01
            } else {
                (lead_score - 0.18).max(0.5)
            };

            grouped.retain(|(_, _, top_score)| *top_score >= retain_floor);
            grouped.truncate(max_files);
        }
    }

    let files: Vec<Value> = grouped
        .into_iter()
        .take(if normalized_query.is_empty() {
            usize::MAX
        } else {
            5
        })
        .map(|(file, mut symbols, _)| {
            if normalized_query.is_empty() {
                symbols.sort_by(|a, b| {
                    a.symbol["line"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                });
            } else {
                symbols.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| {
                            a.symbol["line"]
                                .as_u64()
                                .unwrap_or(0)
                                .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                        })
                });
                symbols.truncate(8);
            }

            let total = symbols.len();
            json!({
                "file": file,
                "symbols": symbols.into_iter().map(|hit| hit.symbol).collect::<Vec<_>>(),
                "total": total
            })
        })
        .collect();

    let total_symbols: usize = files
        .iter()
        .map(|f| f["total"].as_u64().unwrap_or(0) as usize)
        .sum();

    json!({
        "path": if let Some(path) = resolved_filter {
            json!(strip_base(&path.to_string_lossy(), codebase))
        } else {
            json!(".")
        },
        "query": if raw_query.is_empty() {
            Value::Null
        } else {
            json!(raw_query)
        },
        "files": files,
        "total_files": files.len(),
        "total_symbols": total_symbols,
    })
}
