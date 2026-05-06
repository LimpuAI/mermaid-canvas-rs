//! Sequence 渲染器
//!
//! 将 Layout 转换为 Sequence Diagram 的 Canvas 2D 指令。
//! 渲染层次：Background → Subgraphs (控制块/rect) → Edges (生命线+消息箭头) → Nodes (参与者头+激活框) → Labels

use mermaid_canvas_core::{
    instruction::{DrawCmd, PathSegment, RenderOutput},
    layer::LayerKind,
    style::{FillStyle, StrokeStyle, TextStyle, TextAnchor, TextBaseline},
    NodeShape,
};

use crate::diagram::DiagramOutput;
use crate::error::ComponentError;
use crate::layout::{EdgeLayout, Layout, NodeLayout};
use crate::theme::Theme;

/// 序列图渲染器
pub struct SequenceRenderer;

impl SequenceRenderer {
    /// 渲染序列图
    pub fn render<T: Theme>(
        layout: &Layout,
        theme: &T,
    ) -> Result<DiagramOutput, ComponentError> {
        let mut layers = mermaid_canvas_core::layer::RenderLayers::new();

        // Separate activation boxes from participant nodes
        let (participant_nodes, activation_nodes): (Vec<_>, Vec<_>) = layout
            .nodes
            .values()
            .partition(|nl| !nl.id.starts_with("__act_"));

        // Separate lifelines from message edges
        let (lifelines, messages): (Vec<_>, Vec<_>) = layout
            .edges
            .iter()
            .partition(|el| el.to.ends_with("_lifeline_end"));

        // 1. Background layer
        let bg_cmd = DrawCmd::Rect {
            x: 0.0,
            y: 0.0,
            width: layout.width,
            height: layout.height,
            fill: Some(FillStyle::Color(theme.background_color().to_string())),
            stroke: None,
            corner_radius: None,
        };
        layers.update_layer(LayerKind::Background, RenderOutput::from_commands(vec![bg_cmd]));

        // 2. Subgraphs layer (control blocks + rect backgrounds)
        let subgraph_commands: Vec<DrawCmd> = layout
            .subgraphs
            .iter()
            .map(|sg| {
                let bg_color = theme.subgraph_background().to_string();
                DrawCmd::Rect {
                    x: sg.x,
                    y: sg.y,
                    width: sg.width,
                    height: sg.height,
                    fill: Some(FillStyle::Color(bg_color)),
                    stroke: Some(StrokeStyle::Color(theme.subgraph_border().to_string())),
                    corner_radius: None,
                }
            })
            .collect();
        layers.update_layer(
            LayerKind::Subgraphs,
            RenderOutput::from_commands(subgraph_commands),
        );

        // 3. Edges layer — lifelines (dashed vertical) + message arrows
        let mut edge_commands = Vec::new();

        // Lifelines as dashed vertical lines
        for ll in &lifelines {
            let segments: Vec<PathSegment> = ll
                .points
                .iter()
                .enumerate()
                .map(|(i, &(x, y))| {
                    if i == 0 {
                        PathSegment::MoveTo(x, y)
                    } else {
                        PathSegment::LineTo(x, y)
                    }
                })
                .collect();
            edge_commands.push(DrawCmd::Path {
                segments,
                fill: None,
                stroke: Some(StrokeStyle::Color(theme.edge_color().to_string())),
            });
        }

        // Message arrows
        for msg in &messages {
            edge_commands.push(Self::render_message(msg, theme));
        }

        layers.update_layer(
            LayerKind::Edges,
            RenderOutput::from_commands(edge_commands),
        );

        // 4. Nodes layer — participant headers + activation boxes
        let mut node_commands = Vec::new();

        // Participant header boxes
        for nl in &participant_nodes {
            node_commands.push(Self::render_participant_header(nl, theme));
        }

        // Activation boxes
        for nl in &activation_nodes {
            node_commands.push(DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(FillStyle::Color(theme.node_fill_color(&NodeShape::Rectangle).to_string())),
                stroke: Some(StrokeStyle::Color(theme.node_stroke().to_string())),
                corner_radius: None,
            });
        }

        layers.update_layer(
            LayerKind::Nodes,
            RenderOutput::from_commands(node_commands),
        );

        // 5. Labels layer — participant names, message text, control block labels
        let mut label_commands = Vec::new();

