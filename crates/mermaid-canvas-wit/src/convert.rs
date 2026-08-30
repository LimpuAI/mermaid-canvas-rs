//! 类型转换层 — WIT types ↔ internal types（v2：无损投影）

use super::wit_types::*;
use mermaid_canvas_component::{ThemeRecord, Layout};
use mermaid_canvas_core::{
    DiagramKind, Direction,
    DrawCmd, LayerKind, Layer,
    style::{FillStyle, StrokeStyle, TextStyle, FontWeight, FontStyle},
};

/// 转换错误
#[derive(Debug, Clone, PartialEq)]
pub enum ConvertError {
    /// 不支持的图表类型
    UnsupportedDiagramKind(String),
    /// 不支持的方向
    UnsupportedDirection(String),
    /// 不支持的形状
    UnsupportedShape(String),
    /// 缺少必需字段
    MissingRequiredField(String),
    /// 类型转换失败
    TypeMismatch(String),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::UnsupportedDiagramKind(s) => write!(f, "Unsupported diagram kind: {}", s),
            ConvertError::UnsupportedDirection(s) => write!(f, "Unsupported direction: {}", s),
            ConvertError::UnsupportedShape(s) => write!(f, "Unsupported shape: {}", s),
            ConvertError::MissingRequiredField(s) => write!(f, "Missing required field: {}", s),
            ConvertError::TypeMismatch(s) => write!(f, "Type mismatch: {}", s),
        }
    }
}

impl std::error::Error for ConvertError {}

/// 字符串转图表类型
pub fn str_to_diagram_kind(s: &str) -> Result<DiagramKind, ConvertError> {
    match s {
        "flowchart" => Ok(DiagramKind::Flowchart),
        "sequence" => Ok(DiagramKind::Sequence),
        "class" => Ok(DiagramKind::Class),
        "state" => Ok(DiagramKind::State),
        "er" => Ok(DiagramKind::Er),
        "pie" => Ok(DiagramKind::Pie),
        _ => Err(ConvertError::UnsupportedDiagramKind(s.to_string())),
    }
}

/// 图表类型转字符串
pub fn diagram_kind_to_str(kind: DiagramKind) -> String {
    match kind {
        DiagramKind::Flowchart => "flowchart",
        DiagramKind::Sequence => "sequence",
        DiagramKind::Class => "class",
        DiagramKind::State => "state",
        DiagramKind::Er => "er",
        DiagramKind::Pie => "pie",
        DiagramKind::Mindmap => "mindmap",
        DiagramKind::Journey => "journey",
        DiagramKind::Timeline => "timeline",
        DiagramKind::Gantt => "gantt",
        DiagramKind::Requirement => "requirement",
        DiagramKind::GitGraph => "gitgraph",
        DiagramKind::C4 => "c4",
        DiagramKind::Sankey => "sankey",
        DiagramKind::Quadrant => "quadrant",
        DiagramKind::Block => "block",
        DiagramKind::Packet => "packet",
        DiagramKind::Kanban => "kanban",
        DiagramKind::Architecture => "architecture",
        DiagramKind::Radar => "radar",
        DiagramKind::Treemap => "treemap",
        DiagramKind::XYChart => "xychart",
    }.to_string()
}

/// 字符串转方向
pub fn str_to_direction(s: &str) -> Result<Direction, ConvertError> {
    match s {
        "TD" | "TB" => Ok(Direction::TopDown),
        "BT" => Ok(Direction::BottomUp),
        "LR" => Ok(Direction::LeftToRight),
        "RL" => Ok(Direction::RightToLeft),
        _ => Err(ConvertError::UnsupportedDirection(s.to_string())),
    }
}

/// 方向转字符串
pub fn direction_to_str(dir: Direction) -> String {
    match dir {
        Direction::TopDown => "TD".to_string(),
        Direction::BottomUp => "BT".to_string(),
        Direction::LeftToRight => "LR".to_string(),
        Direction::RightToLeft => "RL".to_string(),
    }
}

/// FillStyle → WIT paint（无损：Color → Solid，线性 Gradient → Gradient；
/// 径向渐变超出共享词汇表 → None）
pub fn fill_style_to_paint(fill: FillStyle) -> Option<WitPaint> {
    match fill {
        FillStyle::Color(c) => Some(WitPaint::Solid(c)),
        FillStyle::Gradient(g) => match g.kind {
            mermaid_canvas_core::GradientKind::Linear { x0, y0, x1, y1 } =>
                Some(WitPaint::Gradient(WitLinearGradient {
                    x0, y0, x1, y1,
                    stops: g.stops.into_iter()
                        .map(|s| WitGradientStop { pos: s.offset, color: s.color })
                        .collect(),
                })),
            mermaid_canvas_core::GradientKind::Radial { .. } => None,
        },
        FillStyle::None => None,
    }
}

