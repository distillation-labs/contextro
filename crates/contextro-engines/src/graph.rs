//! High-performance code graph with pre-indexed caller/callee lookups
//! and token-based inverted index for sub-millisecond fuzzy search.

use std::collections::{HashMap, HashSet};

use contextro_core::graph::{RelationshipType, UniversalNode, UniversalRelationship};
use parking_lot::RwLock;

/// Thread-safe code graph with O(1) caller/callee lookups and token-indexed fuzzy search.
pub struct CodeGraph {
    inner: RwLock<GraphInner>,
}

struct GraphInner {
    nodes: HashMap<String, UniversalNode>,
    relationships: HashMap<String, UniversalRelationship>,
    // Pre-indexed for O(1) traversal
    callers: HashMap<String, Vec<String>>, // target_id → [caller_node_ids]
    callees: HashMap<String, Vec<String>>, // source_id → [callee_node_ids]
    nodes_by_name: HashMap<String, Vec<String>>, // lowercase name → [node_ids]
    nodes_by_file: HashMap<String, Vec<String>>,
    // Token-based inverted index for fast fuzzy search
    token_index: HashMap<String, Vec<String>>, // token → [node_ids]
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
                token_index: HashMap::new(),
            }),
        }
    }

    pub fn add_node(&self, node: UniversalNode) {
        let mut inner = self.inner.write();
        if inner.nodes.contains_key(&node.id) {
            return;
        }
        inner
            .nodes_by_name
            .entry(node.name.to_lowercase())
            .or_default()
            .push(node.id.clone());
        inner
            .nodes_by_file
            .entry(node.location.file_path.clone())
            .or_default()
            .push(node.id.clone());
        // Index name tokens for fuzzy search
        for token in tokenize_name(&node.name) {
            inner
                .token_index
                .entry(token)
                .or_default()
                .push(node.id.clone());
        }
        inner.nodes.insert(node.id.clone(), node);
    }

    pub fn add_relationship(&self, rel: UniversalRelationship) {
        let mut inner = self.inner.write();
        if !inner.nodes.contains_key(&rel.source_id) || !inner.nodes.contains_key(&rel.target_id) {
            return;
        }
        if rel.relationship_type == RelationshipType::Calls {
            inner
                .callers
                .entry(rel.target_id.clone())
                .or_default()
                .push(rel.source_id.clone());
            inner
                .callees
                .entry(rel.source_id.clone())
                .or_default()
                .push(rel.target_id.clone());
        }
        inner.relationships.insert(rel.id.clone(), rel);
    }

    /// Find nodes by name. Exact uses the name index (O(1)). Fuzzy uses the token index.
    pub fn find_nodes_by_name(&self, name: &str, exact: bool) -> Vec<UniversalNode> {
        let inner = self.inner.read();
        if exact {
            inner
                .nodes_by_name
                .get(&name.to_lowercase())
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| inner.nodes.get(id).cloned())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            if name.is_empty() {
                return inner.nodes.values().cloned().collect();
            }
            // Use token index: find all nodes whose tokens match query tokens
            let query_tokens = tokenize_name(name);
            if query_tokens.is_empty() {
                // Fallback to substring for very short queries
                let lower = name.to_lowercase();
                return inner
                    .nodes
                    .values()
                    .filter(|n| n.name.to_lowercase().contains(&lower))
                    .cloned()
                    .collect();
            }

            // Intersect token matches — nodes that match ALL query tokens
            let mut candidate_ids: Option<HashSet<&String>> = None;
            for token in &query_tokens {
                let matching: HashSet<&String> = inner
                    .token_index
                    .get(token)
                    .map(|ids| ids.iter().collect())
                    .unwrap_or_default();
                candidate_ids = Some(match candidate_ids {
                    Some(existing) => existing.intersection(&matching).cloned().collect(),
                    None => matching,
                });
            }

            let ids = candidate_ids.unwrap_or_default();
            if !ids.is_empty() {
                return ids
                    .iter()
                    .filter_map(|id| inner.nodes.get(*id).cloned())
                    .collect();
            }

            // If token intersection yields nothing, try union (any token matches)
            let mut seen = HashSet::new();
            let mut results = Vec::new();
            for token in &query_tokens {
                if let Some(ids) = inner.token_index.get(token) {
                    for id in ids {
                        if seen.insert(id) {
                            if let Some(node) = inner.nodes.get(id) {
                                results.push(node.clone());
                            }
                        }
                    }
                }
            }
            results
        }
    }

    /// O(1) caller lookup via pre-indexed map.
    pub fn get_callers(&self, node_id: &str) -> Vec<UniversalNode> {
        let inner = self.inner.read();
        inner
            .callers
            .get(node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| inner.nodes.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// O(1) callee lookup via pre-indexed map.
    pub fn get_callees(&self, node_id: &str) -> Vec<UniversalNode> {
        let inner = self.inner.read();
        inner
            .callees
            .get(node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| inner.nodes.get(id).cloned())
                    .collect()
            })
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
        inner.token_index.clear();
    }

    pub fn remove_file_nodes(&self, file_path: &str) {
        let mut inner = self.inner.write();
        let node_ids = inner.nodes_by_file.remove(file_path).unwrap_or_default();
        for id in &node_ids {
            if let Some(node) = inner.nodes.remove(id) {
                if let Some(names) = inner.nodes_by_name.get_mut(&node.name.to_lowercase()) {
                    names.retain(|n| n != id);
                }
                for token in tokenize_name(&node.name) {
                    if let Some(ids) = inner.token_index.get_mut(&token) {
                        ids.retain(|n| n != id);
                    }
                }
            }
            inner.callers.remove(id);
            inner.callees.remove(id);
            for list in inner.callers.values_mut() {
                list.retain(|n| n != id);
            }
            for list in inner.callees.values_mut() {
                list.retain(|n| n != id);
            }
        }
        inner.relationships.retain(|_, rel| {
            !node_ids.contains(&rel.source_id) && !node_ids.contains(&rel.target_id)
        });
    }
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Tokenize a symbol name into searchable tokens.
/// Handles camelCase, PascalCase, snake_case, and kebab-case.
fn tokenize_name(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in name.chars() {
        if ch == '_' || ch == '-' || ch == '.' {
            if current.len() >= 2 {
                tokens.push(current.to_lowercase());
            }
            current.clear();
        } else if ch.is_uppercase() && !current.is_empty() {
            if current.len() >= 2 {
                tokens.push(current.to_lowercase());
            }
            current.clear();
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    if current.len() >= 2 {
        tokens.push(current.to_lowercase());
    }

    // Also add the full lowercase name as a token
    let full = name.to_lowercase();
    if full.len() >= 2 && !tokens.contains(&full) {
        tokens.push(full);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::graph::{NodeType, UniversalLocation};

    fn make_node(id: &str, name: &str) -> UniversalNode {
        UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "test.rs".into(),
                start_line: 1,
                end_line: 10,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            line_count: 10,
            ..Default::default()
        }
    }

    #[test]
    fn test_token_index_fuzzy_search() {
        let graph = CodeGraph::new();
        graph.add_node(make_node("1", "createUser"));
        graph.add_node(make_node("2", "deleteUser"));
        graph.add_node(make_node("3", "authenticate"));

        // Fuzzy search by token
        let results = graph.find_nodes_by_name("user", false);
        assert_eq!(results.len(), 2);

        let results = graph.find_nodes_by_name("create", false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "createUser");
    }

    #[test]
    fn test_tokenize_name() {
        assert_eq!(
            tokenize_name("createUser"),
            vec!["create", "user", "createuser"]
        );
        assert_eq!(
            tokenize_name("find_nodes_by_name"),
            vec!["find", "nodes", "by", "name", "find_nodes_by_name"]
        );
    }
}
