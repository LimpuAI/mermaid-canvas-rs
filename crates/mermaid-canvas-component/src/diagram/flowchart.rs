//! Flowchart 渲染器 — 对标 deneb-component 的 chart/line.rs
//!
//! 将 Layout 转换为 Flowchart 的 Canvas 2D 指令。

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

/// Flowchart 渲染器
pub struct FlowchartRenderer;

impl FlowchartRenderer {
    /// 渲染流程图
    ///
    /// # Arguments
    /// * `layout` - 布局结果
    /// * `theme` - 主题配置
    pub fn render<T: Theme>(
        layout: &Layout,
        theme: &T,
    ) -> Result<DiagramOutput, ComponentError> {
        let mut layers = mermaid_canvas_core::layer::RenderLayers::new();

        // 1. 背景层
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

        // 2. 节点层
        let node_commands: Vec<DrawCmd> = layout
            .nodes
            .values()
            .map(|nl| Self::render_node(nl, theme))
            .collect();
        layers.update_layer(
            LayerKind::Nodes,
            RenderOutput::from_commands(node_commands),
        );

        // 3. 边层
        let edge_commands: Vec<DrawCmd> = layout
            .edges
            .iter()
            .map(|el| Self::render_edge(el, theme))
            .collect();
        layers.update_layer(
            LayerKind::Edges,
            RenderOutput::from_commands(edge_commands),
        );

        // 4. 标签层
        let label_commands = Self::render_labels(layout, theme);
        layers.update_layer(LayerKind::Labels, RenderOutput::from_commands(label_commands));

        Ok(DiagramOutput {
            layers,
            hit_regions: Vec::new(),
        })
    }

    /// 渲染单个节点
    fn render_node<T: Theme>(nl: &NodeLayout, theme: &T) -> DrawCmd {
        let fill = FillStyle::Color(theme.node_fill(&nl.shape).to_color_string());
        let stroke = StrokeStyle::Color(theme.node_stroke().to_string());

        match nl.shape {
            NodeShape::Rectangle | NodeShape::Subroutine => DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(fill),
                stroke: Some(stroke),
                corner_radius: None,
            },
            NodeShape::RoundRect | NodeShape::Stadium => DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(fill),
                stroke: Some(stroke),
                corner_radius: Some(8.0),
            },
            NodeShape::Circle | NodeShape::DoubleCircle => DrawCmd::Circle {
                cx: nl.x + nl.width / 2.0,
                cy: nl.y + nl.height / 2.0,
                r: nl.width.min(nl.height) / 2.0,
                fill: Some(fill),
                stroke: Some(stroke),
            },
            NodeShape::Diamond => {
                let cx = nl.x + nl.width / 2.0;
                let cy = nl.y + nl.height / 2.0;
                let hw = nl.width / 2.0;
                let hh = nl.height / 2.0;
                DrawCmd::Path {
                    segments: vec![
                        PathSegment::MoveTo(cx, cy - hh),
                        PathSegment::LineTo(cx + hw, cy),
                        PathSegment::LineTo(cx, cy + hh),
                        PathSegment::LineTo(cx - hw, cy),
                        PathSegment::Close,
                    ],
                    fill: Some(fill),
                    stroke: Some(stroke),
                }
            }
            _ => DrawCmd::Rect {
                // 默认退化为矩形
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

    /// 渲染单条边
    fn render_edge<T: Theme>(el: &EdgeLayout, theme: &T) -> DrawCmd {
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

    /// 渲染所有标签
    fn render_labels<T: Theme>(layout: &Layout, theme: &T) -> Vec<DrawCmd> {
        let mut commands = Vec::new();

        // 节点标签
        for nl in layout.nodes.values() {
            let text_style = TextStyle::new()
                .with_font_family(theme.font_family())
                .with_font_size(theme.font_size())
                .with_fill(FillStyle::Color(theme.node_text_color().to_string()));

            commands.push(DrawCmd::Text {
                x: nl.x + nl.width / 2.0,
                y: nl.y + nl.height / 2.0,
                content: nl.label.text.clone(),
                style: text_style,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
            });
        }

        // 边标签
        for el in &layout.edges {
            if let Some(ref label) = el.label {
                let text_style = TextStyle::new()
                    .with_font_family(theme.font_family())
                    .with_font_size(theme.font_size() * 0.85)
                    .with_fill(FillStyle::Color(theme.edge_color().to_string()));

                let (lx, ly) = el
                    .label_anchor
                    .unwrap_or_else(|| {
                        // 默认放在中间点
                        let mid = el.points.len() / 2;
                        el.points.get(mid).copied().unwrap_or((0.0, 0.0))
                    });

                commands.push(DrawCmd::Text {
                    x: lx,
                    y: ly,
                    content: label.text.clone(),
                    style: text_style,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Bottom,
                });
            }
        }

        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{EdgeLayout, Layout, NodeLayout, TextBlock};
    use crate::theme::{DarkTheme, DefaultTheme};
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

    fn make_node_layout(id: &str, x: f64, y: f64, shape: NodeShape) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            x,
            y,
            width: 100.0,
            height: 50.0,
            label: make_text_block(id),
            shape,
            bounds: BoundingBox::new(x, y, 100.0, 50.0),
        }
    }

    fn make_edge_layout(from: &str, to: &str, points: Vec<(f64, f64)>) -> EdgeLayout {
        EdgeLayout {
            from: from.to_string(),
            to: to.to_string(),
            points,
            label: None,
            label_anchor: None,
            directed: true,
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
    fn test_render_single_node() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 10.0, 20.0, NodeShape::Rectangle));

        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();

        // Background layer should have a rect
        let bg = get_layer_cmds(&result, LayerKind::Background);
        assert!(!bg.is_empty(), "Background layer should not be empty");

        // Nodes layer should have at least one command
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert!(!node_cmds.is_empty(), "Nodes layer should have commands for the single node");
    }

