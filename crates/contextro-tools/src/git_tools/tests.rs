use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("contextro-repos-{unique}-{name}"))
}

mod registry;
mod search_a;
mod search_b;
