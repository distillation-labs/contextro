"""High-performance code graph using rustworkx.

Thread-safe directed graph with advanced traversal algorithms.
"""

import logging
import re
import threading
from collections import defaultdict
from typing import Any, Dict, List, Optional, Set

import rustworkx as rx

from contextro_mcp.core.graph_models import (
    NodeType,
    RelationshipType,
    UniversalNode,
    UniversalRelationship,
)

logger = logging.getLogger(__name__)
TOKEN_RE = re.compile(r"[A-Za-z0-9]+")
CAMEL_BOUNDARY_RE = re.compile(r"([a-z0-9])([A-Z])")


class RustworkxCodeGraph:
    """Thread-safe code graph using rustworkx PyDiGraph."""

    def __init__(self):
        self._lock = threading.RLock()
        self.graph = rx.PyDiGraph()
        self.nodes: Dict[str, UniversalNode] = {}
        self.relationships: Dict[str, UniversalRelationship] = {}

        # Node ID → rustworkx index
        self._id_to_index: Dict[str, int] = {}
        self._index_to_id: Dict[int, str] = {}

        # Performance indexes (defaultdict avoids per-key existence checks)
        self._nodes_by_type: Dict[NodeType, Set[str]] = defaultdict(set)
        self._nodes_by_language: Dict[str, Set[str]] = defaultdict(set)
        self._file_nodes: Dict[str, Set[str]] = defaultdict(set)
        self._nodes_by_name: Dict[str, Set[str]] = defaultdict(set)
        self._nodes_by_name_token: Dict[str, Set[str]] = defaultdict(set)

    def add_node(self, node: UniversalNode) -> int:
        """Add node to graph. Returns rustworkx index."""
        with self._lock:
            if node.id in self._id_to_index:
                return self._id_to_index[node.id]

            idx = self.graph.add_node(node.id)
            self._id_to_index[node.id] = idx
            self._index_to_id[idx] = node.id
            self.nodes[node.id] = node

            # Update indexes
            self._nodes_by_type[node.node_type].add(node.id)
            self._nodes_by_name[node.name.lower()].add(node.id)
            for token in self._name_tokens(node.name):
                self._nodes_by_name_token[token].add(node.id)

            if node.language:
                self._nodes_by_language[node.language].add(node.id)

            if node.location:
                self._file_nodes[node.location.file_path].add(node.id)

            return idx

    def add_relationship(self, rel: UniversalRelationship) -> Optional[int]:
        """Add relationship (edge) to graph."""
        with self._lock:
            src_idx = self._id_to_index.get(rel.source_id)
            tgt_idx = self._id_to_index.get(rel.target_id)

            if src_idx is None or tgt_idx is None:
                logger.debug("Skipping edge: missing node (%s -> %s)", rel.source_id, rel.target_id)
                return None

            edge_idx = self.graph.add_edge(src_idx, tgt_idx, rel.id)
            self.relationships[rel.id] = rel
            return edge_idx

    def get_node(self, node_id: str) -> Optional[UniversalNode]:
        """Return node by ID, or None if not found."""
        with self._lock:
            return self.nodes.get(node_id)

    def get_nodes_by_type(self, node_type: NodeType) -> List[UniversalNode]:
        """Return all nodes of the given type."""
        with self._lock:
            ids = self._nodes_by_type.get(node_type, set())
            return self._sorted_nodes(ids)

    def get_nodes_by_language(self, language: str) -> List[UniversalNode]:
        """Return all nodes for the given language."""
        with self._lock:
            ids = self._nodes_by_language.get(language, set())
            return self._sorted_nodes(ids)

    def find_nodes_by_name(self, name: str, exact: bool = True) -> List[UniversalNode]:
        """Find nodes by name. If exact is False, uses case-insensitive substring match."""
        with self._lock:
            name_lower = name.lower()
            if exact:
                ids = self._nodes_by_name.get(name_lower, set())
                return self._sorted_nodes(ids)
            candidate_ids = set(self._nodes_by_name.get(name_lower, set()))
            candidate_ids.update(self._nodes_by_name_token.get(name_lower, set()))
            if candidate_ids:
                return self._sorted_nodes(candidate_ids)
            return sorted(
                [node for node in self.nodes.values() if name_lower in node.name.lower()],
                key=self._node_sort_key,
            )

    def _get_callers_unlocked(self, node_id: str) -> List[UniversalNode]:
        """Get callers without acquiring lock. Caller must hold self._lock."""
        idx = self._id_to_index.get(node_id)
        if idx is None:
            return []
        callers = []
        for pred_idx in self.graph.predecessor_indices(idx):
            pred_id = self._index_to_id.get(pred_idx)
            if pred_id and pred_id in self.nodes:
                edge_data = self.graph.get_edge_data(pred_idx, idx)
                if edge_data and edge_data in self.relationships:
                    rel = self.relationships[edge_data]
                    if rel.relationship_type == RelationshipType.CALLS:
                        callers.append(self.nodes[pred_id])
        return sorted(callers, key=self._node_sort_key)

    def get_callers(self, node_id: str) -> List[UniversalNode]:
        """Get nodes that call the given node (predecessors via CALLS edges)."""
        with self._lock:
            return self._get_callers_unlocked(node_id)

    def _get_callees_unlocked(self, node_id: str) -> List[UniversalNode]:
        """Get callees without acquiring lock. Caller must hold self._lock."""
        idx = self._id_to_index.get(node_id)
        if idx is None:
            return []
        callees = []
        for succ_idx in self.graph.successor_indices(idx):
            succ_id = self._index_to_id.get(succ_idx)
            if succ_id and succ_id in self.nodes:
                edge_data = self.graph.get_edge_data(idx, succ_idx)
                if edge_data and edge_data in self.relationships:
                    rel = self.relationships[edge_data]
                    if rel.relationship_type == RelationshipType.CALLS:
                        callees.append(self.nodes[succ_id])
        return sorted(callees, key=self._node_sort_key)

    def get_callees(self, node_id: str) -> List[UniversalNode]:
        """Get nodes called by the given node (successors via CALLS edges)."""
        with self._lock:
            return self._get_callees_unlocked(node_id)

    def get_predecessors(self, node_id: str) -> List[UniversalNode]:
        """Get all predecessor nodes."""
        with self._lock:
            idx = self._id_to_index.get(node_id)
            if idx is None:
                return []
            result = []
            for pred_idx in self.graph.predecessor_indices(idx):
                pred_id = self._index_to_id.get(pred_idx)
                if pred_id and pred_id in self.nodes:
                    result.append(self.nodes[pred_id])
            return sorted(result, key=self._node_sort_key)

    def get_successors(self, node_id: str) -> List[UniversalNode]:
        """Get all successor nodes."""
        with self._lock:
            idx = self._id_to_index.get(node_id)
            if idx is None:
                return []
            result = []
            for succ_idx in self.graph.successor_indices(idx):
                succ_id = self._index_to_id.get(succ_idx)
                if succ_id and succ_id in self.nodes:
                    result.append(self.nodes[succ_id])
            return sorted(result, key=self._node_sort_key)

    def get_transitive_callers(self, node_id: str, max_depth: int = 10) -> List[UniversalNode]:
        """Get transitive closure of callers (for impact analysis)."""
        with self._lock:
            visited: Set[str] = set()
            result: List[UniversalNode] = []
            queue = [node_id]
            depth = 0

            while queue and depth < max_depth:
                next_queue = []
                for nid in queue:
                    if nid in visited:
                        continue
                    visited.add(nid)
                    callers = self._get_callers_unlocked(nid)
                    for caller in callers:
                        if caller.id not in visited:
                            result.append(caller)
                            next_queue.append(caller.id)
                queue = next_queue
                depth += 1

            return result

    def get_relationships_from(self, node_id: str) -> List[UniversalRelationship]:
        """Return outgoing relationships from the given node."""
        with self._lock:
            idx = self._id_to_index.get(node_id)
            if idx is None:
                return []
            result = []
            for succ_idx in self.graph.successor_indices(idx):
                edge_data = self.graph.get_edge_data(idx, succ_idx)
                if edge_data and edge_data in self.relationships:
                    result.append(self.relationships[edge_data])
            return result

    def get_relationships_to(self, node_id: str) -> List[UniversalRelationship]:
        """Return incoming relationships to the given node."""
        with self._lock:
            idx = self._id_to_index.get(node_id)
            if idx is None:
                return []
            result = []
            for pred_idx in self.graph.predecessor_indices(idx):
                edge_data = self.graph.get_edge_data(pred_idx, idx)
                if edge_data and edge_data in self.relationships:
                    result.append(self.relationships[edge_data])
            return result

    def get_relationships_by_type(self, rel_type: RelationshipType) -> List[UniversalRelationship]:
        """Return all relationships of the given type."""
        with self._lock:
            return [r for r in self.relationships.values() if r.relationship_type == rel_type]

    def remove_file_nodes(self, file_path: str) -> int:
        """Remove all nodes from a specific file (for incremental reindex)."""
        with self._lock:
            node_ids = self._file_nodes.pop(file_path, set())
            # Clean up relationships referencing removed nodes
            stale_rels = [
                rid
                for rid, rel in self.relationships.items()
                if rel.source_id in node_ids or rel.target_id in node_ids
            ]
            for rid in stale_rels:
                del self.relationships[rid]

            for nid in node_ids:
                node = self.nodes.get(nid)
                idx = self._id_to_index.pop(nid, None)
                if idx is not None:
                    self._index_to_id.pop(idx, None)
                    try:
                        self.graph.remove_node(idx)
                    except Exception as e:
                        logger.debug("Failed to remove node %s from file mapping: %s", nid, e)
                        pass
                self.nodes.pop(nid, None)
                if node is not None:
                    self._nodes_by_name[node.name.lower()].discard(nid)
                    for token in self._name_tokens(node.name):
                        self._nodes_by_name_token[token].discard(nid)
                for type_set in self._nodes_by_type.values():
                    type_set.discard(nid)
                for lang_set in self._nodes_by_language.values():
                    lang_set.discard(nid)
            return len(node_ids)

    def get_node_degree(self, node_id: str) -> tuple:
        """Return (in_degree, out_degree) for a node, or (0, 0) if not found."""
        with self._lock:
            idx = self._id_to_index.get(node_id)
            if idx is None:
                return (0, 0)
            return (self.graph.in_degree(idx), self.graph.out_degree(idx))

    def get_statistics(self) -> Dict[str, Any]:
        """Return graph statistics: node/relationship counts, type/language breakdowns."""
        with self._lock:
            stats: Dict[str, Any] = {
                "total_nodes": len(self.nodes),
                "total_relationships": len(self.relationships),
                "total_files": len(self._file_nodes),
                "nodes_by_type": {nt.value: len(ids) for nt, ids in self._nodes_by_type.items()},
                "nodes_by_language": {
                    lang: len(ids) for lang, ids in self._nodes_by_language.items()
                },
            }
            return stats

    def get_nodes_for_file(self, file_path: str) -> List[UniversalNode]:
        """Return all nodes that belong to a specific file."""
        with self._lock:
            ids = self._file_nodes.get(file_path, set())
            return self._sorted_nodes(ids)

    def get_related_files(
        self,
        file_path: str,
        *,
        relationship_types: Optional[Set[RelationshipType]] = None,
        reverse: bool = False,
    ) -> List[str]:
        """Return neighboring file paths connected through filtered relationships."""
        with self._lock:
            node_ids = self._file_nodes.get(file_path, set())
            related: Set[str] = set()
            for node_id in node_ids:
                relationships = (
                    self.get_relationships_to(node_id)
                    if reverse
                    else self.get_relationships_from(node_id)
                )
                for rel in relationships:
                    if relationship_types and rel.relationship_type not in relationship_types:
                        continue
                    other_id = rel.source_id if reverse else rel.target_id
                    other_node = self.nodes.get(other_id)
                    if other_node and other_node.location.file_path != file_path:
                        related.add(other_node.location.file_path)
            return sorted(related)

    def get_reachable_nodes(
        self,
        start_ids: List[str],
        *,
        relationship_types: Optional[Set[RelationshipType]] = None,
        reverse: bool = False,
        max_depth: Optional[int] = None,
    ) -> List[UniversalNode]:
        """Return the transitive closure of nodes reachable from a seed set."""
        with self._lock:
            seen: Set[str] = set()
            ordered: List[str] = []
            frontier = [node_id for node_id in start_ids if node_id in self.nodes]
            depth = 0
            while frontier and (max_depth is None or depth <= max_depth):
                next_frontier: List[str] = []
                for node_id in frontier:
                    if node_id in seen or node_id not in self.nodes:
                        continue
                    seen.add(node_id)
                    ordered.append(node_id)
                    for neighbor in self._neighbor_ids_unlocked(
                        node_id,
                        relationship_types=relationship_types,
                        reverse=reverse,
                    ):
                        if neighbor not in seen and neighbor not in next_frontier:
                            next_frontier.append(neighbor)
                frontier = next_frontier
                depth += 1
            return [self.nodes[node_id] for node_id in ordered if node_id in self.nodes]

    def get_strongly_connected_components(
        self,
        relationship_type: Optional[RelationshipType] = None,
    ) -> List[List[UniversalNode]]:
        """Return SCCs over the graph, optionally filtered by relationship type."""
        with self._lock:
            adjacency: Dict[str, Set[str]] = defaultdict(set)
            for rel in self.relationships.values():
                if relationship_type and rel.relationship_type != relationship_type:
                    continue
                if rel.source_id in self.nodes and rel.target_id in self.nodes:
                    adjacency[rel.source_id].add(rel.target_id)

            index = 0
            indices: Dict[str, int] = {}
            lowlinks: Dict[str, int] = {}
            stack: List[str] = []
            on_stack: Set[str] = set()
            components: List[List[UniversalNode]] = []

            def strongconnect(node_id: str) -> None:
                nonlocal index
                indices[node_id] = index
                lowlinks[node_id] = index
                index += 1
                stack.append(node_id)
                on_stack.add(node_id)

                for neighbor in adjacency.get(node_id, []):
                    if neighbor not in indices:
                        strongconnect(neighbor)
                        lowlinks[node_id] = min(lowlinks[node_id], lowlinks[neighbor])
                    elif neighbor in on_stack:
                        lowlinks[node_id] = min(lowlinks[node_id], indices[neighbor])

                if lowlinks[node_id] == indices[node_id]:
                    component_ids: List[str] = []
                    while stack:
                        member = stack.pop()
                        on_stack.discard(member)
                        component_ids.append(member)
                        if member == node_id:
                            break
                    has_self_loop = (
                        len(component_ids) == 1 and node_id in adjacency.get(node_id, set())
                    )
                    if len(component_ids) > 1 or has_self_loop:
                        components.append(self._sorted_nodes(component_ids))

            for node_id in sorted(self.nodes, key=lambda nid: self._node_sort_key(self.nodes[nid])):
                if node_id not in indices:
                    strongconnect(node_id)
            return sorted(components, key=lambda comp: (-len(comp), [node.id for node in comp]))

    def clear(self) -> None:
        """Remove all nodes, relationships, and indexes."""
        with self._lock:
            self.graph = rx.PyDiGraph()
            self.nodes.clear()
            self.relationships.clear()
            self._id_to_index.clear()
            self._index_to_id.clear()
            self._nodes_by_type = defaultdict(set)
            self._nodes_by_language = defaultdict(set)
            self._file_nodes = defaultdict(set)
            self._nodes_by_name = defaultdict(set)
            self._nodes_by_name_token = defaultdict(set)

    def _neighbor_ids_unlocked(
        self,
        node_id: str,
        *,
        relationship_types: Optional[Set[RelationshipType]] = None,
        reverse: bool = False,
    ) -> List[str]:
        relationships = (
            self.get_relationships_to(node_id)
            if reverse
            else self.get_relationships_from(node_id)
        )
        neighbor_ids = []
        for rel in relationships:
            if relationship_types and rel.relationship_type not in relationship_types:
                continue
            neighbor_ids.append(rel.source_id if reverse else rel.target_id)
        return [node.id for node in self._sorted_nodes(set(neighbor_ids))]

    def _sorted_nodes(self, node_ids: Set[str] | List[str]) -> List[UniversalNode]:
        return sorted(
            [self.nodes[node_id] for node_id in node_ids if node_id in self.nodes],
            key=self._node_sort_key,
        )

    @staticmethod
    def _node_sort_key(node: UniversalNode) -> tuple[str, int, int, str, str]:
        return (
            node.location.file_path,
            node.location.start_line,
            node.location.start_column,
            node.name,
            node.id,
        )

    @staticmethod
    def _name_tokens(name: str) -> Set[str]:
        normalized = CAMEL_BOUNDARY_RE.sub(r"\1 \2", name.replace("_", " "))
        return {
            token.lower()
            for token in TOKEN_RE.findall(normalized)
            if len(token) > 1 and not token.isdigit()
        }
