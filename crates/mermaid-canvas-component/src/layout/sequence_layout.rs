//! Sequence diagram layout engine
//!
//! Computes positions for participants (X axis), steps (Y axis),
//! lifelines, activation boxes, and control block backgrounds.

use std::collections::BTreeMap;

use mermaid_canvas_core::{DiagramAst, EdgeStyle, interaction::BoundingBox, NodeShape};

use crate::config::LayoutConfig;
use crate::layout::{EdgeLayout, Layout, NodeLayout, SubgraphLayout, TextBlock};
use crate::theme::Theme;

// ── Layout constants ─────────────────────────────────────────────────

/// Horizontal margin from edge of canvas
const MARGIN_X: f64 = 50.0;
/// Vertical margin from top
const MARGIN_Y: f64 = 30.0;
/// Height of participant header box
const HEADER_HEIGHT: f64 = 40.0;
/// Gap below header before messages start
const HEADER_GAP: f64 = 20.0;
/// Height of each step row
const STEP_HEIGHT: f64 = 45.0;
/// Width of activation box
const ACTIVATION_WIDTH: f64 = 16.0;
/// Offset per stacked activation depth
const ACTIVATION_DEPTH_OFFSET: f64 = 6.0;
/// Default column width for each participant
const DEFAULT_COLUMN_WIDTH: f64 = 160.0;
/// Bottom margin
const BOTTOM_MARGIN: f64 = 40.0;

