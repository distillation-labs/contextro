use std::path::Path;
use std::time::Instant;

#[test]
fn bench_index_contextro_source() {
    let target_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    if !target_path.exists() {
        eprintln!("Skipping: {:?} does not exist", target_path);
        return;
    }

    let settings = contextro_config::Settings::default();
    let pipeline = contextro_indexing::IndexingPipeline::new(settings);

    let start = Instant::now();
    let (result, _symbols, _chunks) = pipeline.index(&target_path).unwrap();
    let elapsed = start.elapsed();

    println!("\n=== Rust Indexing Benchmark ===");
    println!("Path: {:?}", target_path);
    println!("Files: {}", result.total_files);
    println!("Symbols: {}", result.total_symbols);
    println!("Chunks: {}", result.total_chunks);
    println!("Time: {:.3}s", elapsed.as_secs_f64());
    println!("Discover: {:.2}ms", result.discover_ms);
    println!("Parse: {:.2}ms", result.parse_ms);
    println!("Chunk: {:.2}ms", result.chunk_ms);
    println!(
        "Files/sec: {:.0}",
        result.total_files as f64 / elapsed.as_secs_f64()
    );
    println!(
        "Symbols/sec: {:.0}",
        result.total_symbols as f64 / elapsed.as_secs_f64()
    );
    println!("===============================\n");

    assert!(result.total_files > 0, "Should find files");
    assert!(result.total_symbols > 0, "Should find symbols");
    assert!(elapsed.as_secs_f64() < 10.0, "Should complete in under 10s");
}
