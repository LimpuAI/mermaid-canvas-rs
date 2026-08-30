//! Flowchart 渲染器 — 对标 deneb-component 的 chart/line.rs
//!
//! 将 Layout 转换为 Flowchart 的 Canvas 2D 指令。
//! 渲染层次：Background → Subgraphs（T14 子图框）→ Edges（T11 箭头 + T12 线型）
//! → Nodes（T13 真实几何 + hit-region id 接线）→ Labels → Title（T15）。

use mermaid_canvas_core::{
    instruction::{CmdDecor, CmdShadow, DrawCmd, PathSegment, RenderOutput},
    layer::LayerKind,
    style::{FillStyle, Gradient, GradientKind, GradientStop, StrokeStyle, TextStyle, TextAnchor, TextBaseline},
    NodeShape,
};

use crate::diagram::arrow;
use crate::diagram::DiagramOutput;
use crate::error::ComponentError;
use crate::layout::{EdgeLayout, Layout, NodeLayout, SubgraphLayout};
use crate::preset::StylePreset;
use crate::sigil;
use crate::theme::{lighten_color, vertical_gradient_fill, with_color_alpha, Theme};

/// 三次贝塞尔近似椭圆的 kappa 系数
const ELLIPSE_KAPPA: f64 = 0.552_284_749_830_898;