/// StrokeStyle → WIT paint
pub fn stroke_style_to_paint(stroke: StrokeStyle) -> Option<WitPaint> {
    match stroke {
        StrokeStyle::Color(c) => Some(WitPaint::Solid(c)),
        StrokeStyle::None => None,
    }
}

/// TextStyle → WIT font-desc（Bold ≙ 700 为 CSS 标准等价）
pub fn text_style_to_font_desc(style: &TextStyle) -> WitFontDesc {
    WitFontDesc {
        family: Some(style.font_family.clone()),
        weight: match style.font_weight {
            FontWeight::Normal => None,
            FontWeight::Bold => Some(700),
            FontWeight::Number(n) => Some(n),
        },
        italic: matches!(style.font_style, FontStyle::Italic),
        features: None,
    }
}

/// 装饰通道并入展平指令（Some 才覆盖 — 不清空内层已带的值）
fn apply_decor(cmd: &mut WitDrawCmd, decor: &mermaid_canvas_core::CmdDecor) {
    if decor.stroke_width.is_some() {
        cmd.stroke_width = decor.stroke_width;
    }
    if decor.dash.is_some() {
        cmd.dash = decor.dash.clone();
    }
    if decor.line_cap.is_some() {
        cmd.line_cap = decor.line_cap.clone();
    }
    if decor.id.is_some() {
        cmd.id = decor.id;
    }
    if decor.shadow.is_some() {
        cmd.shadow = decor.shadow.as_ref().map(|s| WitShadowDesc {
            offset_x: s.offset_x,
            offset_y: s.offset_y,
            blur: s.blur,
            spread: s.spread,
            color: s.color.clone(),
            alpha: s.alpha,
            width: s.width,
            height: s.height,
            rotation: s.rotation,
        });
    }
}

/// 内部 DrawCmd 转换为展平的 WIT DrawCmd 列表（v2 无损：corner-radius/font/paint
/// + Decorated 装饰通道（dash/line-cap/线宽/id）全量过 ABI）
pub fn draw_cmd_to_wit_draw_cmd_flat(cmd: DrawCmd, depth: u32) -> Vec<WitDrawCmd> {
    match cmd {
        DrawCmd::Rect { x, y, width, height, fill, stroke, corner_radius } => {
            vec![WitDrawCmd {
                cmd_type: "rect".to_string(),
                params: vec![x, y, width, height],
                fill: fill.and_then(fill_style_to_paint),
                stroke: stroke.and_then(stroke_style_to_paint),
                stroke_width: None,
                corner_radius,
                corner_radii: None,
                dash: None,
                line_cap: None,
                shadow: None,
                text_content: None,
                font: None,
                group_depth: depth,
                id: None,
                anims: Vec::new(),
            }]
        }
        DrawCmd::Circle { cx, cy, r, fill, stroke } => {
            vec![WitDrawCmd {
                cmd_type: "circle".to_string(),
                params: vec![cx, cy, r],
                fill: fill.and_then(fill_style_to_paint),
                stroke: stroke.and_then(stroke_style_to_paint),
                stroke_width: None,
                corner_radius: None,
                corner_radii: None,
                dash: None,
                line_cap: None,
                shadow: None,
                text_content: None,
                font: None,
                group_depth: depth,
                id: None,
                anims: Vec::new(),
            }]
        }
        DrawCmd::Text { x, y, content, style, anchor, baseline } => {
            let anchor_code = match anchor { mermaid_canvas_core::TextAnchor::Start => 0.0, mermaid_canvas_core::TextAnchor::Middle => 1.0, mermaid_canvas_core::TextAnchor::End => 2.0 };
            let baseline_code = match baseline { mermaid_canvas_core::TextBaseline::Top => 0.0, mermaid_canvas_core::TextBaseline::Middle => 1.0, mermaid_canvas_core::TextBaseline::Bottom => 2.0, mermaid_canvas_core::TextBaseline::Alphabetic => 3.0 };
            let font = text_style_to_font_desc(&style);
            vec![WitDrawCmd {
                cmd_type: "text".to_string(),
                params: vec![x, y, style.font_size, anchor_code, baseline_code],
                fill: fill_style_to_paint(style.fill),
                stroke: None,
                stroke_width: None,
                corner_radius: None,
                corner_radii: None,
                dash: None,
                line_cap: None,
                shadow: None,
                text_content: Some(content),
                font: Some(font),
                group_depth: depth,
                id: None,
                anims: Vec::new(),
            }]
        }
        DrawCmd::Path { segments, fill, stroke } => {
            let mut params = Vec::new();
            for seg in &segments {
                match seg {
                    mermaid_canvas_core::PathSegment::MoveTo(x, y) => params.extend_from_slice(&[0.0, *x, *y]),
                    mermaid_canvas_core::PathSegment::LineTo(x, y) => params.extend_from_slice(&[1.0, *x, *y]),
                    mermaid_canvas_core::PathSegment::BezierTo(cp1x, cp1y, cp2x, cp2y, x, y) =>
                        params.extend_from_slice(&[2.0, *cp1x, *cp1y, *cp2x, *cp2y, *x, *y]),
                    mermaid_canvas_core::PathSegment::QuadraticTo(cpx, cpy, x, y) =>
                        params.extend_from_slice(&[3.0, *cpx, *cpy, *x, *y]),
                    mermaid_canvas_core::PathSegment::Arc(cx, cy, r, start, end, ccw) =>
                        params.extend_from_slice(&[4.0, *cx, *cy, *r, *start, *end, if *ccw { 1.0 } else { 0.0 }]),
                    mermaid_canvas_core::PathSegment::Close => params.push(5.0),
                }
            }
            vec![WitDrawCmd {
                cmd_type: "path".to_string(),
                params,
                fill: fill.and_then(fill_style_to_paint),
                stroke: stroke.and_then(stroke_style_to_paint),
                stroke_width: None,
                corner_radius: None,
                corner_radii: None,
                dash: None,
                line_cap: None,
                shadow: None,
                text_content: None,
                font: None,
                group_depth: depth,
                id: None,
                anims: Vec::new(),
            }]
        }
        DrawCmd::Group { label: _, items } => {
            items.into_iter().flat_map(|c| draw_cmd_to_wit_draw_cmd_flat(c, depth + 1)).collect()
        }
        DrawCmd::Decorated { inner, decor } => {
            // 装饰并入内层展平结果（Group 时应用到全部子指令）
            draw_cmd_to_wit_draw_cmd_flat(*inner, depth)
                .into_iter()
                .map(|mut w| {
                    apply_decor(&mut w, &decor);
                    w
                })
                .collect()
        }
    }
}

