//! Node positioning within ranks
//!
//! Assigns x,y coordinates to nodes based on their rank and ordering.
//! Uses even spacing within each rank and between ranks.

use std::collections::{BTreeMap, HashMap};
use mermaid_canvas_core::{DiagramAst, Direction, NodeShape, interaction::BoundingBox};
use crate::config::LayoutConfig;
use crate::layout::{NodeLayout, TextBlock};
use crate::theme::Theme;

/// Margin from the edge of the layout canvas
pub(crate) const CANVAS_MARGIN: f64 = 40.0;

/// 标题带与内容的间距（与 layout/mod.rs 的 TITLE_GAP 语义一致）
const TITLE_GAP: f64 = 10.0;

/// Assign x,y positions to nodes based on ranks and ordering.
///
/// Physical arrangement honors `ast.direction`: TopDown/BottomUp stack ranks
/// vertically, LeftToRight/RightToLeft advance ranks horizontally (matching
/// the assumptions of `routing::route_vertical` / `route_horizontal`).
/// BottomUp / RightToLeft are laid out as their forward counterpart and then
/// mirrored.
///
/// 存在标题时在内容上方预留标题带（title_font_size 行高 + 间距）。
/// Returns `(node_layouts, total_width, total_height)`.
pub fn assign_positions<T: Theme>(
    ranks: &[Vec<String>],
    _rank_map: &HashMap<String, usize>,
    ast: &DiagramAst,
    config: &LayoutConfig,
    theme: &T,
) -> (BTreeMap<String, NodeLayout>, f64, f64) {
    // 标题带高度（T15：Title 层在内容带上方预留）
    let title_band = ast.title.as_ref()
        .map(|_| theme.title_font_size() * 1.4 + TITLE_GAP)
        .unwrap_or(0.0);
    match ast.direction {
        Direction::TopDown | Direction::BottomUp => {
            let (mut layouts, w, h) = assign_vertical(ranks, ast, config, theme, title_band);
            if ast.direction == Direction::BottomUp {
                mirror_y(&mut layouts, h);
            }
            (layouts, w, h)
        }
        Direction::LeftToRight | Direction::RightToLeft => {
            let (mut layouts, w, h) = assign_horizontal(ranks, ast, config, theme, title_band);
            if ast.direction == Direction::RightToLeft {
                mirror_x(&mut layouts, w);
            }
            (layouts, w, h)
        }
    }
}

fn compute_all_node_dims<T: Theme>(
    ast: &DiagramAst,
    config: &LayoutConfig,
    theme: &T,
) -> HashMap<String, (f64, f64)> {
    let font_size = theme.font_size();
    ast.nodes
        .iter()
        .map(|(id, node)| {
            let dims = compute_node_size(&node.label, node.shape, config, font_size);
            (id.clone(), dims)
        })
        .collect()
}

fn build_node_layout(
    id: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    node: Option<&mermaid_canvas_core::DiagramNode>,
    font_size: f64,
) -> NodeLayout {
    let label_text = node.map(|n| n.label.as_str()).unwrap_or("");
    let shape = node.map(|n| n.shape).unwrap_or(NodeShape::RoundRect);

    NodeLayout {
        id: id.to_string(),
        x,
        y,
        width: w,
        height: h,
        label: TextBlock {
            text: label_text.to_string(),
            x: x + w / 2.0,
            y: y + h / 2.0,
            width: w,
            height: h,
            font_size,
        },
        shape,
        bounds: BoundingBox::new(x, y, w, h),
    }
}

