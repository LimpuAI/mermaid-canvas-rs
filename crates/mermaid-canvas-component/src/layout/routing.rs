//! Edge routing
//!
//! Computes polyline routes between node ports.
//! Simple orthogonal routing: straight lines between source/target ports
//! with bends for back-edges and same-rank connections.

use std::collections::{BTreeMap, HashMap};
use mermaid_canvas_core::{DiagramEdge, Direction};
use crate::layout::{EdgeLayout, NodeLayout, TextBlock};

/// Route all edges as polylines between node ports.
pub fn route_edges(
    edges: &[DiagramEdge],
    nodes: &BTreeMap<String, NodeLayout>,
    _ranks: &[Vec<String>],
    rank_map: &HashMap<String, usize>,
    direction: Direction,
) -> Vec<EdgeLayout> {
    edges.iter().map(|edge| route_single_edge(edge, nodes, rank_map, direction)).collect()
}

/// Route a single edge.
fn route_single_edge(
    edge: &DiagramEdge,
    nodes: &BTreeMap<String, NodeLayout>,
    rank_map: &HashMap<String, usize>,
    direction: Direction,
) -> EdgeLayout {
    let from_node = match nodes.get(&edge.from) {
        Some(n) => n,
        None => return empty_edge(edge),
    };
    let to_node = match nodes.get(&edge.to) {
        Some(n) => n,
        None => return empty_edge(edge),
    };

    let from_rank = rank_map.get(&edge.from).copied().unwrap_or(0);
    let to_rank = rank_map.get(&edge.to).copied().unwrap_or(0);

    let points = match direction {
        Direction::TopDown | Direction::BottomUp => {
            route_vertical(edge, from_node, to_node, from_rank, to_rank, direction)
        }
        Direction::LeftToRight | Direction::RightToLeft => {
            route_horizontal(edge, from_node, to_node, from_rank, to_rank, direction)
        }
    };

    // Compute label anchor at midpoint
    let label_anchor = if edge.label.is_some() && points.len() >= 2 {
        let mid_idx = points.len() / 2;
        let (x1, y1) = points[mid_idx - 1];
        let (x2, y2) = points[mid_idx];
        Some(((x1 + x2) / 2.0, (y1 + y2) / 2.0))
    } else {
        None
    };

    let label = edge.label.as_ref().map(|text| TextBlock {
        text: text.clone(),
        x: label_anchor.unwrap_or((0.0, 0.0)).0,
        y: label_anchor.unwrap_or((0.0, 0.0)).1,
        // 字宽估算按字符数（CJK 字节数会高估 3x）
        width: text.chars().count() as f64 * 7.0,
        height: 16.0,
        font_size: 12.0,
    });

    EdgeLayout {
        from: edge.from.clone(),
        to: edge.to.clone(),
        points,
        label,
        label_anchor,
        directed: edge.directed,
        // 形态学元数据透传（T11 箭头 / T12 线型消费）
        arrow_start: edge.arrow_start,
        arrow_end: edge.arrow_end,
        start_decoration: edge.start_decoration,
        end_decoration: edge.end_decoration,
        style: edge.style,
    }
}

/// Route edge for vertical layouts (TopDown / BottomUp).
fn route_vertical(
    edge: &DiagramEdge,
    from: &NodeLayout,
    to: &NodeLayout,
    from_rank: usize,
    to_rank: usize,
    direction: Direction,
) -> Vec<(f64, f64)> {
    let from_cx = from.x + from.width / 2.0;
    let to_cx = to.x + to.width / 2.0;

    if edge.from == edge.to {
        // Self-loop: route as a small bubble to the right
        let right = from.x + from.width;
        let top = from.y;
        let bottom = from.y + from.height;
        let offset = 20.0;
        return vec![
            (right, top + from.height * 0.3),
            (right + offset, top + from.height * 0.3),
            (right + offset, bottom - from.height * 0.3),
            (right, bottom - from.height * 0.3),
        ];
    }

    if from_rank == to_rank {
        // Same rank — route around (U-shape below or above)
        let is_top_down = direction == Direction::TopDown;
        let y_offset = if is_top_down {
            from.y + from.height + 20.0
        } else {
            from.y - 20.0
        };
        return vec![
            (from_cx, if is_top_down { from.y + from.height } else { from.y }),
            (from_cx, y_offset),
            (to_cx, y_offset),
            (to_cx, if is_top_down { to.y + to.height } else { to.y }),
        ];
    }

    let is_forward = match direction {
        Direction::TopDown => to_rank > from_rank,
        Direction::BottomUp => to_rank < from_rank,
        _ => to_rank > from_rank,
    };

    if is_forward {
        // Forward edge: straight from source port to target port
        let (start_y, end_y) = if direction == Direction::TopDown {
            (from.y + from.height, to.y)
        } else {
            (from.y, to.y + to.height)
        };

        if (from_cx - to_cx).abs() < 1.0 {
            // Vertically aligned — straight line
            return vec![(from_cx, start_y), (to_cx, end_y)];
        }

        // Angled: use a midpoint bend
        let mid_y = (start_y + end_y) / 2.0;
        return vec![
            (from_cx, start_y),
            (from_cx, mid_y),
            (to_cx, mid_y),
            (to_cx, end_y),
        ];
    } else {
        // Back-edge: route around the side
        let is_top_down = direction == Direction::TopDown;
        let (start_y, end_y) = if is_top_down {
            (from.y + from.height, to.y)
        } else {
            (from.y, to.y + to.height)
        };

        // Route to the left side
        let side_x = from.x.min(to.x) - 30.0;
        return vec![
            (from_cx, start_y),
            (from_cx, start_y + 10.0),
            (side_x, start_y + 10.0),
            (side_x, end_y - 10.0),
            (to_cx, end_y - 10.0),
            (to_cx, end_y),
        ];
    }
}