/// LayerKind 转字符串
pub fn layer_kind_to_str(kind: LayerKind) -> String {
    match kind {
        LayerKind::Background => "background",
        LayerKind::Subgraphs => "subgraphs",
        LayerKind::Edges => "edges",
        LayerKind::Nodes => "nodes",
        LayerKind::Labels => "labels",
        LayerKind::Title => "title",
        LayerKind::Annotations => "annotations",
    }.to_string()
}

/// 内部 Layer 转 WIT Layer
pub fn layer_to_wit_layer(layer: Layer) -> WitLayer {
    let commands: Vec<WitDrawCmd> = layer.commands.semantic.into_iter()
        .flat_map(|c| draw_cmd_to_wit_draw_cmd_flat(c, 0))
        .collect();
    WitLayer {
        kind: layer_kind_to_str(layer.kind),
        dirty: layer.dirty,
        z_index: layer.z_index,
        commands,
    }
}

/// WIT diagram-theme record → 内部 ThemeRecord
pub fn wit_theme_to_record(theme: WitDiagramTheme) -> ThemeRecord {
    ThemeRecord {
        background: theme.background,
        foreground: theme.foreground,
        edge_color: theme.edge_color,
        edge_label_background: theme.edge_label_background,
        node_colors: theme.node_colors,
        node_stroke: theme.node_stroke,
        title_color: theme.title_color,
        hover_color: theme.hover_color,
        style_preset: theme.style_preset,
        font_family: theme.font_family,
        base_font_size: theme.base_font_size,
        title_font_size: theme.title_font_size,
        margin: theme.margin.into(),
    }
}

/// 内部 ThemeRecord → WIT diagram-theme record
pub fn record_to_wit_theme(record: ThemeRecord) -> WitDiagramTheme {
    WitDiagramTheme {
        background: record.background,
        foreground: record.foreground,
        edge_color: record.edge_color,
        edge_label_background: record.edge_label_background,
        node_colors: record.node_colors,
        node_stroke: record.node_stroke,
        title_color: record.title_color,
        hover_color: record.hover_color,
        style_preset: record.style_preset,
        font_family: record.font_family,
        base_font_size: record.base_font_size,
        title_font_size: record.title_font_size,
        margin: record.margin.into(),
    }
}

