//! Sequence 渲染器
//!
//! 将 Layout 转换为 Sequence Diagram 的 Canvas 2D 指令。
//! 渲染层次：Background → Subgraphs (控制块/rect) → Edges (生命线+消息箭头)
//! → Nodes (参与者头+激活框) → Labels → Title（T15）

use mermaid_canvas_core::{
    instruction::{CmdDecor, CmdShadow, DrawCmd, PathSegment, RenderOutput},
    layer::LayerKind,
    style::{FillStyle, StrokeStyle, TextStyle, TextAnchor, TextBaseline},
    NodeShape,
};

use crate::diagram::arrow;
use crate::diagram::DiagramOutput;
use crate::error::ComponentError;
use crate::layout::{EdgeLayout, Layout, NodeLayout};
use crate::theme::{vertical_gradient_fill, with_color_alpha, Theme};

/// 序列图渲染器
pub struct SequenceRenderer;

impl SequenceRenderer {
    /// 渲染序列图
    pub fn render<T: Theme>(
        layout: &Layout,
        theme: &T,
    ) -> Result<DiagramOutput, ComponentError> {
        let mut layers = mermaid_canvas_core::layer::RenderLayers::new();

        // Separate activation boxes from participant nodes（按 BTreeMap key —
        // 与 convert::layout_to_hit_regions 的过滤口径一致：key 序 = 命中区序）
        let (participant_nodes, activation_nodes): (Vec<(&String, &NodeLayout)>, Vec<(&String, &NodeLayout)>) = layout
            .nodes
            .iter()
            .partition(|(key, _)| !key.starts_with("__act_"));

        // Separate lifelines from message edges（保留布局边索引 — 关联聚焦分组键）
        let (lifelines, messages): (Vec<_>, Vec<_>) = layout
            .edges
            .iter()
            .enumerate()
            .partition(|(_, el)| el.to.ends_with("_lifeline_end"));

        // 1. Background layer（R7 层次:底色 + preset 网格/提光）
        let mut bg_commands = vec![DrawCmd::Rect {
            x: 0.0,
            y: 0.0,
            width: layout.width,
            height: layout.height,
            fill: Some(FillStyle::Color(theme.background_color().to_string())),
            stroke: None,
            corner_radius: None,
        }];
        let bg_preset = theme.style_preset();
        if let Some((spacing, alpha)) = bg_preset.fine_grid() {
            bg_commands.push(super::grid_cmd(layout, theme, spacing, alpha));
        }
        if let Some((spacing, alpha)) = bg_preset.major_grid() {
            bg_commands.push(super::grid_cmd(layout, theme, spacing, alpha));
        }
        if bg_preset.top_light() > 0.0 {
            bg_commands.push(super::top_light_cmd(layout, theme, bg_preset.top_light()));
        }
        layers.update_layer(LayerKind::Background, RenderOutput::from_commands(bg_commands));

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

        // Lifelines as dashed vertical lines（T12 — Dashed 线型过 decor；携带边索引）
        let preset = theme.style_preset();
        for (ei, ll) in &lifelines {
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
            let mut decor = arrow::edge_style_decor(ll.style);
            if decor.stroke_width.is_none() {
                decor.stroke_width = Some(preset.stroke_width());
            }
            let cmds = super::stamp_edge_id(
                vec![DrawCmd::Decorated {
                    inner: Box::new(DrawCmd::Path {
                        segments,
                        fill: None,
                        stroke: Some(StrokeStyle::Color(theme.edge_color().to_string())),
                    }),
                    decor,
                }],
                *ei as u32,
            );
            edge_commands.extend(cmds);
        }

        // Message arrows（T11 — 实心/开放/叉号箭头 + T12 线型；携带边索引）
        for (ei, msg) in &messages {
            edge_commands.extend(super::stamp_edge_id(
                Self::render_message(msg, theme),
                *ei as u32,
            ));
        }

        layers.update_layer(
            LayerKind::Edges,
            RenderOutput::from_commands(edge_commands),
        );

        // 4. Nodes layer — participant headers + activation boxes
        // 参与者 decor.id = hit-region index（过滤 BTreeMap 序）；激活框非命中区
        let preset = theme.style_preset();
        let mut node_commands = Vec::new();
        for (idx, (_, nl)) in participant_nodes.iter().enumerate() {
            node_commands.extend(Self::participant_card(nl, theme, Some(idx as u32)));
        }

        // Activation boxes
        for (_, nl) in &activation_nodes {
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
        for (_, nl) in &participant_nodes {
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

        // Message labels（T19 — 边标签字号通道;R7 圆角底盘垫底）
        for (_, msg) in &messages {
            if let Some(ref label) = msg.label {
                let text_style = TextStyle::new()
                    .with_font_family(theme.font_family())
                    .with_font_size(theme.edge_label_font_size())
                    .with_fill(FillStyle::Color(theme.edge_color().to_string()));

                let (lx, ly) = msg.label_anchor
                    .unwrap_or_else(|| {
                        let mid = msg.points.len() / 2;
                        msg.points.get(mid).copied().unwrap_or((0.0, 0.0))
                    });

                label_commands.push(super::edge_label_plate(lx, ly - 4.0, label, theme));
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
                    .with_font_size(theme.edge_label_font_size())
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

        // 6. Title layer（T15）
        let title_commands = match &layout.title {
            None => Vec::new(),
            Some(tb) => {
                let text_style = TextStyle::new()
                    .with_font_family(theme.font_family())
                    .with_font_size(theme.title_font_size())
                    .with_fill(FillStyle::Color(theme.title_color().to_string()));
                vec![DrawCmd::Text {
                    x: tb.x,
                    y: tb.y,
                    content: tb.text.clone(),
                    style: text_style,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                }]
            }
        };
        layers.update_layer(LayerKind::Title, RenderOutput::from_commands(title_commands));

        Ok(DiagramOutput {
            layers,
            hit_regions: Vec::new(),
        })
    }

    /// Render a participant header box（tint 填充 + 槽位描边 + preset 圆角/渐变）
    fn render_participant_header<T: Theme>(nl: &NodeLayout, theme: &T) -> DrawCmd {
        let preset = theme.style_preset();
        let base_fill = theme.node_fill(&nl.shape);
        let fill = if preset.gradient_fill() {
            vertical_gradient_fill(&base_fill, nl.x, nl.y, nl.width, nl.height, preset.gradient_range())
        } else {
            base_fill
        };
        let stroke = StrokeStyle::Color(theme.node_stroke_for(&nl.shape));

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
                corner_radius: Some(preset.corner_radius()),
            },
        }
    }

    /// 参与者头卡片族 — 软阴影(随主体指令 shadow 字段) + 主体 + 内侧高光
    /// bevel(R10 对齐流程图节点:阴影与主体同一动画单位)
    fn participant_card<T: Theme>(nl: &NodeLayout, theme: &T, hit_id: Option<u32>) -> Vec<DrawCmd> {
        let preset = theme.style_preset();
        let header = Self::render_participant_header(nl, theme);
        let mut commands = Vec::new();
        let mut decor = CmdDecor {
            id: hit_id,
            stroke_width: Some(preset.stroke_width()),
            ..Default::default()
        };
        if let Some((dy, blur, spread, alpha)) = preset.node_shadow() {
            decor.shadow = Some(CmdShadow {
                offset_x: 0.0,
                offset_y: dy,
                blur,
                spread,
                color: "#000000".to_string(),
                alpha,
                width: 0.0,
                height: 0.0,
                rotation: 0.0,
            });
        }
        commands.push(DrawCmd::Decorated { inner: Box::new(header.clone()), decor });
        if preset.inset_highlight() {
            let (x, y, w, h, radius) = match &header {
                DrawCmd::Rect { x, y, width, height, corner_radius, .. } => {
                    (*x, *y, *width, *height, corner_radius.unwrap_or(0.0))
                }
                _ => (nl.x, nl.y, nl.width, nl.height, preset.corner_radius()),
            };
            let stroke_hex = match &header {
                DrawCmd::Rect { stroke: Some(StrokeStyle::Color(c)), .. } => c.clone(),
                _ => theme.node_stroke_for(&nl.shape),
            };
            commands.push(DrawCmd::Decorated {
                inner: Box::new(DrawCmd::Rect {
                    x: x + 1.5,
                    y: y + 1.5,
                    width: w - 3.0,
                    height: h - 3.0,
                    fill: None,
                    stroke: Some(StrokeStyle::Color(with_color_alpha(
                        &crate::theme::lighten_color(&stroke_hex, 0.45),
                        0.28,
                    ))),
                    corner_radius: Some((radius - 1.5).max(0.0)),
                }),
                decor: CmdDecor { id: hit_id, stroke_width: Some(1.0), ..Default::default() },
            });
        }
        commands
    }

    /// Render a message — 主线（线型 decor）+ 末端箭头（实心/开放/叉号）
    fn render_message<T: Theme>(el: &EdgeLayout, theme: &T) -> Vec<DrawCmd> {
        let preset = theme.style_preset();
        let color = theme.edge_color().to_string();
        let thick = matches!(el.style, mermaid_canvas_core::EdgeStyle::Thick);
        let mut commands = Vec::new();

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
        // 主线（基线宽 = preset 档位；Thick 覆盖为 2.5）
        let mut decor = arrow::edge_style_decor(el.style);
        if decor.stroke_width.is_none() {
            decor.stroke_width = Some(preset.stroke_width());
        }
        commands.push(DrawCmd::Decorated {
            inner: Box::new(DrawCmd::Path {
                segments,
                fill: None,
                stroke: Some(StrokeStyle::Color(color.clone())),
            }),
            decor,
        });

        // 末端箭头（`->>` 实心 / `->` 开放 / `-x` 叉号；反向箭头绘于起端）
        if el.arrow_end.is_some() || el.end_decoration.is_some() {
            let (tip, dir) = arrow::end_tip_dir(&el.points);
            if let Some(kind) = el.arrow_end {
                commands.push(arrow::arrowhead_cmd(tip, dir, kind, &color, thick));
            }
            if let Some(deco) = el.end_decoration {
                commands.push(arrow::decoration_cmd(tip, dir, deco, &color));
            }
        }
        if el.arrow_start.is_some() || el.start_decoration.is_some() {
            let (tip, dir) = arrow::start_tip_dir(&el.points);
            if let Some(kind) = el.arrow_start {
                commands.push(arrow::arrowhead_cmd(tip, dir, kind, &color, thick));
            }
            if let Some(deco) = el.start_decoration {
                commands.push(arrow::decoration_cmd(tip, dir, deco, &color));
            }
        }

        commands
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

        let mut msg = EdgeLayout::plain("A".to_string(), "B".to_string(), vec![(90.0, 120.0), (210.0, 120.0)], true);
        msg.label = Some(TextBlock {
            text: "Hello".to_string(),
            x: 150.0,
            y: 112.0,
            width: 40.0,
            height: 16.0,
            font_size: 12.0,
        });
        msg.label_anchor = Some((150.0, 112.0));
        let mut ll_a = EdgeLayout::plain("A".to_string(), "A_lifeline_end".to_string(), vec![(90.0, 70.0), (90.0, 300.0)], false);
        ll_a.style = mermaid_canvas_core::EdgeStyle::Dashed;
        let mut ll_b = EdgeLayout::plain("B".to_string(), "B_lifeline_end".to_string(), vec![(250.0, 70.0), (250.0, 300.0)], false);
        ll_b.style = mermaid_canvas_core::EdgeStyle::Dashed;
        let edges = vec![msg, ll_a, ll_b];

        let layout = Layout {
            width: 400.0,
            height: 350.0,
            nodes,
            edges,
            subgraphs: vec![],
            title: None,
        };

        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();

        // Background
        let bg = get_layer_cmds(&result, LayerKind::Background);
        assert!(!bg.is_empty());

        // Nodes（R10 卡片族:2 参与者 × [主体含软阴影+bevel] = 4）
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 4);

        // Edges: 1 message 主线 + 2 lifelines（消息无箭头则仅主线）
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 3);

        // Labels (2 participant + message 底盘 + message 文本 = 4)
        let label_cmds = get_layer_cmds(&result, LayerKind::Labels);
        assert_eq!(label_cmds.len(), 4);
    }

