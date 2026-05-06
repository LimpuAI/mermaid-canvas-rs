//! Rank assignment and node ordering
//!
//! Implements the ranking phase of the Sugiyama algorithm:
//! 1. Assign ranks (layers) using longest path from sources
//! 2. Order nodes within ranks using barycenter heuristic

use std::collections::{HashMap, HashSet, VecDeque};
use mermaid_canvas_core::{DiagramAst, DiagramEdge};

/// Assign ranks (layers) to nodes using longest path from sources.
///
/// Returns `(ranks, rank_map)` where:
/// - `ranks[i]` is the list of node IDs at rank i
/// - `rank_map[node_id]` is the rank index of that node
pub fn assign_ranks(ast: &DiagramAst) -> (Vec<Vec<String>>, HashMap<String, usize>) {
    let node_ids: Vec<&String> = ast.node_order.iter()
        .filter(|id| ast.nodes.contains_key(*id))
        .collect();

    if node_ids.is_empty() {
        return (Vec::new(), HashMap::new());
    }

    // Build adjacency info
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut predecessors: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut successors: HashMap<&str, Vec<&str>> = HashMap::new();

    for id in &node_ids {
        let key = id.as_str();
        in_degree.entry(key).or_insert(0);
        predecessors.entry(key).or_insert_with(Vec::new);
        successors.entry(key).or_insert_with(Vec::new);
    }

    // Collect all valid node IDs
    let valid_ids: HashSet<&str> = node_ids.iter().map(|s| s.as_str()).collect();

    for edge in &ast.edges {
        let from = edge.from.as_str();
        let to = edge.to.as_str();
        if valid_ids.contains(from) && valid_ids.contains(to) && from != to {
            *in_degree.entry(to).or_insert(0) += 1;
            predecessors.entry(to).or_insert_with(Vec::new).push(from);
            successors.entry(from).or_insert_with(Vec::new).push(to);
        }
    }

    // Topological sort with longest-path ranking
    let mut rank_map: HashMap<String, usize> = HashMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    // Find all source nodes (in_degree == 0)
    for (&id, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(id);
            rank_map.insert(id.to_string(), 0);
        }
    }

    // If no sources found (pure cycles), pick first node in order as source
    if queue.is_empty() {
        if let Some(first) = node_ids.first() {
            queue.push_back(first.as_str());
            rank_map.insert((*first).clone(), 0);
        }
    }

    // BFS to assign ranks — use longest path from sources
    let mut processed: HashSet<&str> = HashSet::new();
    while let Some(node) = queue.pop_front() {
        if processed.contains(node) {
            continue;
        }

        // Check all predecessors are ranked
        let preds_done = predecessors.get(node)
            .map(|ps| ps.iter().all(|p| rank_map.contains_key(*p)))
            .unwrap_or(true);

        if !preds_done && !rank_map.contains_key(node) {
            // Re-queue for later
            queue.push_back(node);
            continue;
        }

        processed.insert(node);

        // Assign rank = max(predecessor ranks) + 1 (if not already assigned as source)
        if !rank_map.contains_key(node) {
            let max_pred_rank = predecessors.get(node)
                .map(|ps| {
                    ps.iter()
                        .filter_map(|p| rank_map.get(*p).copied())
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            rank_map.insert(node.to_string(), max_pred_rank + 1);
        }

        // Process successors
        if let Some(succs) = successors.get(node) {
            for &succ in succs {
                let current_rank = rank_map.get(succ).copied().unwrap_or(usize::MAX);
                let new_rank = rank_map[node] + 1;
                if new_rank < current_rank {
                    // This shouldn't happen in DAG, but handle gracefully
                }
                if current_rank == usize::MAX {
                    queue.push_back(succ);
                } else {
                    // Already ranked; might need to update for longest path
                    let succ_rank = rank_map.get(succ).copied().unwrap_or(0);
                    let needed_rank = rank_map[node] + 1;
                    if needed_rank > succ_rank {
                        rank_map.insert(succ.to_string(), needed_rank);
                    }
                }
            }
        }
    }

    // Handle any unranked nodes (isolated or in cycles)
    let order_map: HashMap<&str, usize> = node_ids.iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    for id in &node_ids {
        let key = id.as_str();
        if !rank_map.contains_key(*id) {
            // Assign rank based on order as fallback
            rank_map.insert((*id).clone(), order_map[key]);
        }
    }

    // Build ranks structure
    let max_rank = rank_map.values().copied().max().unwrap_or(0);
    let mut ranks: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];
    for id in &node_ids {
        if let Some(&rank) = rank_map.get(*id) {
            ranks[rank].push((*id).clone());
        }
    }

    // Remove empty ranks (shouldn't happen, but be safe)
    ranks.retain(|r| !r.is_empty());

    // Re-index rank_map after removing empty ranks
    let mut new_rank_map = HashMap::new();
    for (rank_idx, rank_nodes) in ranks.iter().enumerate() {
        for node_id in rank_nodes {
            new_rank_map.insert(node_id.clone(), rank_idx);
        }
    }

    (ranks, new_rank_map)
}

