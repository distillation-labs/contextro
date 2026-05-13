//! Universal graph data structures for code analysis.
//!
//! Language-agnostic representations for code nodes, relationships,
//! and graph structure with indexed lookups.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

// ─── Node Types ──────────────────────────────────────────────────────────────

/// Universal node types across all languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Module,
    Class,
    Function,
    Variable,
    Parameter,
    Conditional,
    Loop,
    Exception,
    Interface,
    Enum,
    Namespace,
    Import,
    Literal,
    Call,
    Reference,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{:?}", self).to_lowercase());
        write!(f, "{}", s)
    }
}

// ─── Relationship Types ──────────────────────────────────────────────────────

/// Universal relationship types between code elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationshipType {
    Contains,
    Inherits,
    Implements,
    Calls,
    Imports,
    References,
    DependsOn,
    Overrides,
    Extends,
    Uses,
}

// ─── Location ────────────────────────────────────────────────────────────────

/// Location information for code elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UniversalLocation {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(default)]
    pub start_column: u32,
    #[serde(default)]
    pub end_column: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
}

// ─── Node ────────────────────────────────────────────────────────────────────

/// Universal representation of a code element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub location: UniversalLocation,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,
    #[serde(default)]
    pub complexity: u32,
    #[serde(default)]
    pub line_count: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub visibility: String,
    #[serde(default)]
    pub is_static: bool,
    #[serde(default)]
    pub is_abstract: bool,
    #[serde(default)]
    pub is_async: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl Default for UniversalNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: String::new(),
                start_line: 0,
                end_line: 0,
                start_column: 0,
                end_column: 0,
                language: String::new(),
            },
            content: String::new(),
            docstring: None,
            complexity: 0,
            line_count: 0,
            language: String::new(),
            visibility: String::new(),
            is_static: false,
            is_abstract: false,
            is_async: false,
            return_type: None,
            parameter_types: vec![],
            parent: None,
        }
    }
}

// ─── Relationship ────────────────────────────────────────────────────────────

/// Relationship between code elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalRelationship {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: RelationshipType,
    #[serde(default)]
    pub strength: f64,
}