/// TopDown arrangement: ranks stacked vertically, nodes spread horizontally within a rank.
fn assign_vertical<T: Theme>(
    ranks: &[Vec<String>],
    ast: &DiagramAst,
    config: &LayoutConfig,
    theme: &T,
    title_band: f64,
) -> (BTreeMap<String, NodeLayout>, f64, f64) {
    let font_size = theme.font_size();
    let node_dims = compute_all_node_dims(ast, config, theme);

    let mut rank_widths: Vec<f64> = Vec::with_capacity(ranks.len());
    for rank_nodes in ranks {
        if rank_nodes.is_empty() {
            rank_widths.push(0.0);
            continue;
        }
        let total_node_width: f64 = rank_nodes.iter()
            .filter_map(|id| node_dims.get(id).map(|(w, _)| *w))
            .sum();
        let total_spacing = if rank_nodes.len() > 1 {
            config.node_spacing * (rank_nodes.len() - 1) as f64
        } else {
            0.0
        };
        rank_widths.push(total_node_width + total_spacing);
    }

    let max_rank_width = rank_widths.iter().copied().fold(0.0_f64, f64::max);
    let total_width = max_rank_width + 2.0 * CANVAS_MARGIN;

    // rank_spacing is the GAP between ranks, not the stride:
    // the cursor advances by max_node_height_in_rank + rank_spacing.
    let mut layouts: BTreeMap<String, NodeLayout> = BTreeMap::new();
    let mut y_cursor = CANVAS_MARGIN + title_band;

    for (rank_idx, rank_nodes) in ranks.iter().enumerate() {
        if rank_nodes.is_empty() {
            continue;
        }

        let rank_w = rank_widths[rank_idx];
        let rank_start_x = CANVAS_MARGIN + (max_rank_width - rank_w) / 2.0;

        let max_h_in_rank: f64 = rank_nodes.iter()
            .filter_map(|id| node_dims.get(id).map(|(_, h)| *h))
            .fold(40.0, f64::max);

        let mut x_offset = rank_start_x;
        for id in rank_nodes {
            let (w, h) = node_dims.get(id).unwrap_or(&(80.0, 40.0));
            let nl = build_node_layout(id, x_offset, y_cursor, *w, *h, ast.nodes.get(id), font_size);
            layouts.insert(id.clone(), nl);
            x_offset += w + config.node_spacing;
        }

        // Skip the gap after the last rank; it is re-added via CANVAS_MARGIN below.
        y_cursor += max_h_in_rank + config.rank_spacing;
    }

    let total_height = y_cursor - config.rank_spacing + CANVAS_MARGIN;

    (layouts, total_width, total_height)
}

/// LeftToRight arrangement: ranks advance horizontally, nodes stacked
/// vertically within each rank column (transpose of [`assign_vertical`]).
fn assign_horizontal<T: Theme>(
    ranks: &[Vec<String>],
    ast: &DiagramAst,
    config: &LayoutConfig,
    theme: &T,
    title_band: f64,
) -> (BTreeMap<String, NodeLayout>, f64, f64) {
    let font_size = theme.font_size();
    let node_dims = compute_all_node_dims(ast, config, theme);

    let mut rank_heights: Vec<f64> = Vec::with_capacity(ranks.len());
    for rank_nodes in ranks {
        if rank_nodes.is_empty() {
            rank_heights.push(0.0);
            continue;
        }
        let total_node_height: f64 = rank_nodes.iter()
            .filter_map(|id| node_dims.get(id).map(|(_, h)| *h))
            .sum();
        let total_spacing = if rank_nodes.len() > 1 {
            config.node_spacing * (rank_nodes.len() - 1) as f64
        } else {
            0.0
        };
        rank_heights.push(total_node_height + total_spacing);
    }

    let max_rank_height = rank_heights.iter().copied().fold(0.0_f64, f64::max);
    let total_height = max_rank_height + 2.0 * CANVAS_MARGIN + title_band;

    let mut layouts: BTreeMap<String, NodeLayout> = BTreeMap::new();
    let mut x_cursor = CANVAS_MARGIN;

    for (rank_idx, rank_nodes) in ranks.iter().enumerate() {
        if rank_nodes.is_empty() {
            continue;
        }

        let rank_h = rank_heights[rank_idx];
        let rank_start_y = CANVAS_MARGIN + title_band + (max_rank_height - rank_h) / 2.0;

        let max_w_in_rank: f64 = rank_nodes.iter()
            .filter_map(|id| node_dims.get(id).map(|(w, _)| *w))
            .fold(0.0, f64::max);

        let mut y_offset = rank_start_y;
        for id in rank_nodes {
            let (w, h) = node_dims.get(id).unwrap_or(&(80.0, 40.0));
            let nl = build_node_layout(id, x_cursor, y_offset, *w, *h, ast.nodes.get(id), font_size);
            layouts.insert(id.clone(), nl);
            y_offset += h + config.node_spacing;
        }

        x_cursor += max_w_in_rank + config.rank_spacing;
    }

    let total_width = x_cursor - config.rank_spacing + CANVAS_MARGIN;

    (layouts, total_width, total_height)
}