        // Participant name labels
        for nl in &participant_nodes {
            let text_style = TextStyle::new()
                .with_font_family(theme.font_family())
                .with_font_size(theme.font_size())
                .with_fill(FillStyle::Color(theme.node_text_color().to_string()));

            label_commands.push(DrawCmd::Text {
                x: nl.x + nl.width / 2.0,
                y: nl.y + nl.height / 2.0,
                content: nl.label.text.clone(),
                style: text_style,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
            });
        }

        // Message labels
        for msg in &messages {
            if let Some(ref label) = msg.label {
                let text_style = TextStyle::new()
                    .with_font_family(theme.font_family())
                    .with_font_size(theme.font_size() * 0.85)
                    .with_fill(FillStyle::Color(theme.edge_color().to_string()));

                let (lx, ly) = msg.label_anchor
                    .unwrap_or_else(|| {
                        let mid = msg.points.len() / 2;
                        msg.points.get(mid).copied().unwrap_or((0.0, 0.0))
                    });

                label_commands.push(DrawCmd::Text {
                    x: lx,
                    y: ly - 4.0,
                    content: label.text.clone(),
                    style: text_style,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Bottom,
                });
            }
        }

        // Control block labels
        for sg in &layout.subgraphs {
            if !sg.label.text.is_empty() {
                let text_style = TextStyle::new()
                    .with_font_family(theme.font_family())
                    .with_font_size(theme.font_size() * 0.85)
                    .with_fill(FillStyle::Color(theme.title_color().to_string()));

                label_commands.push(DrawCmd::Text {
                    x: sg.x + 5.0,
                    y: sg.y + 14.0,
                    content: sg.label.text.clone(),
                    style: text_style,
                    anchor: TextAnchor::Start,
                    baseline: TextBaseline::Top,
                });
            }
        }

        layers.update_layer(
            LayerKind::Labels,
            RenderOutput::from_commands(label_commands),
        );

