use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

fn main() {
    let platform_path = Path::new("/Users/japneetkalkat/platform");
    if !platform_path.is_dir() {
        eprintln!("ERROR: /Users/japneetkalkat/platform not found");
        std::process::exit(1);
    }

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  CONTEXTRO RUST MCP — PERFORMANCE BENCHMARK                 ║");
    println!("║  Target: /Users/japneetkalkat/platform                      ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    let settings = contextro_config::Settings::default();
    let pipeline = contextro_indexing::IndexingPipeline::new(settings);

    // ═══ INDEXING ═══
    let start = Instant::now();
    let (result, symbols) = pipeline.index(platform_path).unwrap();
    let idx_time = start.elapsed();

    println!("║  INDEXING                                                    ║");
    println!(
        "║  Files: {:>5}  Symbols: {:>6}  Chunks: {:>6}               ║",
        result.total_files, result.total_symbols, result.total_chunks
    );
    println!(
        "║  Time: {:>8.2}ms                                            ║",
        idx_time.as_secs_f64() * 1000.0
    );
    println!(
        "║  Symbols/sec: {:>10.0}                                      ║",
        result.total_symbols as f64 / idx_time.as_secs_f64()
    );

    // ═══ BM25 INDEX BUILD ═══
    let chunks = contextro_indexing::create_chunks(&symbols);
    let bm25 = contextro_engines::bm25::Bm25Engine::new_in_memory();
    let bm25_start = Instant::now();
    bm25.index_chunks(&chunks);
    let bm25_idx_time = bm25_start.elapsed();
    println!(
        "║  BM25 index: {:>6.1}ms ({} chunks)                          ║",
        bm25_idx_time.as_secs_f64() * 1000.0,
        chunks.len()
    );

    // ═══ GRAPH BUILD ═══
    let graph = contextro_engines::graph::CodeGraph::new();
    let graph_start = Instant::now();
    let mut known: HashMap<String, String> = HashMap::with_capacity(symbols.len());
    for (i, sym) in symbols.iter().enumerate() {
        let node_id = format!("n{}", i);
        known.insert(sym.name.clone(), node_id.clone());
        graph.add_node(contextro_core::UniversalNode {
            id: node_id,
            name: sym.name.clone(),
            node_type: contextro_core::NodeType::Function,
            location: contextro_core::UniversalLocation {
                file_path: sym.filepath.clone(),
                start_line: sym.line_start,
                end_line: sym.line_end,
                start_column: 0,
                end_column: 0,
                language: sym.language.clone(),
            },
            language: sym.language.clone(),
            line_count: sym.line_count(),
            ..Default::default()
        });
    }
    let mut rc = 0;
    for sym in &symbols {
        if let Some(cid) = known.get(&sym.name) {
            for call in &sym.calls {
                if let Some(tid) = known.get(call) {
                    if cid != tid {
                        graph.add_relationship(contextro_core::UniversalRelationship {
                            id: format!("r{}", rc),
                            source_id: cid.clone(),
                            target_id: tid.clone(),
                            relationship_type: contextro_core::RelationshipType::Calls,
                            strength: 1.0,
                        });
                        rc += 1;
                    }
                }
            }
        }
    }
    let graph_time = graph_start.elapsed();
    println!(
        "║  Graph: {:>5} nodes, {:>5} edges ({:.1}ms)                  ║",
        graph.node_count(),
        graph.relationship_count(),
        graph_time.as_secs_f64() * 1000.0
    );

    // ═══ SEARCH LATENCY ═══
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  SEARCH LATENCY (1000 iterations each)                      ║");

    let queries = &[
        ("bm25_narrow", "authentication middleware"),
        ("bm25_broad", "how does the API handle requests"),
        ("bm25_symbol", "IndexingPipeline"),
        ("bm25_exact", "create_user"),
    ];

    // Warm up
    for (_, q) in queries {
        bm25.search(q, 10);
    }

    for (name, query) in queries {
        let mut times = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let start = Instant::now();
            let _ = bm25.search(query, 10);
            times.push(start.elapsed());
        }
        times.sort();
        let avg_us = times
            .iter()
            .map(|t| t.as_nanos() as f64 / 1000.0)
            .sum::<f64>()
            / times.len() as f64;
        let p50 = times[500].as_nanos() as f64 / 1000.0;
        let p99 = times[990].as_nanos() as f64 / 1000.0;
        println!(
            "║  {:15} avg:{:>7.1}µs  p50:{:>6.1}µs  p99:{:>7.1}µs  ║",
            name, avg_us, p50, p99
        );
    }

    // ═══ GRAPH OPERATIONS ═══
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  GRAPH OPERATIONS (100k iterations each)                    ║");

    let graph_ops: Vec<(&str, Box<dyn Fn() -> usize>)> = vec![
        (
            "find_exact",
            Box::new(|| graph.find_nodes_by_name("create_user", true).len()),
        ),
        (
            "find_fuzzy",
            Box::new(|| graph.find_nodes_by_name("auth", false).len()),
        ),
        (
            "get_callers",
            Box::new(|| {
                let m = graph.find_nodes_by_name("create_user", true);
                m.first()
                    .map(|n| graph.get_callers(&n.id).len())
                    .unwrap_or(0)
            }),
        ),
        (
            "get_callees",
            Box::new(|| {
                let m = graph.find_nodes_by_name("create_user", true);
                m.first()
                    .map(|n| graph.get_callees(&n.id).len())
                    .unwrap_or(0)
            }),
        ),
    ];

    for (name, op) in &graph_ops {
        let mut times = Vec::with_capacity(100_000);
        for _ in 0..100_000 {
            let start = Instant::now();
            let _ = op();
            times.push(start.elapsed());
        }
        times.sort();
        let avg_ns = times.iter().map(|t| t.as_nanos() as f64).sum::<f64>() / times.len() as f64;
        let p50 = times[50_000].as_nanos() as f64;
        println!(
            "║  {:15} avg:{:>7.0}ns  p50:{:>6.0}ns                    ║",
            name, avg_ns, p50
        );
    }

    // ═══ CACHE OPERATIONS ═══
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  CACHE OPERATIONS                                           ║");

    let cache = contextro_engines::cache::QueryCache::new(256, 300.0);
    // Populate cache
    for (_, q) in queries {
        let results = bm25.search(q, 10);
        cache.put(q, serde_json::to_value(&results).unwrap_or_default());
    }

    let mut cache_times = Vec::with_capacity(100_000);
    for _ in 0..100_000 {
        let start = Instant::now();
        let _ = cache.get("authentication middleware");
        cache_times.push(start.elapsed());
    }
    cache_times.sort();
    let cache_avg = cache_times
        .iter()
        .map(|t| t.as_nanos() as f64 / 1000.0)
        .sum::<f64>()
        / cache_times.len() as f64;
    let cache_p50 = cache_times[50_000].as_nanos() as f64 / 1000.0;
    println!(
        "║  cache_hit       avg:{:>7.2}µs  p50:{:>6.2}µs                ║",
        cache_avg, cache_p50
    );

    // ═══ THROUGHPUT ═══
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  THROUGHPUT (mixed workload, 5 seconds)                     ║");

    let throughput_start = Instant::now();
    let mut ops_count = 0u64;
    while throughput_start.elapsed().as_secs() < 5 {
        // Mix of operations
        bm25.search("authentication", 10);
        ops_count += 1;
        let _ = graph.find_nodes_by_name("create", false);
        ops_count += 1;
        let _ = cache.get("authentication middleware");
        ops_count += 1;
        bm25.search("database connection", 5);
        ops_count += 1;
    }
    let throughput_time = throughput_start.elapsed();
    let ops_per_sec = ops_count as f64 / throughput_time.as_secs_f64();
    println!(
        "║  {:>10.0} ops/sec ({} ops in {:.1}s)                    ║",
        ops_per_sec,
        ops_count,
        throughput_time.as_secs_f64()
    );

    // ═══ SUMMARY ═══
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  SUMMARY vs TARGETS                                         ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Metric          │ Target    │ Actual    │ Status           ║");
    println!("╟──────────────────┼───────────┼───────────┼──────────────────╢");
    let idx_ms = idx_time.as_secs_f64() * 1000.0;
    let idx_status = if idx_ms <= 5.5 {
        "✓ PASS"
    } else {
        "✗ NEEDS WORK"
    };
    println!(
        "║  Index time      │ ≤5.5ms    │ {:>6.1}ms  │ {:16}║",
        idx_ms, idx_status
    );
    let search_avg = queries
        .iter()
        .map(|(_, q)| {
            let mut t = Vec::new();
            for _ in 0..100 {
                let s = Instant::now();
                bm25.search(q, 10);
                t.push(s.elapsed());
            }
            t.iter().map(|x| x.as_nanos() as f64 / 1000.0).sum::<f64>() / t.len() as f64
        })
        .sum::<f64>()
        / queries.len() as f64;
    let search_status = if search_avg <= 137.0 {
        "✓ PASS"
    } else {
        "✗ NEEDS WORK"
    };
    println!(
        "║  Search latency  │ ≤137µs    │ {:>6.1}µs  │ {:16}║",
        search_avg, search_status
    );
    let tp_status = if ops_per_sec >= 7281.0 {
        "✓ PASS"
    } else {
        "✗ NEEDS WORK"
    };
    println!(
        "║  Throughput      │ ≥7281/s   │ {:>7.0}/s │ {:16}║",
        ops_per_sec, tp_status
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
}
