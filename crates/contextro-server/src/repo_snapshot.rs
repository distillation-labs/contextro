use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use contextro_core::models::{CodeChunk, Symbol};
use contextro_engines::graph::GraphSnapshot;

use super::state::RepoScopeSnapshot;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedChunkVector {
    id: String,
    #[serde(default)]
    vector: Vec<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PersistedRepoSnapshot {
    #[serde(default)]
    symbols: Vec<Symbol>,
    #[serde(default)]
    chunks: Vec<CodeChunk>,
    #[serde(default)]
    vectors: Vec<PersistedChunkVector>,
}

pub(crate) fn load_repo_snapshot(path: &Path) -> Option<RepoScopeSnapshot> {
    let persisted = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PersistedRepoSnapshot>(&bytes).ok())?;
    let PersistedRepoSnapshot {
        symbols,
        chunks,
        vectors,
    } = persisted;
    let mut chunks = if chunks.is_empty() {
        contextro_indexing::create_chunks(&symbols)
    } else {
        chunks
    };
    let mut vectors_by_id = vectors
        .into_iter()
        .map(|entry| (entry.id, entry.vector))
        .collect::<HashMap<_, _>>();
    for chunk in &mut chunks {
        if let Some(vector) = vectors_by_id.remove(&chunk.id) {
            chunk.vector = vector;
        }
    }
    Some(RepoScopeSnapshot {
        symbols,
        chunks,
        graph: GraphSnapshot::default(),
    })
}

pub(crate) fn save_repo_snapshot(path: &Path, snapshot: &RepoScopeSnapshot) {
    #[derive(Serialize)]
    struct PersistedChunkVector<'a> {
        id: &'a str,
        vector: &'a [f32],
    }

    #[derive(Serialize)]
    struct PersistedRepoScopeSnapshot<'a> {
        symbols: &'a [Symbol],
        #[serde(skip_serializing_if = "Vec::is_empty")]
        vectors: Vec<PersistedChunkVector<'a>>,
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("json.tmp");
    if let Ok(file) = std::fs::File::create(&tmp_path) {
        let mut writer = std::io::BufWriter::new(file);
        let vectors = snapshot
            .chunks
            .iter()
            .filter(|chunk| !chunk.vector.is_empty())
            .map(|chunk| PersistedChunkVector {
                id: &chunk.id,
                vector: &chunk.vector,
            })
            .collect();
        let persisted = PersistedRepoScopeSnapshot {
            symbols: &snapshot.symbols,
            vectors,
        };
        use std::io::Write;
        if serde_json::to_writer(&mut writer, &persisted).is_ok() && writer.flush().is_ok() {
            let _ = std::fs::rename(&tmp_path, path);
        }
    }
}