/// Layout 节点 → 命中区列表（按 BTreeMap key 过滤序列图激活框等内部节点；
/// key 序 = 稳定索引；hover 声明由 session 按 preset 档位补充）
pub fn layout_to_hit_regions(layout: &Layout) -> Vec<WitHitRegion> {
    layout.nodes
        .iter()
        .filter(|(key, _)| !key.starts_with("__act_"))
        .enumerate()
        .map(|(i, (_, nl))| WitHitRegion {
            index: i as u32,
            node_id: Some(nl.id.clone()),
            bounds_x: nl.bounds.x,
            bounds_y: nl.bounds.y,
            bounds_w: nl.bounds.width,
            bounds_h: nl.bounds.height,
            hover: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mermaid_canvas_core::{
        DrawCmd, PathSegment, LayerKind, Layer,
        FillStyle, StrokeStyle, TextStyle, TextAnchor, TextBaseline,
        DiagramKind, Direction,
        RenderOutput,
    };
    use mermaid_canvas_component::{builtin_theme_record, Margin};

    // ─── DrawCmd::Rect ──────────────────────────────────────

    #[test]
    fn test_rect_with_fill_and_stroke() {
        let cmd = DrawCmd::Rect {
            x: 10.0, y: 20.0, width: 100.0, height: 50.0,
            fill: Some(FillStyle::Color("#ff0000".into())),
            stroke: Some(StrokeStyle::Color("#000000".into())),
            corner_radius: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        let w = &result[0];
        assert_eq!(w.cmd_type, "rect");
        assert_eq!(w.params, vec![10.0, 20.0, 100.0, 50.0]);
        assert_eq!(w.fill, Some(WitPaint::Solid("#ff0000".to_string())));
        assert_eq!(w.stroke, Some(WitPaint::Solid("#000000".to_string())));
        assert!(w.text_content.is_none());
        assert!(w.anims.is_empty());
        assert_eq!(w.group_depth, 0);
    }

    #[test]
    fn test_rect_corner_radius_lossless() {
        let cmd = DrawCmd::Rect {
            x: 0.0, y: 0.0, width: 80.0, height: 40.0,
            fill: Some(FillStyle::Color("#fff".into())),
            stroke: None,
            corner_radius: Some(8.0),
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        // v2 无损：corner_radius 过 ABI（v1 丢弃）
        assert_eq!(result[0].corner_radius, Some(8.0));
    }

    #[test]
    fn test_rect_no_fill_no_stroke() {
        let cmd = DrawCmd::Rect {
            x: 0.0, y: 0.0, width: 50.0, height: 50.0,
            fill: None, stroke: None, corner_radius: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 2);
        assert_eq!(result.len(), 1);
        let w = &result[0];
        assert!(w.fill.is_none());
        assert!(w.stroke.is_none());
        assert_eq!(w.group_depth, 2);
    }

    #[test]
    fn test_rect_zero_dimensions() {
        let cmd = DrawCmd::Rect {
            x: 5.0, y: 5.0, width: 0.0, height: 0.0,
            fill: Some(FillStyle::Color("#abc".into())),
            stroke: None, corner_radius: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].params, vec![5.0, 5.0, 0.0, 0.0]);
    }

    #[test]
    fn test_rect_negative_coords() {
        let cmd = DrawCmd::Rect {
            x: -10.0, y: -20.0, width: 30.0, height: 40.0,
            fill: None, stroke: None, corner_radius: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result[0].params, vec![-10.0, -20.0, 30.0, 40.0]);
    }

    // ─── DrawCmd::Circle ────────────────────────────────────

    #[test]
    fn test_circle_basic() {
        let cmd = DrawCmd::Circle {
            cx: 50.0, cy: 60.0, r: 25.0,
            fill: Some(FillStyle::Color("#00ff00".into())),
            stroke: Some(StrokeStyle::Color("#111".into())),
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        let w = &result[0];
        assert_eq!(w.cmd_type, "circle");
        assert_eq!(w.params, vec![50.0, 60.0, 25.0]);
        assert_eq!(w.fill, Some(WitPaint::Solid("#00ff00".to_string())));
        assert_eq!(w.stroke, Some(WitPaint::Solid("#111".to_string())));
    }

    #[test]
    fn test_circle_no_fill() {
        let cmd = DrawCmd::Circle {
            cx: 0.0, cy: 0.0, r: 10.0,
            fill: None, stroke: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 3);
        assert_eq!(result.len(), 1);
        assert!(result[0].fill.is_none());
        assert!(result[0].stroke.is_none());
        assert_eq!(result[0].group_depth, 3);
    }

    // ─── DrawCmd::Text ──────────────────────────────────────

    #[test]
    fn test_text_basic() {
        let style = TextStyle::new().with_font_size(16.0);
        let cmd = DrawCmd::Text {
            x: 100.0, y: 200.0,
            content: "Hello".into(),
            style,
            anchor: TextAnchor::Start,
            baseline: TextBaseline::Top,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        let w = &result[0];
        assert_eq!(w.cmd_type, "text");
        // params: [x, y, font_size, anchor_code, baseline_code]
        assert_eq!(w.params[0], 100.0);
        assert_eq!(w.params[1], 200.0);
        assert_eq!(w.params[2], 16.0);
        assert_eq!(w.params[3], 0.0); // Start
        assert_eq!(w.params[4], 0.0); // Top
        assert_eq!(w.text_content.as_deref(), Some("Hello"));
        assert!(w.fill.is_some());
        assert!(w.stroke.is_none());
    }

    #[test]
    fn test_text_anchor_and_baseline_codes() {
        let style = TextStyle::new();
        // Middle anchor, Middle baseline
        let cmd = DrawCmd::Text {
            x: 0.0, y: 0.0, content: "test".into(),
            style, anchor: TextAnchor::Middle, baseline: TextBaseline::Middle,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result[0].params[3], 1.0); // Middle anchor
        assert_eq!(result[0].params[4], 1.0); // Middle baseline

        let style = TextStyle::new();
        // End anchor, Bottom baseline
        let cmd = DrawCmd::Text {
            x: 0.0, y: 0.0, content: "test".into(),
            style, anchor: TextAnchor::End, baseline: TextBaseline::Bottom,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result[0].params[3], 2.0); // End anchor
        assert_eq!(result[0].params[4], 2.0); // Bottom baseline

        let style = TextStyle::new();
        // Alphabetic baseline
        let cmd = DrawCmd::Text {
            x: 0.0, y: 0.0, content: "test".into(),
            style, anchor: TextAnchor::Start, baseline: TextBaseline::Alphabetic,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result[0].params[4], 3.0); // Alphabetic baseline
    }

    #[test]
    fn test_text_font_desc_lossless() {
        let style = TextStyle::new()
            .with_font_family("Serif")
            .with_font_size(14.0);
        let cmd = DrawCmd::Text {
            x: 0.0, y: 0.0, content: "t".into(),
            style, anchor: TextAnchor::Start, baseline: TextBaseline::Top,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        let font = result[0].font.as_ref().expect("text cmd must carry font-desc");
        assert_eq!(font.family.as_deref(), Some("Serif"));
        assert_eq!(font.weight, None);
        assert!(!font.italic);

        // Bold ≙ 700
        let mut style = TextStyle::new();
        style.font_weight = FontWeight::Bold;
        let cmd = DrawCmd::Text {
            x: 0.0, y: 0.0, content: "t".into(),
            style, anchor: TextAnchor::Start, baseline: TextBaseline::Top,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result[0].font.as_ref().unwrap().weight, Some(700));

        // Italic
        let mut style = TextStyle::new();
        style.font_style = FontStyle::Italic;
        let cmd = DrawCmd::Text {
            x: 0.0, y: 0.0, content: "t".into(),
            style, anchor: TextAnchor::Start, baseline: TextBaseline::Top,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert!(result[0].font.as_ref().unwrap().italic);
    }

    // ─── DrawCmd::Path ──────────────────────────────────────

    #[test]
    fn test_path_empty() {
        let cmd = DrawCmd::Path {
            segments: vec![],
            fill: None, stroke: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].cmd_type, "path");
        assert!(result[0].params.is_empty());
    }

    #[test]
    fn test_path_move_line_close() {
        let cmd = DrawCmd::Path {
            segments: vec![
                PathSegment::MoveTo(0.0, 0.0),
                PathSegment::LineTo(100.0, 0.0),
                PathSegment::LineTo(100.0, 100.0),
                PathSegment::Close,
            ],
            fill: Some(FillStyle::Color("#fff".into())),
            stroke: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        let w = &result[0];
        assert_eq!(w.cmd_type, "path");
        // MoveTo(0,0) → [0, 0, 0], LineTo(100,0) → [1, 100, 0], LineTo(100,100) → [1, 100, 100], Close → [5]
        assert_eq!(w.params, vec![0.0, 0.0, 0.0, 1.0, 100.0, 0.0, 1.0, 100.0, 100.0, 5.0]);
        assert_eq!(w.fill, Some(WitPaint::Solid("#fff".to_string())));
    }

    #[test]
    fn test_path_bezier() {
        let cmd = DrawCmd::Path {
            segments: vec![
                PathSegment::MoveTo(0.0, 0.0),
                PathSegment::BezierTo(10.0, 10.0, 20.0, 20.0, 30.0, 30.0),
            ],
            fill: None, stroke: Some(StrokeStyle::Color("#333".into())),
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        // MoveTo → [0, 0, 0], BezierTo → [2, 10, 10, 20, 20, 30, 30]
        assert_eq!(result[0].params, vec![0.0, 0.0, 0.0, 2.0, 10.0, 10.0, 20.0, 20.0, 30.0, 30.0]);
        assert_eq!(result[0].stroke, Some(WitPaint::Solid("#333".to_string())));
    }

    #[test]
    fn test_path_quadratic_and_arc() {
        let cmd = DrawCmd::Path {
            segments: vec![
                PathSegment::QuadraticTo(5.0, 5.0, 10.0, 10.0),
                PathSegment::Arc(50.0, 50.0, 10.0, 0.0, 3.14, true),
            ],
            fill: None, stroke: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        // QuadraticTo → [3, 5, 5, 10, 10], Arc → [4, 50, 50, 10, 0, 3.14, 1]
        assert_eq!(result[0].params, vec![3.0, 5.0, 5.0, 10.0, 10.0, 4.0, 50.0, 50.0, 10.0, 0.0, 3.14, 1.0]);
    }

    #[test]
    fn test_path_arc_ccw_false() {
        let cmd = DrawCmd::Path {
            segments: vec![
                PathSegment::Arc(0.0, 0.0, 5.0, 0.0, 1.0, false),
            ],
            fill: None, stroke: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        // Arc → [4, 0, 0, 5, 0, 1, 0]  (ccw=false → 0.0)
        assert_eq!(result[0].params.last(), Some(&0.0));
    }

    // ─── DrawCmd::Group ─────────────────────────────────────

    #[test]
    fn test_group_flattens_with_incremented_depth() {
        let cmd = DrawCmd::Group {
            label: Some("g1".into()),
            items: vec![
                DrawCmd::Rect {
                    x: 0.0, y: 0.0, width: 10.0, height: 10.0,
                    fill: None, stroke: None, corner_radius: None,
                },
                DrawCmd::Circle {
                    cx: 5.0, cy: 5.0, r: 3.0,
                    fill: None, stroke: None,
                },
            ],
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].cmd_type, "rect");
        assert_eq!(result[0].group_depth, 1); // depth+1
        assert_eq!(result[1].cmd_type, "circle");
        assert_eq!(result[1].group_depth, 1);
    }

    #[test]
    fn test_group_nested() {
        let inner = DrawCmd::Group {
            label: None,
            items: vec![
                DrawCmd::Rect {
                    x: 1.0, y: 1.0, width: 2.0, height: 2.0,
                    fill: None, stroke: None, corner_radius: None,
                },
            ],
        };
        let outer = DrawCmd::Group {
            label: None,
            items: vec![inner],
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(outer, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].group_depth, 2); // outer depth=0 → inner depth=1 → rect depth=2
    }

    #[test]
    fn test_group_empty() {
        let cmd = DrawCmd::Group {
            label: None,
            items: vec![],
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 5);
        assert!(result.is_empty());
    }

    // ─── Gradient paint（v2 无损）────────────────────────────

    #[test]
    fn test_rect_gradient_fill_lossless() {
        let gradient = FillStyle::Gradient(mermaid_canvas_core::Gradient {
            kind: mermaid_canvas_core::GradientKind::Linear {
                x0: 0.0, y0: 0.0, x1: 100.0, y1: 0.0,
            },
            stops: vec![
                mermaid_canvas_core::GradientStop::new(0.0, "#ffffff"),
                mermaid_canvas_core::GradientStop::new(1.0, "#000000"),
            ],
        });
        let cmd = DrawCmd::Rect {
            x: 0.0, y: 0.0, width: 10.0, height: 10.0,
            fill: Some(gradient), stroke: None, corner_radius: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        // v2 无损：线性渐变完整过 ABI（v1 丢弃为 None）
        match &result[0].fill {
            Some(WitPaint::Gradient(g)) => {
                assert_eq!((g.x0, g.y0, g.x1, g.y1), (0.0, 0.0, 100.0, 0.0));
                assert_eq!(g.stops.len(), 2);
                assert_eq!(g.stops[0].pos, 0.0);
                assert_eq!(g.stops[0].color, "#ffffff");
                assert_eq!(g.stops[1].pos, 1.0);
                assert_eq!(g.stops[1].color, "#000000");
            }
            other => panic!("expected gradient paint, got {:?}", other),
        }
    }

    #[test]
    fn test_circle_none_stroke_becomes_none() {
        let stroke = StrokeStyle::None;
        let cmd = DrawCmd::Circle {
            cx: 0.0, cy: 0.0, r: 5.0,
            fill: Some(FillStyle::None), stroke: Some(stroke),
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert!(result[0].fill.is_none());
        assert!(result[0].stroke.is_none());
    }

    // ─── LayerKind ↔ str ───────────────────────────────────

    #[test]
    fn test_layer_kind_to_str_all_variants() {
        assert_eq!(layer_kind_to_str(LayerKind::Background), "background");
        assert_eq!(layer_kind_to_str(LayerKind::Subgraphs), "subgraphs");
        assert_eq!(layer_kind_to_str(LayerKind::Edges), "edges");
        assert_eq!(layer_kind_to_str(LayerKind::Nodes), "nodes");
        assert_eq!(layer_kind_to_str(LayerKind::Labels), "labels");
        assert_eq!(layer_kind_to_str(LayerKind::Title), "title");
        assert_eq!(layer_kind_to_str(LayerKind::Annotations), "annotations");
    }

    // ─── DiagramKind ↔ str ─────────────────────────────────

    #[test]
    fn test_str_to_diagram_kind_valid() {
        assert_eq!(str_to_diagram_kind("flowchart"), Ok(DiagramKind::Flowchart));
        assert_eq!(str_to_diagram_kind("sequence"), Ok(DiagramKind::Sequence));
        assert_eq!(str_to_diagram_kind("class"), Ok(DiagramKind::Class));
        assert_eq!(str_to_diagram_kind("state"), Ok(DiagramKind::State));
        assert_eq!(str_to_diagram_kind("er"), Ok(DiagramKind::Er));
        assert_eq!(str_to_diagram_kind("pie"), Ok(DiagramKind::Pie));
    }

    #[test]
    fn test_str_to_diagram_kind_invalid() {
        assert!(str_to_diagram_kind("unknown").is_err());
        assert!(str_to_diagram_kind("").is_err());
    }

    #[test]
    fn test_diagram_kind_to_str_roundtrip() {
        let kinds = vec![
            DiagramKind::Flowchart, DiagramKind::Sequence, DiagramKind::Class,
            DiagramKind::State, DiagramKind::Er, DiagramKind::Pie,
        ];
        for kind in kinds {
            let s = diagram_kind_to_str(kind);
            let parsed = str_to_diagram_kind(&s);
            assert_eq!(parsed, Ok(kind), "roundtrip failed for {:?}", kind);
        }
    }

    // ─── Direction ↔ str ───────────────────────────────────

    #[test]
    fn test_str_to_direction_valid() {
        assert_eq!(str_to_direction("TD"), Ok(Direction::TopDown));
        assert_eq!(str_to_direction("TB"), Ok(Direction::TopDown));
        assert_eq!(str_to_direction("BT"), Ok(Direction::BottomUp));
        assert_eq!(str_to_direction("LR"), Ok(Direction::LeftToRight));
        assert_eq!(str_to_direction("RL"), Ok(Direction::RightToLeft));
    }

    #[test]
    fn test_str_to_direction_invalid() {
        assert!(str_to_direction("XX").is_err());
        assert!(str_to_direction("").is_err());
    }

    #[test]
    fn test_direction_to_str_roundtrip() {
        let dirs = vec![
            Direction::TopDown, Direction::BottomUp,
            Direction::LeftToRight, Direction::RightToLeft,
        ];
        for dir in dirs {
            let s = direction_to_str(dir);
            let parsed = str_to_direction(&s);
            assert_eq!(parsed, Ok(dir), "roundtrip failed for {:?}", dir);
        }
    }

    // ─── Layer → WitLayer ───────────────────────────────────

    #[test]
    fn test_layer_to_wit_layer() {
        let mut layer = Layer::new(LayerKind::Nodes);
        layer.mark_clean();
        layer.update_commands(RenderOutput::from_commands(vec![
            DrawCmd::Rect {
                x: 0.0, y: 0.0, width: 100.0, height: 50.0,
                fill: Some(FillStyle::Color("#fff".into())),
                stroke: None, corner_radius: None,
            },
        ]));
        let wit = layer_to_wit_layer(layer);
        assert_eq!(wit.kind, "nodes");
        assert!(wit.dirty);
        assert_eq!(wit.z_index, 3);
        assert_eq!(wit.commands.len(), 1);
        assert_eq!(wit.commands[0].cmd_type, "rect");
    }

    #[test]
    fn test_layer_to_wit_layer_with_group() {
        let mut layer = Layer::new(LayerKind::Edges);
        layer.update_commands(RenderOutput::from_commands(vec![
            DrawCmd::Group {
                label: None,
                items: vec![
                    DrawCmd::Path {
                        segments: vec![PathSegment::MoveTo(0.0, 0.0), PathSegment::LineTo(10.0, 10.0)],
                        fill: None, stroke: Some(StrokeStyle::Color("#000".into())),
                    },
                    DrawCmd::Text {
                        x: 5.0, y: 5.0, content: "edge".into(),
                        style: TextStyle::new(), anchor: TextAnchor::Middle, baseline: TextBaseline::Middle,
                    },
                ],
            },
        ]));
        let wit = layer_to_wit_layer(layer);
        assert_eq!(wit.kind, "edges");
        assert_eq!(wit.commands.len(), 2);
        assert_eq!(wit.commands[0].cmd_type, "path");
        assert_eq!(wit.commands[0].group_depth, 1);
        assert_eq!(wit.commands[1].cmd_type, "text");
        assert_eq!(wit.commands[1].group_depth, 1);
    }

    // ─── diagram-theme record ↔ ThemeRecord ─────────────────

    #[test]
    fn test_wit_theme_record_roundtrip() {
        let wit = WitDiagramTheme {
            background: "#101010".into(),
            foreground: "#eeeeee".into(),
            edge_color: "#555555".into(),
            edge_label_background: "#101010".into(),
            node_colors: vec!["#1".into(); 6],
            node_stroke: "#999999".into(),
            title_color: "#ffffff".into(),
            hover_color: Some("#ffffff".into()),
            style_preset: Some("signal-flow".into()),
            font_family: "Mono".into(),
            base_font_size: 13.0,
            title_font_size: 17.0,
            margin: WitMargin { top: 1.0, right: 2.0, bottom: 3.0, left: 4.0 },
        };
        let record = wit_theme_to_record(wit.clone());
        let back = record_to_wit_theme(record);
        assert_eq!(back, wit);
    }

    #[test]
    fn test_builtin_record_theme_v2_fields_default_none() {
        // 内置主题不声明 preset/hover 色 — v2 新字段缺省 None（= classic）
        let wit = record_to_wit_theme(builtin_theme_record("default").unwrap());
        assert_eq!(wit.hover_color, None);
        assert_eq!(wit.style_preset, None);
    }

    // ─── Decorated 装饰通道投影（v2）──────────────────────────

    #[test]
    fn test_decorated_path_carries_dash_linecap_id() {
        let cmd = DrawCmd::Decorated {
            inner: Box::new(DrawCmd::Path {
                segments: vec![PathSegment::MoveTo(0.0, 0.0), PathSegment::LineTo(10.0, 0.0)],
                fill: None,
                stroke: Some(StrokeStyle::Color("#333".into())),
            }),
            decor: mermaid_canvas_core::CmdDecor {
                stroke_width: Some(2.5),
                dash: Some(vec![6.0, 4.0]),
                line_cap: Some("round".to_string()),
                shadow: None,
                id: Some(3),
            },
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        let w = &result[0];
        assert_eq!(w.cmd_type, "path");
        assert_eq!(w.stroke_width, Some(2.5));
        assert_eq!(w.dash, Some(vec![6.0, 4.0]));
        assert_eq!(w.line_cap.as_deref(), Some("round"));
        assert_eq!(w.id, Some(3));
        assert!(w.anims.is_empty());
    }

    #[test]
    fn test_decorated_partial_decor_does_not_clear_rest() {
        // 装饰通道部分字段 — 未声明的不覆盖
        let cmd = DrawCmd::Decorated {
            inner: Box::new(DrawCmd::Rect {
                x: 0.0, y: 0.0, width: 10.0, height: 10.0,
                fill: None, stroke: None, corner_radius: Some(4.0),
            }),
            decor: mermaid_canvas_core::CmdDecor {
                id: Some(1),
                ..Default::default()
            },
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result[0].id, Some(1));
        assert_eq!(result[0].stroke_width, None);
        assert_eq!(result[0].corner_radius, Some(4.0));
    }

    #[test]
    fn test_builtin_record_to_wit_theme_has_seven_slots() {
        let wit = record_to_wit_theme(builtin_theme_record("forest").unwrap());
        assert_eq!(wit.node_colors.len(), 7);
        assert_eq!(wit.background, "#1b2a1b");
        assert_eq!(wit.margin, WitMargin::from(Margin::all(20.0)));
    }
}
