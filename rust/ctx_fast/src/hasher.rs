//! Fast file content hashing using xxHash3.
//!
//! xxHash3 is ~10x faster than SHA-256 and provides excellent distribution
//! for change detection purposes (not cryptographic).

use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use xxhash_rust::xxh3::xxh3_64;

/// Hash a single file's content with xxHash3.
pub fn hash_file_impl(path: &str) -> Result<String, std::io::Error> {
    let content = fs::read(path)?;
    let hash = xxh3_64(&content);
    Ok(format!("{:016x}", hash))
}

/// Hash multiple files in parallel. Skips files that can't be read.
pub fn hash_files_parallel(paths: &[String]) -> HashMap<String, String> {
    paths
        .par_iter()
        .filter_map(|path| {
            hash_file_impl(path)
                .ok()
                .map(|hash| (path.clone(), hash))
        })
        .collect()
}