/// Compute sequence diagram layout
pub fn compute_sequence_layout<T: Theme>(
    ast: &DiagramAst,
    theme: &T,
    _config: &LayoutConfig,
) -> Layout {
    let meta = match &ast.sequence_meta {
        Some(m) => m,
        None => return empty_layout(),
    };

    if meta.participant_order.is_empty() {
        return empty_layout();
    }

    // 标题带（T15 — Title 层在参与头之上预留）
    let title_band = ast.title.as_ref()
        .map(|_| theme.title_font_size() * 1.4 + 8.0)
        .unwrap_or(0.0);
    let top = MARGIN_Y + title_band;

    // 1. Compute participant X positions
    let participant_count = meta.participant_order.len();
    let column_width = compute_column_width(ast, participant_count);

    let mut participant_x: BTreeMap<String, f64> = BTreeMap::new();
    for (i, pid) in meta.participant_order.iter().enumerate() {
        let x = MARGIN_X + i as f64 * column_width;
        participant_x.insert(pid.clone(), x);
    }

    // 2. Compute Y positions per step
    let first_message_y = top + HEADER_HEIGHT + HEADER_GAP;
    let total_steps = meta.total_steps.max(ast.edges.len());

    // 3. Build node layouts for participant headers
    let mut nodes = BTreeMap::new();
    for pid in &meta.participant_order {
        let x = *participant_x.get(pid).unwrap();
        let node = ast.nodes.get(pid);
        let label_text = node.map(|n| n.label.as_str()).unwrap_or(pid.as_ref());
        let is_actor = meta.is_actor.get(pid).copied().unwrap_or(false);
        let shape = if is_actor { NodeShape::Circle } else { NodeShape::Rectangle };

        let header_width = estimate_text_width(label_text).max(80.0);
        let header_height = if is_actor { 50.0 } else { HEADER_HEIGHT };
        let header_x = x - header_width / 2.0;

        nodes.insert(pid.clone(), NodeLayout {
            id: pid.clone(),
            x: header_x,
            y: top,
            width: header_width,
            height: header_height,
            label: TextBlock {
                text: label_text.to_string(),
                x: header_x,
                y: top,
                width: header_width,
                height: header_height,
                font_size: 14.0,
            },
            shape,
            bounds: BoundingBox::new(header_x, top, header_width, header_height),
        });
    }

    // 4. Build edge layouts for messages
    let mut edges = Vec::new();
    for (step_idx, edge) in ast.edges.iter().enumerate() {
        let from_x = *participant_x.get(&edge.from).unwrap_or(&0.0);
        let to_x = *participant_x.get(&edge.to).unwrap_or(&0.0);
        let y = first_message_y + step_idx as f64 * STEP_HEIGHT;

        if edge.from == edge.to {
            // Self-referencing message: small loop
            let loop_width = 40.0;
            let loop_height = 25.0;
            edges.push(EdgeLayout {
                from: edge.from.clone(),
                to: edge.to.clone(),
                points: vec![
                    (from_x, y),
                    (from_x + loop_width, y),
                    (from_x + loop_width, y + loop_height),
                    (from_x, y + loop_height),
                ],
                label: edge.label.as_ref().map(|l| TextBlock {
                    text: l.clone(),
                    x: from_x + loop_width,
                    y: y - 5.0,
                    width: estimate_text_width(l),
                    height: 16.0,
                    font_size: 12.0,
                }),
                label_anchor: Some((from_x + loop_width + 5.0, y - 5.0)),
                directed: edge.directed,
                arrow_start: edge.arrow_start,
                arrow_end: edge.arrow_end,
                start_decoration: edge.start_decoration,
                end_decoration: edge.end_decoration,
                style: edge.style,
            });
        } else {
            edges.push(EdgeLayout {
                from: edge.from.clone(),
                to: edge.to.clone(),
                points: vec![(from_x, y), (to_x, y)],
                label: edge.label.as_ref().map(|l| {
                    let label_x = (from_x + to_x) / 2.0;
                    TextBlock {
                        text: l.clone(),
                        x: label_x,
                        y: y - 8.0,
                        width: estimate_text_width(l),
                        height: 16.0,
                        font_size: 12.0,
                    }
                }),
                label_anchor: edge.label.as_ref().map(|_| {
                    ((from_x + to_x) / 2.0, y - 8.0)
                }),
                directed: edge.directed,
                arrow_start: edge.arrow_start,
                arrow_end: edge.arrow_end,
                start_decoration: edge.start_decoration,
                end_decoration: edge.end_decoration,
                style: edge.style,
            });
        }
    }

    // 5. Build subgraph layouts for control blocks and rect backgrounds
    let mut subgraphs = Vec::new();

    // Control blocks
    for cb in &meta.control_blocks {
        let start_y = first_message_y + cb.start_step as f64 * STEP_HEIGHT - STEP_HEIGHT / 2.0;
        let end_y = first_message_y + cb.end_step as f64 * STEP_HEIGHT;
        let min_x = MARGIN_X - 20.0;
        let max_x = MARGIN_X + (participant_count - 1) as f64 * column_width + 20.0;
        let label = format!("{:?}", cb.kind);
        let cb_label = if cb.label.is_empty() {
            label.clone()
        } else {
            format!("[{}] {}", label, cb.label)
        };

        subgraphs.push(SubgraphLayout {
            id: format!("cb_{}", subgraphs.len()),
            label: TextBlock {
                text: cb_label.clone(),
                x: min_x + 5.0,
                y: start_y + 2.0,
                width: estimate_text_width(&cb_label),
                height: 16.0,
                font_size: 12.0,
            },
            x: min_x,
            y: start_y,
            width: max_x - min_x,
            height: end_y - start_y,
        });

        // Add group separator labels
        for (group_label, group_step) in &cb.groups {
            let gy = first_message_y + *group_step as f64 * STEP_HEIGHT - STEP_HEIGHT / 2.0;
            if !group_label.is_empty() {
                subgraphs.push(SubgraphLayout {
                    id: format!("cb_{}_group_{}", subgraphs.len(), group_step),
                    label: TextBlock {
                        text: format!("[else] {}", group_label),
                        x: min_x + 5.0,
                        y: gy + 2.0,
                        width: estimate_text_width(group_label) + 50.0,
                        height: 16.0,
                        font_size: 12.0,
                    },
                    x: min_x,
                    y: gy - 1.0,
                    width: max_x - min_x,
                    height: 2.0,
                });
            }
        }
    }

    // Rect backgrounds
    for rect in &meta.rects {
        let start_y = first_message_y + rect.start_step as f64 * STEP_HEIGHT - STEP_HEIGHT / 2.0;
        let end_y = first_message_y + rect.end_step as f64 * STEP_HEIGHT;
        let min_x = MARGIN_X - 20.0;
        let max_x = MARGIN_X + (participant_count - 1) as f64 * column_width + 20.0;

        subgraphs.push(SubgraphLayout {
            id: format!("rect_{}", subgraphs.len()),
            label: TextBlock {
                text: String::new(),
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                font_size: 12.0,
            },
            x: min_x,
            y: start_y,
            width: max_x - min_x,
            height: end_y - start_y,
        });
    }

    // 6. Add activation box rects as nodes (with special id prefix)
    for act in &meta.activations {
        let x = *participant_x.get(&act.participant_id).unwrap_or(&0.0);
        let start_y = first_message_y + act.start_step as f64 * STEP_HEIGHT;
        let end_y = match act.end_step {
            Some(es) => first_message_y + es as f64 * STEP_HEIGHT,
            None => first_message_y + total_steps as f64 * STEP_HEIGHT,
        };
        let depth_offset = act.depth as f64 * ACTIVATION_DEPTH_OFFSET;
        let act_x = x - ACTIVATION_WIDTH / 2.0 + depth_offset;

        nodes.insert(format!("__act_{}", nodes.len()), NodeLayout {
            id: format!("activation_{}_{}", act.participant_id, act.start_step),
            x: act_x,
            y: start_y - STEP_HEIGHT / 2.0,
            width: ACTIVATION_WIDTH,
            height: end_y - start_y + STEP_HEIGHT / 2.0,
            label: TextBlock {
                text: String::new(),
                x: act_x,
                y: start_y,
                width: ACTIVATION_WIDTH,
                height: end_y - start_y,
                font_size: 10.0,
            },
            shape: NodeShape::Rectangle,
            bounds: BoundingBox::new(
                act_x,
                start_y - STEP_HEIGHT / 2.0,
                ACTIVATION_WIDTH,
                end_y - start_y + STEP_HEIGHT / 2.0,
            ),
        });
    }

    // 7. Add lifeline edges (vertical dashed lines)
    // These go from bottom of header box to last step Y
    let last_y = if total_steps > 0 {
        first_message_y + (total_steps - 1) as f64 * STEP_HEIGHT + STEP_HEIGHT
    } else {
        first_message_y
    };

    for pid in &meta.participant_order {
        let x = *participant_x.get(pid).unwrap();
        let header_bottom = top + HEADER_HEIGHT;
        edges.push(EdgeLayout {
            from: pid.clone(),
            to: format!("{}_lifeline_end", pid),
            points: vec![
                (x, header_bottom),
                (x, last_y),
            ],
            label: None,
            label_anchor: None,
            directed: false,
            arrow_start: None,
            arrow_end: None,
            start_decoration: None,
            end_decoration: None,
            // 生命线 = 长虚线（T12 消费为 dash 节律）
            style: EdgeStyle::Dashed,
        });
    }

    // 8. Compute canvas size
    let canvas_width = MARGIN_X * 2.0 + (participant_count - 1).max(0) as f64 * column_width;
    let canvas_height = last_y + BOTTOM_MARGIN;

    // 标题带（顶部居中）
    let title = ast.title.as_ref().map(|text| {
        let fs = theme.title_font_size();
        TextBlock {
            text: text.clone(),
            x: canvas_width / 2.0,
            y: MARGIN_Y / 2.0 + fs / 2.0,
            width: estimate_text_width(text),
            height: fs * 1.4,
            font_size: fs,
        }
    });

    Layout {
        width: canvas_width,
        height: canvas_height,
        nodes,
        edges,
        subgraphs,
        title,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn empty_layout() -> Layout {
    Layout {
        width: 200.0,
        height: 100.0,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        title: None,
    }
}

/// Compute column width based on longest participant label
fn compute_column_width(ast: &DiagramAst, participant_count: usize) -> f64 {
    let max_label_width = ast.nodes.values()
        .map(|n| estimate_text_width(&n.label))
        .fold(0.0f64, f64::max);
    // Each column should be at least as wide as 2x the label + some spacing
    let min_column = (max_label_width * 2.0 + 40.0).max(DEFAULT_COLUMN_WIDTH);
    // But also at least 120
    min_column.max(120.0)
}

/// Rough estimate of text width for layout calculations
fn estimate_text_width(text: &str) -> f64 {
    // Rough estimate: 8px per character at 14px font size
    text.chars().count() as f64 * 8.0
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::DefaultTheme;
    use crate::config::LayoutConfig;
    use mermaid_canvas_core::{parse_mermaid, DiagramKind};

    #[test]
    fn test_layout_basic_sequence() {
        let ast = parse_mermaid("sequenceDiagram\n    A->>B: Hello\n    B-->>A: Hi").unwrap();
        assert_eq!(ast.kind, DiagramKind::Sequence);
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
        // Should have 2 participant nodes + lifelines
        assert!(layout.nodes.contains_key("A"));
        assert!(layout.nodes.contains_key("B"));
        // 2 message edges + 2 lifeline edges = 4
        assert_eq!(layout.edges.len(), 4);
    }

    #[test]
    fn test_layout_participant_positions() {
        let ast = parse_mermaid("sequenceDiagram\n    participant A\n    participant B\n    participant C\n    A->>B: Hello").unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        let a = layout.nodes.get("A").unwrap();
        let b = layout.nodes.get("B").unwrap();
        let c = layout.nodes.get("C").unwrap();
        // A should be left of B, B left of C
        let a_center = a.x + a.width / 2.0;
        let b_center = b.x + b.width / 2.0;
        let c_center = c.x + c.width / 2.0;
        assert!(a_center < b_center, "A ({}) should be left of B ({})", a_center, b_center);
        assert!(b_center < c_center, "B ({}) should be left of C ({})", b_center, c_center);
    }

    #[test]
    fn test_layout_message_positions() {
        let ast = parse_mermaid("sequenceDiagram\n    A->>B: First\n    B->>A: Second").unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        // First message should be above second
        let msg1_y = layout.edges[0].points[0].1;
        let msg2_y = layout.edges[1].points[0].1;
        assert!(msg1_y < msg2_y, "First message Y ({}) should be above second ({})", msg1_y, msg2_y);
    }

    #[test]
    fn test_layout_with_activations() {
        let input = "sequenceDiagram\n\
            Client->>+Server: Request\n\
            Server->>+Database: Query\n\
            Database-->>-Server: Results\n\
            Server-->>-Client: Response";
        let ast = parse_mermaid(input).unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        // Should have activation box nodes (prefixed with __act_)
        let activation_count = layout.nodes.keys().filter(|k| k.starts_with("__act_")).count();
        assert_eq!(activation_count, 2, "Should have 2 activation boxes");
    }

    #[test]
    fn test_layout_with_control_block() {
        let input = "sequenceDiagram\n\
            participant A\n\
            participant B\n\
            loop Every second\n\
                A->>B: Ping\n\
            end";
        let ast = parse_mermaid(input).unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        assert_eq!(layout.subgraphs.len(), 1, "Should have 1 control block subgraph");
        assert!(layout.subgraphs[0].width > 0.0);
        assert!(layout.subgraphs[0].height > 0.0);
    }

    #[test]
    fn test_layout_empty_diagram() {
        let ast = parse_mermaid("sequenceDiagram").unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
    }

    #[test]
    fn test_layout_self_referencing() {
        let ast = parse_mermaid("sequenceDiagram\n    A->>A: Self call").unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        // Self-referencing message should have 4 points (loop)
        assert_eq!(layout.edges[0].points.len(), 4, "Self-ref should create loop path");
    }

    #[test]
    fn test_layout_with_rect() {
        let input = "sequenceDiagram\n\
            participant A\n\
            participant B\n\
            rect rgb(200, 150, 100)\n\
                A->>B: Message\n\
            end";
        let ast = parse_mermaid(input).unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        assert!(layout.subgraphs.len() >= 1, "Should have at least 1 subgraph for rect");
    }

    #[test]
    fn test_layout_lifelines() {
        let ast = parse_mermaid("sequenceDiagram\n    A->>B: Hello").unwrap();
        let layout = compute_sequence_layout(&ast, &DefaultTheme, &LayoutConfig::default());
        // 1 message + 2 lifelines = 3 edges
        assert_eq!(layout.edges.len(), 3);
        // Lifelines should be vertical (same x for both points)
        let lifeline_a = &layout.edges[1];
        assert_eq!(lifeline_a.points[0].0, lifeline_a.points[1].0,
            "Lifeline should be vertical");
    }
}