    #[test]
    fn test_message_arrowhead_rendered_per_arrow_kind() {
        // `->>` 实心箭头：主线 + 箭头 = 2 条
        let mut msg = EdgeLayout::plain("A".to_string(), "B".to_string(), vec![(0.0, 50.0), (100.0, 50.0)], true);
        msg.arrow_end = Some(mermaid_canvas_core::EdgeArrowhead::Arrow);
        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes: BTreeMap::new(),
            edges: vec![msg],
            subgraphs: vec![],
            title: None,
        };
        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        assert_eq!(get_layer_cmds(&result, LayerKind::Edges).len(), 2, "主线 + 实心箭头");

        // `-->` 开放箭头（Dashed 线型）
        let mut msg = EdgeLayout::plain("A".to_string(), "B".to_string(), vec![(0.0, 50.0), (100.0, 50.0)], true);
        msg.arrow_end = Some(mermaid_canvas_core::EdgeArrowhead::OpenTriangle);
        msg.style = mermaid_canvas_core::EdgeStyle::Dashed;
        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes: BTreeMap::new(),
            edges: vec![msg],
            subgraphs: vec![],
            title: None,
        };
        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 2);
        match &edge_cmds[0] {
            DrawCmd::Decorated { decor, .. } => assert_eq!(decor.dash, Some(vec![6.0, 4.0]), "-- 线型 dash"),
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_lifeline_carries_dash_decor() {
        let mut ll = EdgeLayout::plain("A".to_string(), "A_lifeline_end".to_string(), vec![(50.0, 0.0), (50.0, 200.0)], false);
        ll.style = mermaid_canvas_core::EdgeStyle::Dashed;
        let layout = Layout {
            width: 100.0,
            height: 200.0,
            nodes: BTreeMap::new(),
            edges: vec![ll],
            subgraphs: vec![],
            title: None,
        };
        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        match &get_layer_cmds(&result, LayerKind::Edges)[0] {
            DrawCmd::Decorated { decor, .. } => assert_eq!(decor.dash, Some(vec![6.0, 4.0])),
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_participant_commands_carry_hit_ids_activation_none() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node("A", 50.0, 30.0, NodeShape::Rectangle));
        nodes.insert("B".to_string(), make_node("B", 210.0, 30.0, NodeShape::Rectangle));
        nodes.insert("__act_2".to_string(), NodeLayout {
            id: "activation_A_1".to_string(),
            x: 42.0,
            y: 90.0,
            width: 16.0,
            height: 60.0,
            label: make_text_block(""),
            shape: NodeShape::Rectangle,
            bounds: BoundingBox::new(42.0, 90.0, 16.0, 60.0),
        });
        let layout = Layout {
            width: 300.0,
            height: 300.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
            title: None,
        };
        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        // R10:2 参与者 × 2 卡片族 + 1 激活框 = 5
        assert_eq!(node_cmds.len(), 5, "2 参与者卡片族 + 1 激活框");
        match &node_cmds[0] {
            DrawCmd::Decorated { decor, .. } => assert_eq!(decor.id, Some(0)),
            other => panic!("expected Decorated, got {:?}", other),
        }
        match &node_cmds[1] {
            DrawCmd::Decorated { decor, .. } => assert_eq!(decor.id, Some(0)),
            other => panic!("expected Decorated, got {:?}", other),
        }
        // 激活框无 id（非命中区）— R10 卡片族后位于第 4 位（2 参与者 × 2）
        assert!(matches!(&node_cmds[4], DrawCmd::Rect { .. }), "激活框为裸 rect");
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
            title: None,
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
            title: None,
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
            title: None,
        };

        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        // R10:1 参与者卡片族(2) + 1 activation box = 3
        assert_eq!(node_cmds.len(), 3);
    }

    #[test]
    fn test_render_empty_layout() {
        let layout = Layout {
            width: 100.0,
            height: 100.0,
            nodes: BTreeMap::new(),
            edges: vec![],
            subgraphs: vec![],
            title: None,
        };

        let result = SequenceRenderer::render(&layout, &DefaultTheme).unwrap();
        let bg = get_layer_cmds(&result, LayerKind::Background);
        assert!(!bg.is_empty());

        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert!(node_cmds.is_empty());
    }
}
