//! High-performance code graph with pre-indexed caller/callee lookups.

use std::collections::HashMap;

use contextro_core::graph::{NodeType, RelationshipType, UniversalNode, UniversalRelationship};
use parking_lot::RwLock;

/// Thread-safe code graph with O(1) caller/callee lookups.
pub struct CodeGraph {
    inner: RwLock<GraphInner>,
}

struct GraphInner {
    nodes: HashMap<String, UniversalNode>,
    relationships: HashMap<String, UniversalRelationship>,
    // Pre-indexed for O(1) traversal
    callers: HashMap<String, Vec<String>>,   // target_id → [caller_node_ids]
    callees: HashMap<String, Vec<String>>,   // source_id → [callee_node_ids]
    nodes_by_name: HashMap<String, Vec<String>>,
    nodes_by_file: HashMap<String, Vec<String>>,
}

impl CodeGraph {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(GraphInner {
                nodes: HashMap::new(),
                relationships: HashMap::new(),
                callers: HashMap::new(),
                callees: HashMap::new(),
                nodes_by_name: HashMap::new(),
                nodes_by_file: HashMap::new(),
            }),
        }
    }

    pub fn add_node(&self, node: UniversalNode) {
        let mut inner = self.inner.write();
        if inner.nodes.contains_key(&node.id) {
            return;
        }
        inner.nodes_by_name.entry(node.name.to_lowercase()).or_default().push(node.id.clone());
        inner.nodes_by_file.entry(node.location.file_path.clone()).or_default().push(node.id.clone());
        inner.nodes.insert(node.id.clone(), node);
    }

    pub fn add_relationship(&self, rel: UniversalRelationship) {
        let mut inner = self.inner.write();
        if !inner.nodes.contains_key(&rel.source_id) || !inner.nodes.contains_key(&rel.target_id) {
            return;
        }
        if rel.relationship_type == RelationshipType::Calls {
            inner.callers.entry(rel.target_id.clone()).or_default().push(rel.source_id.clone());
            inner.callees.entry(rel.source_id.clone()).or_default().push(rel.target_id.clone());
        }
        inner.relationships.insert(rel.id.clone(), rel);
    }

    pub fn find_nodes_by_name(&self, name: &str, exact: bool) -> Vec<UniversalNode> {
        let inner = self.inner.read();
        if exact {
            inner.nodes_by_name
                .get(&name.to_lowercase())
                .map(|ids| ids.iter().filter_map(|id| inner.nodes.get(id).cloned()).collect())
                .unwrap_or_default()
        } else {
            if name.is_empty() {
                return inner.nodes.values().cloned().collect();
            }
            let lower = name.to_lowercase();
            inner.nodes.values()
                .filter(|n| n.name.to_lowercase().contains(&lower))
                .cloned()
                .collect()
        }
    }

    /// O(1) caller lookup via pre-indexed map.
    pub fn get_callers(&self, node_id: &str) -> Vec<UniversalNode> {
        let inner = self.inner.read();
        inner.callers
            .get(node_id)
            .map(|ids| ids.iter().filter_map(|id| inner.nodes.get(id).cloned()).collect())
            .unwrap_or_default()
    }

    /// O(1) callee lookup via pre-indexed map.
    pub fn get_callees(&self, node_id: &str) -> Vec<UniversalNode> {
        let inner = self.inner.read();
        inner.callees
            .get(node_id)
            .map(|ids| ids.iter().filter_map(|id| inner.nodes.get(id).cloned()).collect())
            .unwrap_or_default()
    }

    pub fn get_node(&self, id: &str) -> Option<UniversalNode> {
        self.inner.read().nodes.get(id).cloned()
    }

    pub fn get_node_degree(&self, node_id: &str) -> (usize, usize) {
        let inner = self.inner.read();
        let in_deg = inner.callers.get(node_id).map(|v| v.len()).unwrap_or(0);
        let out_deg = inner.callees.get(node_id).map(|v| v.len()).unwrap_or(0);
        (in_deg, out_deg)
    }

    pub fn node_count(&self) -> usize {
        self.inner.read().nodes.len()
    }

    pub fn relationship_count(&self) -> usize {
        self.inner.read().relationships.len()
    }

    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.nodes.clear();
        inner.relationships.clear();
        inner.callers.clear();
        inner.callees.clear();
        inner.nodes_by_name.clear();
        inner.nodes_by_file.clear();
    }

    pub fn remove_file_nodes(&self, file_path: &str) {
        let mut inner = self.inner.write();
        let node_ids = inner.nodes_by_file.remove(file_path).unwrap_or_default();
        for id in &node_ids {
            if let Some(node) = inner.nodes.remove(id) {
                if let Some(names) = inner.nodes_by_name.get_mut(&node.name.to_lowercase()) {
                    names.retain(|n| n != id);
                }
            }
            inner.callers.remove(id);
            inner.callees.remove(id);
            // Remove from other nodes' caller/callee lists
            for list in inner.callers.values_mut() {
                list.retain(|n| n != id);
            }
            for list in inner.callees.values_mut() {
                list.retain(|n| n != id);
            }
        }
        inner.relationships.retain(|_, rel| !node_ids.contains(&rel.source_id) && !node_ids.contains(&rel.target_id));
    }
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self::new()
    }
}