    #[test]
    fn test_render_two_connected_nodes() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 10.0, 20.0, NodeShape::Rectangle));
        nodes.insert("B".to_string(), make_node_layout("B", 200.0, 20.0, NodeShape::Rectangle));

        let edges = vec![make_edge_layout("A", "B", vec![(60.0, 45.0), (200.0, 45.0)])];

        let layout = Layout {
            width: 400.0,
            height: 100.0,
            nodes,
            edges,
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();

        // Should have node rects for A and B
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 2, "Should have 2 node commands");

        // Should have edge path
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 1, "Should have 1 edge command");

        // Should have labels for nodes
        let label_cmds = get_layer_cmds(&result, LayerKind::Labels);
        assert_eq!(label_cmds.len(), 2, "Should have 2 label commands");
    }

    #[test]
    fn test_render_diamond_shape() {
        let mut nodes = BTreeMap::new();
        nodes.insert("D".to_string(), make_node_layout("D", 50.0, 50.0, NodeShape::Diamond));

        let layout = Layout {
            width: 200.0,
            height: 150.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 1);

        // Diamond renders as a Path, not a Rect
        match &node_cmds[0] {
            DrawCmd::Path { segments, .. } => {
                assert!(segments.len() >= 4, "Diamond should have at least 4 path segments");
            }
            _ => panic!("Expected DrawCmd::Path for diamond shape"),
        }
    }

    #[test]
    fn test_render_circle_shape() {
        let mut nodes = BTreeMap::new();
        nodes.insert("C".to_string(), make_node_layout("C", 30.0, 30.0, NodeShape::Circle));

        let layout = Layout {
            width: 200.0,
            height: 150.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 1);

        // Circle renders as DrawCmd::Circle
        match &node_cmds[0] {
            DrawCmd::Circle { cx, cy, r, .. } => {
                assert!(*cx > 0.0, "Circle cx should be positive");
                assert!(*cy > 0.0, "Circle cy should be positive");
                assert!(*r > 0.0, "Circle radius should be positive");
            }
            _ => panic!("Expected DrawCmd::Circle for circle shape"),
        }
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

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();

        // Background should still exist (it's always drawn)
        let bg = get_layer_cmds(&result, LayerKind::Background);
        assert!(!bg.is_empty(), "Background should still be rendered");

        // No node commands
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert!(node_cmds.is_empty(), "No nodes in empty layout");

        // No edge commands
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert!(edge_cmds.is_empty(), "No edges in empty layout");
    }

    #[test]
    fn test_render_different_themes() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 10.0, 20.0, NodeShape::Rectangle));

        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
        };

        let default_result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let dark_result = FlowchartRenderer::render(&layout, &DarkTheme).unwrap();

        // Background colors should differ
        let default_bg = get_layer_cmds(&default_result, LayerKind::Background);
        let dark_bg = get_layer_cmds(&dark_result, LayerKind::Background);

        // Extract background color from the first Rect
        let get_fill_color = |cmds: &[DrawCmd]| -> Option<String> {
            cmds.iter().find_map(|cmd| match cmd {
                DrawCmd::Rect { fill: Some(FillStyle::Color(c)), .. } => Some(c.clone()),
                _ => None,
            })
        };

        let default_color = get_fill_color(default_bg);
        let dark_color = get_fill_color(dark_bg);
        assert_ne!(default_color, dark_color, "Default and dark themes should produce different background colors");
    }

    #[test]
    fn test_render_edge_with_label() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 10.0, 20.0, NodeShape::Rectangle));
        nodes.insert("B".to_string(), make_node_layout("B", 200.0, 20.0, NodeShape::Rectangle));

        let mut edge = make_edge_layout("A", "B", vec![(60.0, 45.0), (200.0, 45.0)]);
        edge.label = Some(TextBlock {
            text: "yes".to_string(),
            x: 130.0,
            y: 35.0,
            width: 30.0,
            height: 15.0,
            font_size: 12.0,
        });
        edge.label_anchor = Some((130.0, 35.0));

        let layout = Layout {
            width: 400.0,
            height: 100.0,
            nodes,
            edges: vec![edge],
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();

        // Labels layer should have 2 node labels + 1 edge label = 3
        let label_cmds = get_layer_cmds(&result, LayerKind::Labels);
        assert_eq!(label_cmds.len(), 3, "Should have 3 label commands (2 nodes + 1 edge)");

        // At least one label should contain "yes"
        let has_edge_label = label_cmds.iter().any(|cmd| match cmd {
            DrawCmd::Text { content, .. } => content == "yes",
            _ => false,
        });
        assert!(has_edge_label, "Should have edge label 'yes'");
    }

    #[test]
    fn test_render_round_rect_shape() {
        let mut nodes = BTreeMap::new();
        nodes.insert("R".to_string(), make_node_layout("R", 10.0, 10.0, NodeShape::RoundRect));

        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 1);

        // RoundRect should render as Rect with corner_radius
        match &node_cmds[0] {
            DrawCmd::Rect { corner_radius: Some(_), .. } => {}
            _ => panic!("Expected DrawCmd::Rect with corner_radius for RoundRect"),
        }
    }

    #[test]
    fn test_render_hit_regions_empty() {
        let layout = Layout {
            width: 100.0,
            height: 100.0,
            nodes: BTreeMap::new(),
            edges: vec![],
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        assert!(result.hit_regions.is_empty(), "Hit regions should be empty");
    }

    #[test]
    fn test_render_background_dimensions() {
        let layout = Layout {
            width: 500.0,
            height: 300.0,
            nodes: BTreeMap::new(),
            edges: vec![],
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let bg = get_layer_cmds(&result, LayerKind::Background);

        match bg.first() {
            Some(DrawCmd::Rect { width, height, x, y, .. }) => {
                assert_eq!(*x, 0.0);
                assert_eq!(*y, 0.0);
                assert_eq!(*width, 500.0);
                assert_eq!(*height, 300.0);
            }
            _ => panic!("Expected background rect"),
        }
    }

    #[test]
    fn test_render_multiple_edges() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 10.0, 20.0, NodeShape::Rectangle));
        nodes.insert("B".to_string(), make_node_layout("B", 200.0, 20.0, NodeShape::Rectangle));
        nodes.insert("C".to_string(), make_node_layout("C", 10.0, 120.0, NodeShape::Rectangle));

        let edges = vec![
            make_edge_layout("A", "B", vec![(60.0, 45.0), (200.0, 45.0)]),
            make_edge_layout("A", "C", vec![(60.0, 45.0), (60.0, 120.0)]),
        ];

        let layout = Layout {
            width: 400.0,
            height: 200.0,
            nodes,
            edges,
            subgraphs: vec![],
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 2, "Should have 2 edge commands");
    }
}