impl Default for UniversalRelationship {
    fn default() -> Self {
        Self {
            id: String::new(),
            source_id: String::new(),
            target_id: String::new(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        }
    }
}

// ─── Graph ───────────────────────────────────────────────────────────────────

/// Universal code graph with indexed lookups.
///
/// Provides O(1) node lookup by ID and efficient secondary indexes
/// by type, language, file, and name.
#[derive(Debug, Clone, Default)]
pub struct UniversalGraph {
    pub nodes: HashMap<String, UniversalNode>,
    pub relationships: HashMap<String, UniversalRelationship>,
    nodes_by_type: HashMap<NodeType, HashSet<String>>,
    nodes_by_language: HashMap<String, HashSet<String>>,
    nodes_by_file: HashMap<String, HashSet<String>>,
    nodes_by_name: HashMap<String, HashSet<String>>,
    rels_from: HashMap<String, HashSet<String>>,
    rels_to: HashMap<String, HashSet<String>>,
}

impl UniversalGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: UniversalNode) {
        let id = node.id.clone();
        self.nodes_by_type
            .entry(node.node_type)
            .or_default()
            .insert(id.clone());
        if !node.language.is_empty() {
            self.nodes_by_language
                .entry(node.language.clone())
                .or_default()
                .insert(id.clone());
        }
        self.nodes_by_file
            .entry(node.location.file_path.clone())
            .or_default()
            .insert(id.clone());
        self.nodes_by_name
            .entry(node.name.to_lowercase())
            .or_default()
            .insert(id.clone());
        self.nodes.insert(id, node);
    }

    pub fn add_relationship(&mut self, rel: UniversalRelationship) {
        let id = rel.id.clone();
        self.rels_from
            .entry(rel.source_id.clone())
            .or_default()
            .insert(id.clone());
        self.rels_to
            .entry(rel.target_id.clone())
            .or_default()
            .insert(id.clone());
        self.relationships.insert(id, rel);
    }

    pub fn get_node(&self, id: &str) -> Option<&UniversalNode> {
        self.nodes.get(id)
    }

    pub fn get_nodes_by_type(&self, node_type: NodeType) -> Vec<&UniversalNode> {
        self.nodes_by_type
            .get(&node_type)
            .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn get_nodes_by_file(&self, file_path: &str) -> Vec<&UniversalNode> {
        self.nodes_by_file
            .get(file_path)
            .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn find_nodes_by_name(&self, name: &str, exact: bool) -> Vec<&UniversalNode> {
        let name_lower = name.to_lowercase();
        if exact {
            self.nodes_by_name
                .get(&name_lower)
                .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
                .unwrap_or_default()
        } else {
            self.nodes
                .values()
                .filter(|n| n.name.to_lowercase().contains(&name_lower))
                .collect()
        }
    }

    pub fn get_relationships_from(&self, node_id: &str) -> Vec<&UniversalRelationship> {
        self.rels_from
            .get(node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.relationships.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_relationships_to(&self, node_id: &str) -> Vec<&UniversalRelationship> {
        self.rels_to
            .get(node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.relationships.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn remove_file_nodes(&mut self, file_path: &str) {
        let node_ids: Vec<String> = self
            .nodes_by_file
            .remove(file_path)
            .unwrap_or_default()
            .into_iter()
            .collect();

        for id in &node_ids {
            if let Some(node) = self.nodes.remove(id) {
                if let Some(set) = self.nodes_by_type.get_mut(&node.node_type) {
                    set.remove(id);
                }
                if let Some(set) = self.nodes_by_language.get_mut(&node.language) {
                    set.remove(id);
                }
                self.nodes_by_name
                    .entry(node.name.to_lowercase())
                    .and_modify(|s| {
                        s.remove(id);
                    });
            }

            // Remove relationships involving this node
            if let Some(rel_ids) = self.rels_from.remove(id) {
                for rel_id in &rel_ids {
                    if let Some(rel) = self.relationships.remove(rel_id) {
                        if let Some(set) = self.rels_to.get_mut(&rel.target_id) {
                            set.remove(rel_id);
                        }
                    }
                }
            }
            if let Some(rel_ids) = self.rels_to.remove(id) {
                for rel_id in &rel_ids {
                    if let Some(rel) = self.relationships.remove(rel_id) {
                        if let Some(set) = self.rels_from.get_mut(&rel.source_id) {
                            set.remove(rel_id);
                        }
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.relationships.clear();
        self.nodes_by_type.clear();
        self.nodes_by_language.clear();
        self.nodes_by_file.clear();
        self.nodes_by_name.clear();
        self.rels_from.clear();
        self.rels_to.clear();
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn relationship_count(&self) -> usize {
        self.relationships.len()
    }

    pub fn file_count(&self) -> usize {
        self.nodes_by_file.len()
    }

    /// Get statistics about the graph.
    pub fn statistics(&self) -> GraphStatistics {
        GraphStatistics {
            total_nodes: self.nodes.len(),
            total_relationships: self.relationships.len(),
            total_files: self.nodes_by_file.len(),
        }
    }
}

/// Summary statistics for a code graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatistics {
    pub total_nodes: usize,
    pub total_relationships: usize,
    pub total_files: usize,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, name: &str, file: &str) -> UniversalNode {
        UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: 1,
                end_line: 10,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            content: String::new(),
            docstring: None,
            complexity: 1,
            line_count: 10,
            language: "python".into(),
            visibility: "public".into(),
            is_static: false,
            is_abstract: false,
            is_async: false,
            return_type: None,
            parameter_types: vec![],
            parent: None,
        }
    }

    #[test]
    fn test_graph_add_and_find() {
        let mut graph = UniversalGraph::new();
        graph.add_node(make_node("n1", "authenticate", "src/auth.py"));
        graph.add_node(make_node("n2", "validate_token", "src/auth.py"));

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.find_nodes_by_name("authenticate", true).len(), 1);
        assert_eq!(graph.find_nodes_by_name("auth", false).len(), 1);
        assert_eq!(graph.get_nodes_by_file("src/auth.py").len(), 2);
    }

    #[test]
    fn test_graph_relationships() {
        let mut graph = UniversalGraph::new();
        graph.add_node(make_node("n1", "caller", "a.py"));
        graph.add_node(make_node("n2", "callee", "b.py"));
        graph.add_relationship(UniversalRelationship {
            id: "r1".into(),
            source_id: "n1".into(),
            target_id: "n2".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        assert_eq!(graph.relationship_count(), 1);
        assert_eq!(graph.get_relationships_from("n1").len(), 1);
        assert_eq!(graph.get_relationships_to("n2").len(), 1);
    }

    #[test]
    fn test_graph_remove_file() {
        let mut graph = UniversalGraph::new();
        graph.add_node(make_node("n1", "foo", "a.py"));
        graph.add_node(make_node("n2", "bar", "b.py"));
        graph.add_relationship(UniversalRelationship {
            id: "r1".into(),
            source_id: "n1".into(),
            target_id: "n2".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        graph.remove_file_nodes("a.py");
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.relationship_count(), 0);
    }
}