/// 路径段整体平移（柔影轮廓偏移用）
/// 提取指令描边色 hex（bevel 基色;无描边回落中性灰）
fn stroke_color_of(cmd: &DrawCmd) -> String {
    match cmd {
        DrawCmd::Rect { stroke: Some(StrokeStyle::Color(c)), .. }
        | DrawCmd::Circle { stroke: Some(StrokeStyle::Color(c)), .. }
        | DrawCmd::Path { stroke: Some(StrokeStyle::Color(c)), .. } => c.clone(),
        _ => "#888888".to_string(),
    }
}

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
        let preset = theme.style_preset();

        // 1. 背景层（R7 层次:底色 + 双尺度网格 + 顶部提光渐变）
        let mut bg_commands = vec![DrawCmd::Rect {
            x: 0.0,
            y: 0.0,
            width: layout.width,
            height: layout.height,
            fill: Some(FillStyle::Color(theme.background_color().to_string())),
            stroke: None,
            corner_radius: None,
        }];
        // 细格 → 主格（后画覆盖其上,双尺度工程感）
        if let Some((spacing, alpha)) = preset.fine_grid() {
            bg_commands.push(super::grid_cmd(layout, theme, spacing, alpha));
        }
        if let Some((spacing, alpha)) = preset.major_grid() {
            bg_commands.push(super::grid_cmd(layout, theme, spacing, alpha));
        }
        if preset.top_light() > 0.0 {
            bg_commands.push(super::top_light_cmd(layout, theme, preset.top_light()));
        }
        layers.update_layer(LayerKind::Background, RenderOutput::from_commands(bg_commands));

        // 2. 子图层（T14）— 面积降序：外框先画，嵌套内框覆盖其上
        let subgraph_commands = Self::render_subgraphs(&layout.subgraphs, theme);
        layers.update_layer(
            LayerKind::Subgraphs,
            RenderOutput::from_commands(subgraph_commands),
        );

        // 3. 边层（T11 箭头 + T12 线型 + 端点装饰；组内指令携带边索引 — 关联聚焦分组键）
        let edge_commands: Vec<DrawCmd> = layout
            .edges
            .iter()
            .enumerate()
            .flat_map(|(ei, el)| super::stamp_edge_id(Self::render_edge(el, theme), ei as u32))
            .collect();
        layers.update_layer(
            LayerKind::Edges,
            RenderOutput::from_commands(edge_commands),
        );

        // 4. 节点层（T13 真实几何；decor.id = hit-region index — 与
        // convert::layout_to_hit_regions 的 BTreeMap key 过滤口径一致）
        let node_commands: Vec<DrawCmd> = layout
            .nodes
            .iter()
            .filter(|(key, _)| !key.starts_with("__act_"))
            .enumerate()
            .flat_map(|(idx, (_, nl))| Self::render_node(nl, theme, Some(idx as u32)))
            .collect();
        layers.update_layer(
            LayerKind::Nodes,
            RenderOutput::from_commands(node_commands),
        );

        // 5. 标签层
        let label_commands = Self::render_labels(layout, theme);
        layers.update_layer(LayerKind::Labels, RenderOutput::from_commands(label_commands));

        // 6. 标题层（T15 — 顶部居中，title_font_size/title_color）
        let title_commands = Self::render_title(layout, theme);
        layers.update_layer(LayerKind::Title, RenderOutput::from_commands(title_commands));

        Ok(DiagramOutput {
            layers,
            hit_regions: Vec::new(),
        })
    }

    /// 渲染子图边界框（圆角矩形 + 背景填充 + 标题文字）
    fn render_subgraphs<T: Theme>(subgraphs: &[SubgraphLayout], theme: &T) -> Vec<DrawCmd> {
        let preset = theme.style_preset();
        let mut ordered: Vec<&SubgraphLayout> = subgraphs.iter().collect();
        ordered.sort_by(|a, b| {
            (b.width * b.height)
                .partial_cmp(&(a.width * a.height))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut commands = Vec::new();
        for sg in ordered {
            commands.push(DrawCmd::Rect {
                x: sg.x,
                y: sg.y,
                width: sg.width,
                height: sg.height,
                fill: Some(FillStyle::Color(theme.subgraph_background().to_string())),
                stroke: Some(StrokeStyle::Color(theme.subgraph_border().to_string())),
                corner_radius: Some(preset.corner_radius()),
            });
            if !sg.label.text.is_empty() {
                let text_style = TextStyle::new()
                    .with_font_family(theme.font_family())
                    .with_font_size(theme.edge_label_font_size())
                    .with_fill(FillStyle::Color(theme.title_color().to_string()));
                commands.push(DrawCmd::Text {
                    x: sg.label.x,
                    y: sg.label.y,
                    content: sg.label.text.clone(),
                    style: text_style,
                    anchor: TextAnchor::Start,
                    baseline: TextBaseline::Middle,
                });
            }
        }
        commands
    }

    /// 渲染单个节点 — 返回指令族（柔影/内高光/复合形状/sigil 附加产出多条）
    fn render_node<T: Theme>(nl: &NodeLayout, theme: &T, hit_id: Option<u32>) -> Vec<DrawCmd> {
        let preset = theme.style_preset();
        // tint 填充（T17）；渐变光照（R7 — 顶部提亮 range 档位随 preset）
        let base_fill = theme.node_fill(&nl.shape);
        let fill = if preset.gradient_fill() {
            vertical_gradient_fill(&base_fill, nl.x, nl.y, nl.width, nl.height, preset.gradient_range())
        } else {
            base_fill
        };
        // 描边 = 语义槽位对比色（T17）
        let stroke = StrokeStyle::Color(theme.node_stroke_for(&nl.shape));
        let decor = CmdDecor {
            id: hit_id,
            stroke_width: Some(preset.stroke_width()),
            ..Default::default()
        };

        let shape = match nl.shape {
            NodeShape::Rectangle | NodeShape::Subroutine => DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(fill),
                stroke: Some(stroke),
                corner_radius: None,
            },
            NodeShape::RoundRect => DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(fill),
                stroke: Some(stroke),
                corner_radius: Some(preset.corner_radius()),
            },
            NodeShape::Stadium => DrawCmd::Rect {
                x: nl.x,
                y: nl.y,
                width: nl.width,
                height: nl.height,
                fill: Some(fill),
                stroke: Some(stroke),
                // 体育场形 = 全圆角（半径 = 高度一半；preset 圆角不适用于全圆形状）
                corner_radius: Some(nl.height / 2.0),
            },
            NodeShape::Circle => DrawCmd::Circle {
                cx: nl.x + nl.width / 2.0,
                cy: nl.y + nl.height / 2.0,
                r: nl.width.min(nl.height) / 2.0,
                fill: Some(fill),
                stroke: Some(stroke),
            },
            NodeShape::DoubleCircle => {
                // 双同心圆（T13）— 外圆 + 内圆（间距 4px）
                let cx = nl.x + nl.width / 2.0;
                let cy = nl.y + nl.height / 2.0;
                let outer = nl.width.min(nl.height) / 2.0;
                let inner = (outer - 4.0).max(2.0);
                DrawCmd::Group {
                    label: None,
                    items: vec![
                        DrawCmd::Circle { cx, cy, r: outer, fill: Some(fill), stroke: Some(stroke.clone()) },
                        DrawCmd::Circle { cx, cy, r: inner, fill: None, stroke: Some(stroke) },
                    ],
                }
            }
            NodeShape::Diamond => {
                let (cx, cy) = (nl.x + nl.width / 2.0, nl.y + nl.height / 2.0);
                let (hw, hh) = (nl.width / 2.0, nl.height / 2.0);
                Self::closed_path(
                    vec![(cx, cy - hh), (cx + hw, cy), (cx, cy + hh), (cx - hw, cy)],
                    Some(fill),
                    Some(stroke),
                )
            }
            NodeShape::Hexagon => {
                // 六边形（T13）— 左右尖端收进
                let inset = (nl.width * 0.18).min(24.0);
                let (l, r) = (nl.x, nl.x + nl.width);
                let (t, b) = (nl.y, nl.y + nl.height);
                let cy = nl.y + nl.height / 2.0;
                Self::closed_path(
                    vec![(l, cy), (l + inset, t), (r - inset, t), (r, cy), (r - inset, b), (l + inset, b)],
                    Some(fill),
                    Some(stroke),
                )
            }
            NodeShape::Cylinder => Self::cylinder_path(nl, fill, stroke),
            NodeShape::Parallelogram => {
                // 平行四边形（T13）— 上下边反向斜切
                let slant = (nl.width * 0.15).min(16.0);
                let (l, r) = (nl.x, nl.x + nl.width);
                let (t, b) = (nl.y, nl.y + nl.height);
                Self::closed_path(
                    vec![(l + slant, t), (r, t), (r - slant, b), (l, b)],
                    Some(fill),
                    Some(stroke),
                )
            }
            NodeShape::Trapezoid => {
                // 梯形（T13）— 上短下长
                let slant = (nl.width * 0.15).min(16.0);
                let (l, r) = (nl.x, nl.x + nl.width);
                let (t, b) = (nl.y, nl.y + nl.height);
                Self::closed_path(
                    vec![(l + slant, t), (r - slant, t), (r, b), (l, b)],
                    Some(fill),
                    Some(stroke),
                )
            }
            NodeShape::Asymmetric => {
                // 不对称形（T13）— 右侧下端斜收（旗形）
                let slant = (nl.width * 0.2).min(20.0);
                let (l, r) = (nl.x, nl.x + nl.width);
                let (t, b) = (nl.y, nl.y + nl.height);
                Self::closed_path(
                    vec![(l, t), (r, t), (r - slant, b), (l, b)],
                    Some(fill),
                    Some(stroke),
                )
            }
        };

        // R10 光影卡片:软阴影(SDF 高斯,随主体指令的 shadow 字段)→ 主体 →
        // 内侧高光 bevel → sigil。阴影挂在主体指令上 = 与主体同一动画单位
        // (入场缩放/淡入、hover 聚焦联动天然同步,无家族相位分裂)
        let mut commands = Vec::new();
        let mut decor = decor;
        if let Some((dy, blur, spread, alpha)) = preset.node_shadow() {
            // 菱形 = 正方形旋转 45°:阴影显式声明内接正方形(对角线 = 短边)
            // + rotation 45,宿主旋转采样得到真实菱形轮廓阴影;其余形状
            // (0,0,0) = 宿主按包围盒近似(柔影下形状差异可辨度低)
            let (sw, sh, rot) = match nl.shape {
                NodeShape::Diamond => {
                    let side = nl.width.min(nl.height) * 0.707_1;
                    (side, side, 45.0)
                }
                _ => (0.0, 0.0, 0.0),
            };
            decor.shadow = Some(CmdShadow {
                offset_x: 0.0,
                offset_y: dy,
                blur,
                spread,
                color: "#000000".to_string(),
                alpha,
                width: sw,
                height: sh,
                rotation: rot,
            });
        }
        commands.push(DrawCmd::Decorated { inner: Box::new(shape.clone()), decor });
        if preset.inset_highlight() {
            if let Some(bevel) = Self::inset_highlight_cmd(nl, &stroke_color_of(&shape), preset) {
                commands.push(DrawCmd::Decorated {
                    inner: Box::new(bevel),
                    decor: CmdDecor {
                        id: hit_id,
                        stroke_width: Some(1.0),
                        ..Default::default()
                    },
                });
            }
        }
        // 语义 sigil（T18 — preset 档位门控；颜色 = 节点描边色）
        if preset.sigils() {
            if let Some(sigil) = sigil::sigil_cmd(
                nl.shape,
                sigil::sigil_origin(nl),
                &theme.node_stroke_for(&nl.shape),
                hit_id,
            ) {
                commands.push(sigil);
            }
        }
        commands
    }

    /// 内侧高光 bevel — rect 族/circle 内缩 1.5px 的亮 hairline(卡片 crisp 边)
    fn inset_highlight_cmd(nl: &NodeLayout, stroke_color: &str, preset: StylePreset) -> Option<DrawCmd> {
        let hi = with_color_alpha(&lighten_color(stroke_color, 0.45), 0.28);
        match nl.shape {
            NodeShape::Rectangle
            | NodeShape::RoundRect
            | NodeShape::Stadium
            | NodeShape::Subroutine => Some(DrawCmd::Rect {
                x: nl.x + 1.5,
                y: nl.y + 1.5,
                width: nl.width - 3.0,
                height: nl.height - 3.0,
                fill: None,
                stroke: Some(StrokeStyle::Color(hi)),
                corner_radius: Some((preset.corner_radius() - 1.5).max(0.0)),
            }),
            NodeShape::Circle | NodeShape::DoubleCircle => Some(DrawCmd::Circle {
                cx: nl.x + nl.width / 2.0,
                cy: nl.y + nl.height / 2.0,
                r: (nl.width.min(nl.height) / 2.0 - 2.0).max(2.0),
                fill: None,
                stroke: Some(StrokeStyle::Color(hi)),
            }),
            _ => None,
        }
    }

    /// 闭合多边形 path
    fn closed_path(points: Vec<(f64, f64)>, fill: Option<FillStyle>, stroke: Option<StrokeStyle>) -> DrawCmd {
        let mut segments = Vec::with_capacity(points.len() + 1);
        let mut iter = points.into_iter();
        if let Some((x, y)) = iter.next() {
            segments.push(PathSegment::MoveTo(x, y));
            for (x, y) in iter {
                segments.push(PathSegment::LineTo(x, y));
            }
            segments.push(PathSegment::Close);
        }
        DrawCmd::Path { segments, fill, stroke }
    }

    /// 圆柱体（T13）— 上下椭圆弧 + 侧线（贝塞尔近似椭圆）
    fn cylinder_path(nl: &NodeLayout, fill: FillStyle, stroke: StrokeStyle) -> DrawCmd {
        let rx = nl.width / 2.0;
        let ry = (nl.height * 0.18).min(12.0);
        let cx = nl.x + rx;
        let k = ELLIPSE_KAPPA * ry;
        let (l, r) = (nl.x, nl.x + nl.width);
        let top_cy = nl.y + ry;
        let bot_cy = nl.y + nl.height - ry;

        DrawCmd::Path {
            segments: vec![
                // 上盖：左 → 顶 → 右（半椭圆）
                PathSegment::MoveTo(l, top_cy),
                PathSegment::BezierTo(l, top_cy - k, cx - rx * ELLIPSE_KAPPA, nl.y, cx, nl.y),
                PathSegment::BezierTo(cx + rx * ELLIPSE_KAPPA, nl.y, r, top_cy - k, r, top_cy),
                // 右侧线
                PathSegment::LineTo(r, bot_cy),
                // 下盖：右 → 底 → 左（半椭圆）
                PathSegment::BezierTo(r, bot_cy + k, cx + rx * ELLIPSE_KAPPA, nl.y + nl.height, cx, nl.y + nl.height),
                PathSegment::BezierTo(cx - rx * ELLIPSE_KAPPA, nl.y + nl.height, l, bot_cy + k, l, bot_cy),
                PathSegment::Close,
            ],
            fill: Some(fill),
            stroke: Some(stroke),
        }
    }

    /// 渲染单条边 — 辉光层（SignalFlow）+ 主线（线型 decor）+ 箭头 + 端点装饰
    fn render_edge<T: Theme>(el: &EdgeLayout, theme: &T) -> Vec<DrawCmd> {
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

        // 辉光层（SignalFlow — 沿边路径多层半透明描边，宽度递增从外到内先画宽层）
        if preset.edge_glow() {
            for &(width, alpha) in preset.glow_layers().iter().rev() {
                commands.push(DrawCmd::Decorated {
                    inner: Box::new(DrawCmd::Path {
                        segments: segments.clone(),
                        fill: None,
                        stroke: Some(StrokeStyle::Color(with_color_alpha(&color, alpha))),
                    }),
                    decor: CmdDecor {
                        stroke_width: Some(width),
                        line_cap: Some("round".to_string()),
                        ..Default::default()
                    },
                });
            }
        }

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

        // 末端箭头（按路由末段方向）与端点装饰
        if el.arrow_end.is_some() || el.end_decoration.is_some() {
            let (tip, dir) = arrow::end_tip_dir(&el.points);
            if let Some(kind) = el.arrow_end {
                // R7 箭头辉光:半透明放大副本垫底(SignalFlow)
                if preset.arrow_glow() {
                    commands.push(arrow::arrowhead_cmd(
                        tip,
                        dir,
                        kind,
                        &with_color_alpha(&color, 0.22),
                        true,
                    ));
                }
                commands.push(arrow::arrowhead_cmd(tip, dir, kind, &color, thick));
            }
            if let Some(deco) = el.end_decoration {
                commands.push(arrow::decoration_cmd(tip, dir, deco, &color));
            }
        }

        // 起端箭头与装饰（按首段方向指向起点）
        if el.arrow_start.is_some() || el.start_decoration.is_some() {
            let (tip, dir) = arrow::start_tip_dir(&el.points);
            if let Some(kind) = el.arrow_start {
                if preset.arrow_glow() {
                    commands.push(arrow::arrowhead_cmd(
                        tip,
                        dir,
                        kind,
                        &with_color_alpha(&color, 0.22),
                        true,
                    ));
                }
                commands.push(arrow::arrowhead_cmd(tip, dir, kind, &color, thick));
            }
            if let Some(deco) = el.start_decoration {
                commands.push(arrow::decoration_cmd(tip, dir, deco, &color));
            }
        }

        commands
    }

    /// 渲染所有标签（节点标签 base 字号 / 边标签 0.85x — T19 字号层级）
    fn render_labels<T: Theme>(layout: &Layout, theme: &T) -> Vec<DrawCmd> {
        let mut commands = Vec::new();

        // 节点标签（跳过内部结构节点 — BTreeMap key 口径）
        for (key, nl) in &layout.nodes {
            if key.starts_with("__act_") {
                continue;
            }
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

        // 边标签（R7 — 圆角底盘垫底,线上一线之隔的可读性）
        for el in &layout.edges {
            if let Some(ref label) = el.label {
                let text_style = TextStyle::new()
                    .with_font_family(theme.font_family())
                    .with_font_size(theme.edge_label_font_size())
                    .with_fill(FillStyle::Color(theme.edge_color().to_string()));

                let (lx, ly) = el
                    .label_anchor
                    .unwrap_or_else(|| {
                        // 默认放在中间点
                        let mid = el.points.len() / 2;
                        el.points.get(mid).copied().unwrap_or((0.0, 0.0))
                    });

                commands.push(super::edge_label_plate(lx, ly, label, theme));
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

    /// 渲染标题（T15 — title 层）
    fn render_title<T: Theme>(layout: &Layout, theme: &T) -> Vec<DrawCmd> {
        match &layout.title {
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
        }
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
        EdgeLayout::plain(from.to_string(), to.to_string(), points, true)
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
            title: None,
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
            title: None,
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();

        // Should have node cmds for A and B (R10 卡片:主体含软阴影 + bevel = 2/节点)
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 4, "2 节点 × 2 指令卡片族");

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
            title: None,
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 1, "主体(含软阴影;路径形无 bevel)");

        // Diamond renders as a Path, not a Rect
        match &node_cmds[0] {
            DrawCmd::Decorated { inner, .. } => {
                assert!(matches!(inner.as_ref(), DrawCmd::Path { segments, .. } if segments.len() >= 4));
            }
            _ => panic!("Expected Decorated Path for diamond shape"),
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
            title: None,
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 2, "主体(含软阴影) + bevel");

        // Circle renders as DrawCmd::Circle（经 Decorated 包装）
        match &node_cmds[0] {
            DrawCmd::Decorated { inner, .. } => match inner.as_ref() {
                DrawCmd::Circle { cx, cy, r, .. } => {
                    assert!(*cx > 0.0, "Circle cx should be positive");
                    assert!(*cy > 0.0, "Circle cy should be positive");
                    assert!(*r > 0.0, "Circle radius should be positive");
                }
                other => panic!("Expected DrawCmd::Circle, got {:?}", other),
            },
            _ => panic!("Expected Decorated wrapper for circle shape"),
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
            title: None,
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
            title: None,
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
            title: None,
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();

        // Labels layer: 2 node labels + 边标签底盘 + 边标签 = 4 (R7)
        let label_cmds = get_layer_cmds(&result, LayerKind::Labels);
        assert_eq!(label_cmds.len(), 4, "2 节点标签 + 底盘 + 边标签");

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
            title: None,
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        assert_eq!(node_cmds.len(), 2, "主体(含软阴影) + bevel");

        // RoundRect should render as Rect with corner_radius（经 Decorated 包装）
        match &node_cmds[0] {
            DrawCmd::Decorated { inner, .. } => {
                assert!(matches!(inner.as_ref(), DrawCmd::Rect { corner_radius: Some(_), .. }));
            }
            _ => panic!("Expected Decorated Rect with corner_radius for RoundRect"),
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
            title: None,
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
            title: None,
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
            title: None,
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 2, "Should have 2 edge commands");
    }

    // ─── T11: 箭头 ─────────────────────────────────────────

    #[test]
    fn test_directed_edge_renders_arrowhead() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 10.0, 20.0, NodeShape::Rectangle));
        nodes.insert("B".to_string(), make_node_layout("B", 200.0, 20.0, NodeShape::Rectangle));

        let mut edge = make_edge_layout("A", "B", vec![(60.0, 45.0), (200.0, 45.0)]);
        edge.arrow_end = Some(mermaid_canvas_core::EdgeArrowhead::Arrow);

        let layout = Layout {
            width: 400.0,
            height: 100.0,
            nodes,
            edges: vec![edge],
            subgraphs: vec![],
            title: None,
        };

        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        // 主线 + 箭头 = 2 条
        assert_eq!(edge_cmds.len(), 2, "主线 + 末端箭头");
        // 箭头 = 实心三角（Decorated → Path with fill）
        match &edge_cmds[1] {
            DrawCmd::Decorated { inner, .. } => {
                assert!(matches!(inner.as_ref(), DrawCmd::Path { fill: Some(_), .. }), "箭头为 fill path");
            }
            other => panic!("箭头应为 Decorated path, got {:?}", other),
        }
    }

    #[test]
    fn test_undirected_edge_has_no_arrowhead() {
        let mut edge = make_edge_layout("A", "B", vec![(0.0, 0.0), (50.0, 0.0)]);
        edge.directed = false;
        let layout = Layout {
            width: 100.0,
            height: 50.0,
            nodes: BTreeMap::new(),
            edges: vec![edge],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 1, "无箭头仅主线");
    }

    #[test]
    fn test_bidirectional_edge_two_arrowheads() {
        let mut edge = make_edge_layout("A", "B", vec![(0.0, 0.0), (50.0, 0.0)]);
        edge.arrow_start = Some(mermaid_canvas_core::EdgeArrowhead::Arrow);
        edge.arrow_end = Some(mermaid_canvas_core::EdgeArrowhead::Arrow);
        let layout = Layout {
            width: 100.0,
            height: 50.0,
            nodes: BTreeMap::new(),
            edges: vec![edge],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        assert_eq!(get_layer_cmds(&result, LayerKind::Edges).len(), 3, "主线 + 双端箭头");
    }

    // ─── T12: 线型消费 ─────────────────────────────────────

    #[test]
    fn test_dashed_edge_carries_dash_decor() {
        let mut edge = make_edge_layout("A", "B", vec![(0.0, 0.0), (50.0, 0.0)]);
        edge.style = mermaid_canvas_core::EdgeStyle::Dashed;
        let layout = Layout {
            width: 100.0,
            height: 50.0,
            nodes: BTreeMap::new(),
            edges: vec![edge],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        match &get_layer_cmds(&result, LayerKind::Edges)[0] {
            DrawCmd::Decorated { decor, .. } => {
                assert_eq!(decor.dash, Some(vec![6.0, 4.0]), "Dashed → dash [6,4]");
            }
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_dotted_edge_carries_dot_decor_and_round_cap() {
        let mut edge = make_edge_layout("A", "B", vec![(0.0, 0.0), (50.0, 0.0)]);
        edge.style = mermaid_canvas_core::EdgeStyle::Dotted;
        let layout = Layout {
            width: 100.0,
            height: 50.0,
            nodes: BTreeMap::new(),
            edges: vec![edge],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        match &get_layer_cmds(&result, LayerKind::Edges)[0] {
            DrawCmd::Decorated { decor, .. } => {
                assert_eq!(decor.dash, Some(vec![2.0, 3.0]));
                assert_eq!(decor.line_cap.as_deref(), Some("round"));
            }
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_thick_edge_arrowhead_wider() {
        let mut edge = make_edge_layout("A", "B", vec![(0.0, 0.0), (50.0, 0.0)]);
        edge.style = mermaid_canvas_core::EdgeStyle::Thick;
        edge.arrow_end = Some(mermaid_canvas_core::EdgeArrowhead::Arrow);
        let layout = Layout {
            width: 100.0,
            height: 50.0,
            nodes: BTreeMap::new(),
            edges: vec![edge],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let edge_cmds = get_layer_cmds(&result, LayerKind::Edges);
        assert_eq!(edge_cmds.len(), 2);
        match &edge_cmds[0] {
            DrawCmd::Decorated { decor, .. } => assert_eq!(decor.stroke_width, Some(2.5), "Thick 主线 2.5px"),
            other => panic!("expected Decorated, got {:?}", other),
        }
        // 实心三角 fill 型箭头无描边（thick 只影响描边型）
        assert!(matches!(&edge_cmds[1], DrawCmd::Decorated { .. }));
    }

    // ─── T13: 真实几何 ─────────────────────────────────────

    #[test]
    fn test_degenerate_shapes_render_as_paths_not_rounded_rects() {
        let shapes = [
            NodeShape::Hexagon,
            NodeShape::Cylinder,
            NodeShape::Parallelogram,
            NodeShape::Trapezoid,
            NodeShape::Asymmetric,
        ];
        for shape in shapes {
            let mut nodes = BTreeMap::new();
            nodes.insert("S".to_string(), make_node_layout("S", 10.0, 10.0, shape));
            let layout = Layout {
                width: 200.0,
                height: 100.0,
                nodes,
                edges: vec![],
                subgraphs: vec![],
                title: None,
            };
            let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
            let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
            assert_eq!(node_cmds.len(), 1, "{:?}: 主体(含软阴影;路径形无 bevel)", shape);
            match &node_cmds[0] {
                DrawCmd::Decorated { inner, .. } => {
                    assert!(
                        matches!(inner.as_ref(), DrawCmd::Path { segments, .. } if segments.len() >= 4),
                        "{:?} 应渲染为真实 path",
                        shape,
                    );
                }
                other => panic!("{:?}: expected Decorated, got {:?}", shape, other),
            }
        }
    }

    #[test]
    fn test_cylinder_path_has_six_segments() {
        let mut nodes = BTreeMap::new();
        nodes.insert("DB".to_string(), make_node_layout("DB", 10.0, 10.0, NodeShape::Cylinder));
        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        match &get_layer_cmds(&result, LayerKind::Nodes)[0] {
            DrawCmd::Decorated { inner, .. } => match inner.as_ref() {
                DrawCmd::Path { segments, .. } => {
                    // Move + 2 Bezier(上盖) + Line(侧线) + 2 Bezier(下盖) + Close = 7
                    assert_eq!(segments.len(), 7, "圆柱 = 上下椭圆弧 + 侧线");
                    assert_eq!(segments.iter().filter(|s| matches!(s, PathSegment::BezierTo(..))).count(), 4);
                }
                other => panic!("expected Path, got {:?}", other),
            },
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_double_circle_renders_two_circles() {
        let mut nodes = BTreeMap::new();
        nodes.insert("DC".to_string(), make_node_layout("DC", 10.0, 10.0, NodeShape::DoubleCircle));
        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        match &get_layer_cmds(&result, LayerKind::Nodes)[0] {
            DrawCmd::Decorated { inner, .. } => match inner.as_ref() {
                DrawCmd::Group { items, .. } => {
                    assert_eq!(items.len(), 2, "双同心圆");
                    let radii: Vec<f64> = items.iter().map(|i| match i {
                        DrawCmd::Circle { r, .. } => *r,
                        _ => panic!(),
                    }).collect();
                    assert!(radii[0] > radii[1], "外圆 > 内圆");
                }
                other => panic!("expected Group, got {:?}", other),
            },
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_stadium_uses_full_round_corner() {
        let mut nodes = BTreeMap::new();
        nodes.insert("ST".to_string(), make_node_layout("ST", 10.0, 10.0, NodeShape::Stadium));
        let layout = Layout {
            width: 200.0,
            height: 100.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        match &get_layer_cmds(&result, LayerKind::Nodes)[0] {
            DrawCmd::Decorated { inner, .. } => match inner.as_ref() {
                DrawCmd::Rect { corner_radius, height, .. } => {
                    assert_eq!(*corner_radius, Some(height / 2.0), "体育场 = 全圆角");
                }
                other => panic!("expected Rect, got {:?}", other),
            },
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    // ─── T14: 子图渲染 ─────────────────────────────────────

    #[test]
    fn test_subgraph_renders_box_and_label() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 60.0, 60.0, NodeShape::Rectangle));
        let subgraphs = vec![SubgraphLayout {
            id: "sg1".to_string(),
            label: TextBlock {
                text: "My Group".to_string(),
                x: 50.0,
                y: 40.0,
                width: 60.0,
                height: 14.0,
                font_size: 12.0,
            },
            x: 40.0,
            y: 20.0,
            width: 140.0,
            height: 120.0,
        }];
        let layout = Layout {
            width: 300.0,
            height: 200.0,
            nodes,
            edges: vec![],
            subgraphs,
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let sg_cmds = get_layer_cmds(&result, LayerKind::Subgraphs);
        assert_eq!(sg_cmds.len(), 2, "框 + 标题文字");
        assert!(matches!(&sg_cmds[0], DrawCmd::Rect { corner_radius: Some(_), fill: Some(_), .. }));
        assert!(matches!(&sg_cmds[1], DrawCmd::Text { content, .. } if content == "My Group"));
    }

    #[test]
    fn test_nested_subgraphs_outer_drawn_first() {
        let subgraphs = vec![
            SubgraphLayout {
                id: "outer".to_string(),
                label: make_text_block("Outer"),
                x: 0.0, y: 0.0, width: 300.0, height: 200.0,
            },
            SubgraphLayout {
                id: "inner".to_string(),
                label: make_text_block("Inner"),
                x: 40.0, y: 40.0, width: 100.0, height: 80.0,
            },
        ];
        let layout = Layout {
            width: 400.0,
            height: 300.0,
            nodes: BTreeMap::new(),
            edges: vec![],
            subgraphs,
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let sg_cmds = get_layer_cmds(&result, LayerKind::Subgraphs);
        // 外框（大面积）先画
        match &sg_cmds[0] {
            DrawCmd::Rect { width, .. } => assert!(*width == 300.0, "外框先画"),
            other => panic!("expected Rect, got {:?}", other),
        }
    }

    // ─── T15: 标题层 ───────────────────────────────────────

    #[test]
    fn test_title_renders_to_title_layer() {
        let layout = Layout {
            width: 300.0,
            height: 200.0,
            nodes: BTreeMap::new(),
            edges: vec![],
            subgraphs: vec![],
            title: Some(TextBlock {
                text: "我的图".to_string(),
                x: 150.0,
                y: 20.0,
                width: 60.0,
                height: 25.0,
                font_size: 18.0,
            }),
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let title_cmds = get_layer_cmds(&result, LayerKind::Title);
        assert_eq!(title_cmds.len(), 1);
        match &title_cmds[0] {
            DrawCmd::Text { content, style, anchor, .. } => {
                assert_eq!(content, "我的图");
                assert_eq!(style.font_size, 18.0, "title_font_size");
                assert!(matches!(anchor, TextAnchor::Middle));
            }
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_no_title_layer_commands_without_title() {
        let layout = Layout {
            width: 100.0,
            height: 100.0,
            nodes: BTreeMap::new(),
            edges: vec![],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        assert!(get_layer_cmds(&result, LayerKind::Title).is_empty());
    }

    // ─── 命令身份接线（id → hit-region index）───────────────

    #[test]
    fn test_node_commands_carry_sequential_hit_ids() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".to_string(), make_node_layout("A", 10.0, 20.0, NodeShape::Rectangle));
        nodes.insert("B".to_string(), make_node_layout("B", 200.0, 20.0, NodeShape::Diamond));
        nodes.insert("C".to_string(), make_node_layout("C", 400.0, 20.0, NodeShape::Circle));
        let layout = Layout {
            width: 600.0,
            height: 100.0,
            nodes,
            edges: vec![],
            subgraphs: vec![],
            title: None,
        };
        let result = FlowchartRenderer::render(&layout, &DefaultTheme).unwrap();
        let node_cmds = get_layer_cmds(&result, LayerKind::Nodes);
        // R10 卡片族:A(rect 2) + B(diamond 1) + C(circle 2) = 5
        assert_eq!(node_cmds.len(), 5);
        for cmd in node_cmds.iter() {
            match cmd {
                // 全族携带所属节点 hit id(0/1/2 — 卡片指令与主体同身份)
                DrawCmd::Decorated { decor, .. } => {
                    assert!(
                        matches!(decor.id, Some(0) | Some(1) | Some(2)),
                        "节点卡片族 id 接线: {:?}",
                        decor.id
                    );
                }
                other => panic!("expected Decorated, got {:?}", other),
            }
        }
        // 主体指令仍是各族首个非影子指令;ids 覆盖三个节点
        let ids: std::collections::BTreeSet<u32> = node_cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Decorated { decor, .. } => decor.id,
                _ => None,
            })
            .collect();
        assert_eq!(ids, [0, 1, 2].into_iter().collect(), "三节点 id 全覆盖");
    }
}