        Ok(DiagramOutput {
            layers,
            hit_regions: Vec::new(),
        })
    }

    /// Render a participant header box
    fn render_participant_header<T: Theme>(nl: &NodeLayout, theme: &T) -> DrawCmd {
        let fill = FillStyle::Color(theme.node_fill_color(&nl.shape).to_string());
        let stroke = StrokeStyle::Color(theme.node_stroke().to_string());

        match nl.shape {
            NodeShape::Circle | NodeShape::DoubleCircle => DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(fill),
                stroke: Some(stroke),
                corner_radius: Some(nl.height / 2.0),
            },
            _ => DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(fill),
                stroke: Some(stroke),
                corner_radius: Some(4.0),
            },
        }
    }

    /// Render a message arrow
    fn render_message<T: Theme>(el: &EdgeLayout, theme: &T) -> DrawCmd {
        let segments: Vec<PathSegment> = el
            .points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                if i == 0 {
                    PathSegment::MoveTo(x, y)
                } else {
                    PathSegment::LineTo(x, y)
                }
            })
            .collect();

        DrawCmd::Path {
            segments,
            fill: None,
            stroke: Some(StrokeStyle::Color(theme.edge_color().to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Layout, NodeLayout, TextBlock};
    use crate::theme::{DefaultTheme, DarkTheme};
    use mermaid_canvas_core::interaction::BoundingBox;
    use mermaid_canvas_core::NodeShape;
    use std::collections::BTreeMap;

    fn make_text_block(text: &str) -> TextBlock {
        TextBlock {
            text: text.to_string(),
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 20.0,
            font_size: 14.0,
        }
    }

    fn make_node(id: &str, x: f64, y: f64, shape: NodeShape) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            x,
            y,
            width: 80.0,
            height: 40.0,
            label: make_text_block(id),
            shape,
            bounds: BoundingBox::new(x, y, 80.0, 40.0),
        }
    }

    fn get_layer_cmds(output: &DiagramOutput, kind: LayerKind) -> &[DrawCmd] {
        output
            .layers
            .get_layer(kind)
            .map(|l| l.commands.semantic.as_slice())
            .unwrap_or(&[])
    }

    #[test]
    fn test_render_basic_sequence() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 30.0, NodeShape::Rectangle));
        nodes.insert("B".to_string(), make_node("B", 210.0, 30.0, NodeShape::Rectangle));

        let edges = vec![
            EdgeLayout {
                from: "A".to_string(),
                to: "B".to_string(),
                points: vec![(90.0, 120.0), (210.0, 120.0)],
                label: Some(TextBlock {
                    text: "Hello".to_string(),
                    x: 150.0,
                    y: 112.0,
                    width: 40.0,
                    height: 16.0,
                    font_size: 12.0,
                }),
                label_anchor: Some((150.0, 112.0)),
                directed: true,
            },
            EdgeLayout {
                from: "A".to_string(),
                to: "A_lifeline_end".to_string(),
                points: vec![(90.0, 70.0), (90.0, 300.0)],
                label: None,
                label_anchor: None,
                directed: false,
            },
            EdgeLayout {
                from: "B".to_string(),
                to: "B_lifeline_end".to_string(),
                points: vec![(250.0, 70.0), (250.0, 300.0)],
                label: None,
                label_anchor: None,
                directed: false,
            },
        ];

        let layout = Layout {
            width: 400.0,
            height: 350.0,
            nodes,
            edges,
            subgraphs: vec![],
        };

        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();

        // Background
        let bg = get_layer_cmds(&result, LayerKind::Background);
        assert!(!bg.is_empty());

        // Nodes
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 2);

        // Edges (1 message + 2 lifelines)
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 3);

        // Labels (2 participant + 1 message)
        let label_cmds = get_layer_cmds(&result, LayerKind::Labels);
        assert_eq!(label_cmds.len(), 3);
    }

    #[test]
    fn test_render_with_control_block() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 30.0, NodeShape::Rectangle));

        let layout = Layout {
            width: 300.0,
            height: 300.0,
            nodes,
            edges: vec![],
            subgraphs: vec![crate::layout::SubgraphLayout {
                id: "cb_0".to_string(),
                label: TextBlock {
                    text: "[Loop] Every second".to_string(),
                    x: 35.0,
                    y: 102.0,
                    width: 160.0,
                    height: 16.0,
                    font_size: 12.0,
                },
                x: 30.0,
                y: 100.0,
                width: 240.0,
                height: 80.0,
            }],
        };

        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();

        let subgraph_cmds = get_layer_cmds(&result, LayerKind::Subgraphs);
        assert_eq!(subgraph_cmds.len(), 1);

        // Should have a label for the control block
        let label_cmds = get_layer_cmds(&result, LayerKind::Labels);
        assert!(label_cmds.iter().any(|cmd| {
            matches!(cmd, DrawCmd::Text { content, .. } if content.contains("Loop"))
        }));
    }

    #[test]
    fn test_render_different_themes() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 30.0, NodeShape::Rectangle));

        let layout = Layout {
            width: 200.0,
            height: 200.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
        };

        let default_result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        let dark_result = SequenceRenderer::render(&layout, &DarkTheme).unwrap();

        let default_bg = get_layer_cmds(&default_result, LayerKind::Background);
        let dark_bg = get_layer_cmds(&dark_result, LayerKind::Background);

        let get_fill = |cmds: &[DrawCmd]| -> Option<String> {
            cmds.iter().find_map(|cmd| match cmd {
                DrawCmd::Rect { fill: Some(FillStyle::Color(c)), .. } => Some(c.clone()),
                _ => None,
            })
        };

        assert_ne!(get_fill(default_bg), get_fill(dark_bg));
    }

    #[test]
    fn test_render_activation_box() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 30.0, NodeShape::Rectangle));
        nodes.insert("__act_2".to_string(), NodeLayout {
            id: "activation_A_1".to_string(),
            x: 82.0,
            y: 90.0,
            width: 16.0,
            height: 60.0,
            label: make_text_block(""),
            shape: NodeShape::Rectangle,
            bounds: BoundingBox::new(82.0, 90.0, 16.0, 60.0),
        });

        let layout = Layout {
            width: 300.0,
            height: 300.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
        };

        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        // 1 participant header + 1 activation box = 2
        assert_eq!(node_cmds.len(), 2);
    }

    #[test]
    fn test_render_empty_layout() {
        let layout = Layout {
            width: 100.0,
            height: 100.0,
            nodes: BTreeMap::new(),
            edges: vec![],
            subgraphs: vec![],
        };

        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        let bg = get_layer_cmds(&result, LayerKind::Background);
        assert!(!bg.is_empty());

        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert!(node_cmds.is_empty());
    }
}