fn mirror_x(layouts: &mut BTreeMap<String, NodeLayout>, total_width: f64) {
    for nl in layouts.values_mut() {
        nl.x = total_width - nl.x - nl.width;
        nl.label.x = total_width - nl.label.x;
        nl.bounds = BoundingBox::new(nl.x, nl.y, nl.width, nl.height);
    }
}

fn mirror_y(layouts: &mut BTreeMap<String, NodeLayout>, total_height: f64) {
    for nl in layouts.values_mut() {
        nl.y = total_height - nl.y - nl.height;
        nl.label.y = total_height - nl.label.y;
        nl.bounds = BoundingBox::new(nl.x, nl.y, nl.width, nl.height);
    }
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
        DiagramAst, DiagramKind, DiagramNode, DiagramEdge, NodeShape, EdgeStyle, Direction,
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

    fn ranked_two_node_ast(direction: Direction) -> (DiagramAst, Vec<Vec<String>>, HashMap<String, usize>) {
        let mut ast = DiagramAst::new(DiagramKind::Flowchart);
        ast.direction = direction;
        ast.add_node(make_node("A", "First"));
        ast.add_node(make_node("B", "Second"));
        ast.add_edge(make_edge("A", "B"));
        let ranks = vec![vec!["A".to_string()], vec!["B".to_string()]];
        let rank_map = HashMap::from([("A".to_string(), 0), ("B".to_string(), 1)]);
        (ast, ranks, rank_map)
    }

    #[test]
    fn test_lr_direction_lays_ranks_horizontally() {
        // Regression: echodawn 主题同步 `flowchart LR` 菱形连线端点错位 — rank 必须横向推进
        let (ast, ranks, rank_map) = ranked_two_node_ast(Direction::LeftToRight);
        let config = LayoutConfig::default();
        let theme = DefaultTheme;

        let (nodes, _, _) = assign_positions(&ranks, &rank_map, &ast, &config, &theme);
        assert!(
            nodes["B"].x >= nodes["A"].x + nodes["A"].width,
            "LR layout: rank-1 node B must be right of A, got A.x={} A.w={} B.x={}",
            nodes["A"].x,
            nodes["A"].width,
            nodes["B"].x
        );
        let a_cy = nodes["A"].y + nodes["A"].height / 2.0;
        let b_cy = nodes["B"].y + nodes["B"].height / 2.0;
        assert!((a_cy - b_cy).abs() < 1.0, "LR layout: ranks should be vertically centered on one line");
    }

    #[test]
    fn test_rl_direction_mirrors_lr() {
        let (ast, ranks, rank_map) = ranked_two_node_ast(Direction::RightToLeft);
        let config = LayoutConfig::default();
        let theme = DefaultTheme;

        let (nodes, _, _) = assign_positions(&ranks, &rank_map, &ast, &config, &theme);
        assert!(
            nodes["B"].x + nodes["B"].width <= nodes["A"].x,
            "RL layout: rank-1 node B must be left of A, got A.x={} B.x={} B.w={}",
            nodes["A"].x,
            nodes["B"].x,
            nodes["B"].width
        );
    }

    #[test]
    fn test_bottom_up_mirrors_top_down() {
        let (ast, ranks, rank_map) = ranked_two_node_ast(Direction::BottomUp);
        let config = LayoutConfig::default();
        let theme = DefaultTheme;

        let (nodes, _, _) = assign_positions(&ranks, &rank_map, &ast, &config, &theme);
        assert!(
            nodes["B"].y + nodes["B"].height <= nodes["A"].y,
            "BU layout: rank-1 node B must be above A, got A.y={} B.y={} B.h={}",
            nodes["A"].y,
            nodes["B"].y,
            nodes["B"].height
        );
    }
}