/// Order nodes within ranks using barycenter heuristic.
///
/// Multiple passes to reduce edge crossings.
pub fn order_nodes(
    ranks: &mut [Vec<String>],
    edges: &[DiagramEdge],
    passes: usize,
    _rank_map: &HashMap<String, usize>,
) {
    if ranks.len() <= 1 {
        return;
    }

    // Build predecessor map for quick lookup
    let mut pred_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut succ_map: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges {
        if edge.from != edge.to {
            pred_map.entry(edge.to.clone()).or_default().push(edge.from.clone());
            succ_map.entry(edge.from.clone()).or_default().push(edge.to.clone());
        }
    }

    for pass in 0..passes {
        // Alternate between top-down and bottom-up passes
        let top_down = pass % 2 == 0;

        if top_down {
            // Top-down: for each rank (except first), order by barycenter of predecessors
            // Clone positions to avoid borrow conflicts
            let prev_positions_map: Vec<HashMap<String, f64>> = ranks.iter()
                .map(|rank| {
                    rank.iter()
                        .enumerate()
                        .map(|(i, id)| (id.clone(), i as f64))
                        .collect()
                })
                .collect();

            for rank_idx in 1..ranks.len() {
                let pos: HashMap<&str, f64> = prev_positions_map[rank_idx - 1]
                    .iter()
                    .map(|(k, &v)| (k.as_str(), v))
                    .collect();
                sort_by_barycenter(&mut ranks[rank_idx], &pred_map, &pos);
            }
        } else {
            // Bottom-up: for each rank (except last), order by barycenter of successors
            let next_positions_map: Vec<HashMap<String, f64>> = ranks.iter()
                .map(|rank| {
                    rank.iter()
                        .enumerate()
                        .map(|(i, id)| (id.clone(), i as f64))
                        .collect()
                })
                .collect();

            for rank_idx in (0..ranks.len() - 1).rev() {
                let pos: HashMap<&str, f64> = next_positions_map[rank_idx + 1]
                    .iter()
                    .map(|(k, &v)| (k.as_str(), v))
                    .collect();
                sort_by_barycenter(&mut ranks[rank_idx], &succ_map, &pos);
            }
        }
    }
}

