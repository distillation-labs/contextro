use super::*;
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::NodeType;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("contextro-artifacts-{unique}-{name}"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

mod audit;
mod docs;
mod sidecar;
