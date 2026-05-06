//! Node positioning within ranks
//!
//! Assigns x,y coordinates to nodes based on their rank and ordering.
//! Uses even spacing within each rank and between ranks.

use std::collections::{BTreeMap, HashMap};
use mermaid_canvas_core::{DiagramAst, NodeShape, interaction::BoundingBox};
use crate::config::LayoutConfig;
use crate::layout::{NodeLayout, TextBlock};
use crate::theme::Theme;

/// Margin from the edge of the layout canvas
const CANVAS_MARGIN: f64 = 40.0;

/// Assign x,y positions to nodes based on ranks and ordering.
///
/// Returns `(node_layouts, total_width, total_height)`.
pub fn assign_positions<T: Theme>(
    ranks: &[Vec<String>],
    _rank_map: &HashMap<String, usize>,
    ast: &DiagramAst,
    config: &LayoutConfig,
    theme: &T,
) -> (BTreeMap<String, NodeLayout>, f64, f64) {
    let font_size = theme.font_size();

    // Step 1: Compute node dimensions
    let mut node_dims: HashMap<String, (f64, f64)> = HashMap::new();
    for (id, node) in &ast.nodes {
        let (w, h) = compute_node_size(&node.label, node.shape, config, font_size);
        node_dims.insert(id.clone(), (w, h));
    }

    // Step 2: Find max node width per rank for alignment
    let mut rank_max_width: Vec<f64> = vec![0.0; ranks.len()];
    for (rank_idx, rank_nodes) in ranks.iter().enumerate() {
        for id in rank_nodes {
            if let Some(&(w, _)) = node_dims.get(id) {
                rank_max_width[rank_idx] = rank_max_width[rank_idx].max(w);
            }
        }
    }

    // Step 3: Compute rank widths and positions
    let node_spacing = config.node_spacing;
    let rank_spacing = config.rank_spacing;

    // For each rank, compute total width = sum of max_node_width_in_rank + spacing
    // Actually, each node keeps its own width — we just space them evenly
    let mut rank_widths: Vec<f64> = Vec::with_capacity(ranks.len());
    for (_rank_idx, rank_nodes) in ranks.iter().enumerate() {
        if rank_nodes.is_empty() {
            rank_widths.push(0.0);
            continue;
        }
        let total_node_width: f64 = rank_nodes.iter()
            .filter_map(|id| node_dims.get(id).map(|(w, _)| *w))
            .sum();
        let total_spacing = if rank_nodes.len() > 1 {
            node_spacing * (rank_nodes.len() - 1) as f64
        } else {
            0.0
        };
        rank_widths.push(total_node_width + total_spacing);
    }

    // Overall width is max of all rank widths
    let max_rank_width = rank_widths.iter().copied().fold(0.0_f64, f64::max);
    let total_width = max_rank_width + 2.0 * CANVAS_MARGIN;

    // Step 4: Assign positions using cumulative Y cursor
    // rank_spacing is the GAP between ranks, not the stride.
    // Y cursor advances by max_node_height_in_rank + rank_spacing.
    let mut layouts: BTreeMap<String, NodeLayout> = BTreeMap::new();
    let mut y_cursor = CANVAS_MARGIN;

    for (rank_idx, rank_nodes) in ranks.iter().enumerate() {
        if rank_nodes.is_empty() {
            continue;
        }

        let rank_w = rank_widths[rank_idx];
        // Center this rank within the total width
        let rank_start_x = CANVAS_MARGIN + (max_rank_width - rank_w) / 2.0;

        // Find max node height in this rank for Y cursor advancement
        let max_h_in_rank: f64 = rank_nodes.iter()
            .filter_map(|id| node_dims.get(id).map(|(_, h)| *h))
            .fold(40.0, f64::max);

        let mut x_offset = rank_start_x;
        for id in rank_nodes {
            let (w, h) = node_dims.get(id).unwrap_or(&(80.0, 40.0));
            let node = ast.nodes.get(id);

            let label_text = node.map(|n| n.label.as_str()).unwrap_or("");
            let shape = node.map(|n| n.shape).unwrap_or(NodeShape::RoundRect);

            let label = TextBlock {
                text: label_text.to_string(),
                x: x_offset + w / 2.0,
                y: y_cursor + h / 2.0,
                width: *w,
                height: *h,
                font_size,
            };

            let nl = NodeLayout {
                id: id.clone(),
                x: x_offset,
                y: y_cursor,
                width: *w,
                height: *h,
                label,
                shape,
                bounds: BoundingBox::new(x_offset, y_cursor, *w, *h),
            };

            layouts.insert(id.clone(), nl);
            x_offset += w + node_spacing;
        }

        // Advance Y cursor by max height + gap (skip gap for last rank)
        y_cursor += max_h_in_rank + rank_spacing;
    }

    // Compute total height from Y cursor (already includes last rank height + one gap)
    let total_height = y_cursor - rank_spacing + CANVAS_MARGIN;

    (layouts, total_width, total_height)
}