/// Sort nodes in a rank by the barycenter of their neighbors' positions
fn sort_by_barycenter(
    rank_nodes: &mut [String],
    neighbor_map: &HashMap<String, Vec<String>>,
    positions: &HashMap<&str, f64>,
) {
    if rank_nodes.len() <= 1 {
        return;
    }

    // Compute barycenter for each node
    let mut node_bary: Vec<(String, f64)> = rank_nodes.iter()
        .map(|id| {
            let bary = if let Some(neighbors) = neighbor_map.get(id) {
                let valid_positions: Vec<f64> = neighbors.iter()
                    .filter_map(|n| positions.get(n.as_str()).copied())
                    .collect();
                if valid_positions.is_empty() {
                    // No connected neighbors — keep current position
                    let idx = rank_nodes.iter().position(|x| x == id).unwrap_or(0);
                    idx as f64
                } else {
                    valid_positions.iter().sum::<f64>() / valid_positions.len() as f64
                }
            } else {
                let idx = rank_nodes.iter().position(|x| x == id).unwrap_or(0);
                idx as f64
            };
            (id.clone(), bary)
        })
        .collect();

    // Stable sort by barycenter
    node_bary.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Update the rank in place
    for (i, (id, _)) in node_bary.into_iter().enumerate() {
        rank_nodes[i] = id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mermaid_canvas_core::{
        DiagramAst, DiagramKind, DiagramNode, DiagramEdge, NodeShape, EdgeStyle,
    };
    use mermaid_canvas_core::diagram::NodeStyle;

    fn make_node(id: &str, label: &str) -> DiagramNode {
        DiagramNode {
            id: id.to_string(),
            label: label.to_string(),
            shape: NodeShape::RoundRect,
            style: NodeStyle::default(),
            link: None,
            subgraph: None,
        }
    }

    fn make_edge(from: &str, to: &str) -> DiagramEdge {
        DiagramEdge {
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            start_label: None,
            end_label: None,
            directed: true,
            arrow_start: None,
            arrow_end: None,
            start_decoration: None,
            end_decoration: None,
            style: EdgeStyle::Solid,
        }
    }

    #[test]
    fn test_empty_ast() {
        let ast = DiagramAst::new(DiagramKind::Flowchart);
        let (ranks, rank_map) = assign_ranks(&ast);
        assert!(ranks.is_empty());
        assert!(rank_map.is_empty());
    }

    #[test]
    fn test_single_node() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.add_node(make_node("A", "Node A"));
        let (ranks, rank_map) = assign_ranks(&ast);
        assert_eq!(ranks.len(), 1);
        assert_eq!(ranks[0], vec!["A"]);
        assert_eq!(rank_map["A"], 0);
    }

    #[test]
    fn test_linear_chain() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.add_node(make_node("A", "A"));
        ast.add_node(make_node("B", "B"));
        ast.add_node(make_node("C", "C"));
        ast.add_edge(make_edge("A", "B"));
        ast.add_edge(make_edge("B", "C"));

        let (ranks, rank_map) = assign_ranks(&ast);
        assert_eq!(ranks.len(), 3);
        assert_eq!(rank_map["A"], 0);
        assert_eq!(rank_map["B"], 1);
        assert_eq!(rank_map["C"], 2);
    }

    #[test]
    fn test_diamond_shape() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.add_node(make_node("A", "A"));
        ast.add_node(make_node("B", "B"));
        ast.add_node(make_node("C", "C"));
        ast.add_node(make_node("D", "D"));
        ast.add_edge(make_edge("A", "B"));
        ast.add_edge(make_edge("A", "C"));
        ast.add_edge(make_edge("B", "D"));
        ast.add_edge(make_edge("C", "D"));

        let (_ranks, rank_map) = assign_ranks(&ast);
        assert_eq!(rank_map["A"], 0);
        assert_eq!(rank_map["D"], 2);
        assert!(rank_map["B"] == 1 || rank_map["C"] == 1);
    }

    #[test]
    fn test_order_nodes_reduces_crossings() {
        let mut ranks = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["C".to_string(), "D".to_string()],
        ];
        let edges = vec![
            make_edge("A", "D"),
            make_edge("B", "C"),
        ];
        let rank_map = HashMap::from([
            ("A".to_string(), 0),
            ("B".to_string(), 0),
            ("C".to_string(), 1),
            ("D".to_string(), 1),
        ]);

        order_nodes(&mut ranks, &edges, 4, &rank_map);

        // The barycenter heuristic should find a crossing-free ordering.
        // Valid solutions: ["A","B"]/["D","C"] or ["B","A"]/["C","D"]
        let valid = (ranks[0] == ["A", "B"] && ranks[1] == ["D", "C"])
            || (ranks[0] == ["B", "A"] && ranks[1] == ["C", "D"]);
        assert!(valid, "Expected crossing-free ordering, got {:?} / {:?}", ranks[0], ranks[1]);
    }

    // ── Complex graph tests ─────────────────────────────────────────

    #[test]
    fn test_20_node_linear_chain() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        for i in 0..20 {
            let id = format!("N{}", i);
            ast.add_node(make_node(&id, &id));
        }
        for i in 0..19 {
            ast.add_edge(make_edge(&format!("N{}", i), &format!("N{}", i + 1)));
        }

        let (ranks, rank_map) = assign_ranks(&ast);
        // Each node should be on a different rank in a linear chain
        assert_eq!(ranks.len(), 20, "20-node linear chain should produce 20 ranks");
        assert_eq!(rank_map["N0"], 0);
        assert_eq!(rank_map["N19"], 19);
    }

    #[test]
    fn test_wide_graph_one_source_to_10_targets() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.add_node(make_node("S", "Source"));
        for i in 0..10 {
            let id = format!("T{}", i);
            ast.add_node(make_node(&id, &id));
            ast.add_edge(make_edge("S", &id));
        }

        let (ranks, rank_map) = assign_ranks(&ast);
        // Source should be rank 0, all targets rank 1
        assert_eq!(rank_map["S"], 0, "Source should be at rank 0");
        for i in 0..10 {
            let id = format!("T{}", i);
            assert_eq!(rank_map[&id], 1, "Target {} should be at rank 1", id);
        }
        // Both ranks should exist
        assert_eq!(ranks.len(), 2, "Should have exactly 2 ranks");
        // Rank 1 should contain all 10 targets
        assert_eq!(ranks[1].len(), 10, "Rank 1 should contain all 10 targets");
    }

    #[test]
    fn test_disconnected_components() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        // Component 1: A → B
        ast.add_node(make_node("A", "A"));
        ast.add_node(make_node("B", "B"));
        ast.add_edge(make_edge("A", "B"));
        // Component 2: C → D
        ast.add_node(make_node("C", "C"));
        ast.add_node(make_node("D", "D"));
        ast.add_edge(make_edge("C", "D"));

        let (ranks, rank_map) = assign_ranks(&ast);
        // Both A and C should be sources (rank 0)
        assert_eq!(rank_map["A"], 0);
        assert_eq!(rank_map["C"], 0);
        // Both B and D should be at rank 1
        assert_eq!(rank_map["B"], 1);
        assert_eq!(rank_map["D"], 1);
        assert_eq!(ranks.len(), 2);
        assert_eq!(ranks[0].len(), 2, "Rank 0 should have 2 sources");
        assert_eq!(ranks[1].len(), 2, "Rank 1 should have 2 targets");
    }

    #[test]
    fn test_back_edges_no_panic() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.add_node(make_node("A", "A"));
        ast.add_node(make_node("B", "B"));
        ast.add_node(make_node("C", "C"));
        ast.add_node(make_node("D", "D"));
        ast.add_edge(make_edge("A", "B"));
        ast.add_edge(make_edge("B", "C"));
        ast.add_edge(make_edge("C", "D"));
        ast.add_edge(make_edge("D", "A")); // back-edge creating a cycle

        // Should not panic
        let (ranks, rank_map) = assign_ranks(&ast);
        assert_eq!(rank_map.len(), 4, "All 4 nodes should be ranked");
        assert!(!ranks.is_empty(), "Should produce at least one rank");
    }

    #[test]
    fn test_empty_ast_complex() {
        let ast = DiagramAst::new(DiagramKind::Flowchart);
        let (ranks, rank_map) = assign_ranks(&ast);
        assert!(ranks.is_empty());
        assert!(rank_map.is_empty());
    }

    #[test]
    fn test_complex_diamond_with_extra_branches() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        // Complex: A -> B, A -> C, B -> D, C -> D, B -> E, C -> F
        for id in &["A", "B", "C", "D", "E", "F"] {
            ast.add_node(make_node(id, id));
        }
        ast.add_edge(make_edge("A", "B"));
        ast.add_edge(make_edge("A", "C"));
        ast.add_edge(make_edge("B", "D"));
        ast.add_edge(make_edge("C", "D"));
        ast.add_edge(make_edge("B", "E"));
        ast.add_edge(make_edge("C", "F"));

        let (ranks, rank_map) = assign_ranks(&ast);
        assert_eq!(rank_map["A"], 0, "A should be at rank 0");
        assert_eq!(rank_map["D"], 2, "D should be at rank 2");
        assert!(rank_map["B"] == 1 && rank_map["C"] == 1, "B and C should be at rank 1");
        assert!(rank_map["E"] == 2 && rank_map["F"] == 2, "E and F should be at rank 2");
        assert!(ranks.len() >= 3, "Should have at least 3 ranks");
    }

    #[test]
    fn test_order_nodes_single_rank() {
        let mut ranks = vec![vec!["A".to_string()]];
        let edges: Vec<DiagramEdge> = vec![];
        let rank_map = HashMap::from([("A".to_string(), 0)]);
        order_nodes(&mut ranks, &edges, 4, &rank_map);
        assert_eq!(ranks[0], vec!["A"], "Single node should remain unchanged");
    }
}