/// Route edge for horizontal layouts (LeftToRight / RightToLeft).
fn route_horizontal(
    edge: &DiagramEdge,
    from: &NodeLayout,
    to: &NodeLayout,
    from_rank: usize,
    to_rank: usize,
    direction: Direction,
) -> Vec<(f64, f64)> {
    let from_cy = from.y + from.height / 2.0;
    let to_cy = to.y + to.height / 2.0;

    if edge.from == edge.to {
        // Self-loop
        let right = from.x + from.width;
        let offset = 20.0;
        return vec![
            (right, from_cy - from.height * 0.2),
            (right + offset, from_cy - from.height * 0.2),
            (right + offset, from_cy + from.height * 0.2),
            (right, from_cy + from.height * 0.2),
        ];
    }

    if from_rank == to_rank {
        // Same rank — route around (U-shape to the side)
        let is_ltr = direction == Direction::LeftToRight;
        let x_offset = if is_ltr {
            from.x + from.width + 20.0
        } else {
            from.x - 20.0
        };
        return vec![
            (if is_ltr { from.x + from.width } else { from.x }, from_cy),
            (x_offset, from_cy),
            (x_offset, to_cy),
            (if is_ltr { to.x + to.width } else { to.x }, to_cy),
        ];
    }

    let is_forward = match direction {
        Direction::LeftToRight => to_rank > from_rank,
        Direction::RightToLeft => to_rank < from_rank,
        _ => to_rank > from_rank,
    };

    if is_forward {
        let (start_x, end_x) = if direction == Direction::LeftToRight {
            (from.x + from.width, to.x)
        } else {
            (from.x, to.x + to.width)
        };

        if (from_cy - to_cy).abs() < 1.0 {
            return vec![(start_x, from_cy), (end_x, to_cy)];
        }

        let mid_x = (start_x + end_x) / 2.0;
        return vec![
            (start_x, from_cy),
            (mid_x, from_cy),
            (mid_x, to_cy),
            (end_x, to_cy),
        ];
    } else {
        let is_ltr = direction == Direction::LeftToRight;
        let (start_x, end_x) = if is_ltr {
            (from.x + from.width, to.x)
        } else {
            (from.x, to.x + to.width)
        };

        let side_y = from.y.min(to.y) - 30.0;
        return vec![
            (start_x, from_cy),
            (start_x + if is_ltr { 10.0 } else { -10.0 }, from_cy),
            (start_x + if is_ltr { 10.0 } else { -10.0 }, side_y),
            (end_x + if is_ltr { -10.0 } else { 10.0 }, side_y),
            (end_x + if is_ltr { -10.0 } else { 10.0 }, to_cy),
            (end_x, to_cy),
        ];
    }
}

/// Create an empty edge layout for missing nodes
fn empty_edge(edge: &DiagramEdge) -> EdgeLayout {
    EdgeLayout {
        from: edge.from.clone(),
        to: edge.to.clone(),
        points: vec![(0.0, 0.0), (0.0, 0.0)],
        label: None,
        label_anchor: None,
        directed: edge.directed,
        arrow_start: edge.arrow_start,
        arrow_end: edge.arrow_end,
        start_decoration: edge.start_decoration,
        end_decoration: edge.end_decoration,
        style: edge.style,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{NodeLayout, TextBlock};
    use mermaid_canvas_core::{DiagramEdge, NodeShape, EdgeStyle, interaction::BoundingBox};

    fn make_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            x,
            y,
            width: w,
            height: h,
            label: TextBlock {
                text: id.to_string(),
                x: x + w / 2.0,
                y: y + h / 2.0,
                width: w,
                height: h,
                font_size: 14.0,
            },
            shape: NodeShape::RoundRect,
            bounds: BoundingBox::new(x, y, w, h),
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
    fn test_simple_edge() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 40.0, 80.0, 40.0));
        nodes.insert("B".to_string(), make_node("B", 50.0, 130.0, 80.0, 40.0));

        let rank_map = HashMap::from([("A".to_string(), 0), ("B".to_string(), 1)]);
        let edge = make_edge("A", "B");

        let result = route_single_edge(&edge, &nodes, &rank_map, Direction::TopDown);
        assert_eq!(result.points.len(), 2); // Straight line (aligned)
        assert_eq!(result.from, "A");
        assert_eq!(result.to, "B");
        assert!(result.directed);
    }

    #[test]
    fn test_angled_edge() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 40.0, 80.0, 40.0));
        nodes.insert("B".to_string(), make_node("B", 200.0, 130.0, 80.0, 40.0));

        let rank_map = HashMap::from([("A".to_string(), 0), ("B".to_string(), 1)]);
        let edge = make_edge("A", "B");

        let result = route_single_edge(&edge, &nodes, &rank_map, Direction::TopDown);
        assert_eq!(result.points.len(), 4); // Bend path
    }

    #[test]
    fn test_self_loop() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 40.0, 80.0, 40.0));

        let rank_map = HashMap::from([("A".to_string(), 0)]);
        let edge = make_edge("A", "A");

        let result = route_single_edge(&edge, &nodes, &rank_map, Direction::TopDown);
        assert_eq!(result.points.len(), 4); // Loop path
    }

    #[test]
    fn test_same_rank_edge() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 40.0, 80.0, 40.0));
        nodes.insert("B".to_string(), make_node("B", 200.0, 40.0, 80.0, 40.0));

        let rank_map = HashMap::from([("A".to_string(), 0), ("B".to_string(), 0)]);
        let edge = make_edge("A", "B");

        let result = route_single_edge(&edge, &nodes, &rank_map, Direction::TopDown);
        assert_eq!(result.points.len(), 4); // U-shape
    }
}