/// Compute node width and height based on label text and shape.
fn compute_node_size(label: &str, shape: NodeShape, config: &LayoutConfig, font_size: f64) -> (f64, f64) {
    let char_width = font_size * 0.6;
    let line_height = font_size * config.label_line_height;

    // Split label by newlines for multi-line support
    let lines: Vec<&str> = label.lines().collect();
    let max_line_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1);

    // Wrap if needed
    let effective_chars = max_line_chars.min(config.max_label_width_chars);
    let effective_lines = if max_line_chars > config.max_label_width_chars {
        let extra = (max_line_chars + config.max_label_width_chars - 1) / config.max_label_width_chars;
        lines.len().max(extra)
    } else {
        lines.len()
    };
    let num_lines = effective_lines.max(1);

    let text_width = effective_chars as f64 * char_width;
    let text_height = num_lines as f64 * line_height;

    let base_w = text_width + 2.0 * config.node_padding_x;
    let base_h = text_height + 2.0 * config.node_padding_y;

    // Minimum dimensions
    let min_w = 60.0_f64.max(base_w);
    let min_h = 36.0_f64.max(base_h);

    // Shape-specific adjustments
    let (w, h) = match shape {
        NodeShape::Diamond => (min_w * 1.6, min_h * 1.6),
        NodeShape::Circle | NodeShape::DoubleCircle => {
            let d = min_w.max(min_h);
            (d, d)
        }
        NodeShape::Hexagon => (min_w * 1.3, min_h * 1.1),
        NodeShape::Stadium => (min_w.max(min_h * 2.0), min_h),
        NodeShape::Parallelogram => (min_w * 1.25, min_h),
        NodeShape::Trapezoid => (min_w * 1.2, min_h),
        NodeShape::Cylinder => (min_w, min_h * 1.3),
        NodeShape::Asymmetric => (min_w * 1.1, min_h),
        _ => (min_w, min_h),
    };

    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mermaid_canvas_core::{
        DiagramAst, DiagramKind, DiagramNode, DiagramEdge, NodeShape, EdgeStyle,
    };
    use mermaid_canvas_core::diagram::NodeStyle;
    use crate::theme::DefaultTheme;

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
    fn test_single_node_positioning() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.add_node(make_node("A", "Hello"));
        let ranks = vec![vec!["A".to_string()]];
        let rank_map = HashMap::from([("A".to_string(), 0)]);
        let config = LayoutConfig::default();
        let theme = DefaultTheme;

        let (nodes, w, h) = assign_positions(&ranks, &rank_map, &ast, &config, &theme);
        assert!(nodes.contains_key("A"));
        assert!(w > 0.0);
        assert!(h > 0.0);

        let node = &nodes["A"];
        assert!(node.x >= CANVAS_MARGIN);
        assert!(node.y >= CANVAS_MARGIN);
    }

    #[test]
    fn test_two_rank_positioning() {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.add_node(make_node("A", "Top"));
        ast.add_node(make_node("B", "Bottom"));
        ast.add_edge(make_edge("A", "B"));

        let ranks = vec![vec!["A".to_string()], vec!["B".to_string()]];
        let rank_map = HashMap::from([("A".to_string(), 0), ("B".to_string(), 1)]);
        let config = LayoutConfig::default();
        let theme = DefaultTheme;

        let (nodes, _, _) = assign_positions(&ranks, &rank_map, &ast, &config, &theme);
        assert!(nodes["B"].y > nodes["A"].y);
    }
}
