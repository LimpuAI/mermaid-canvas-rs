//! 类型转换层 — WIT types ↔ internal types

use super::wit_types::*;
use mermaid_canvas_core::{
    DiagramAst, DiagramNode, DiagramEdge, DiagramKind, Direction, NodeShape,
    DrawCmd, LayerKind, Layer,
    style::FillStyle,
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

/// 内部 DrawCmd 转换为展平的 WIT DrawCmd 列表
pub fn draw_cmd_to_wit_draw_cmd_flat(cmd: DrawCmd, depth: u32) -> Vec<WitDrawCmd> {
    match cmd {
        DrawCmd::Rect { x, y, width, height, fill, stroke, corner_radius: _ } => {
            vec![WitDrawCmd {
                cmd_type: "rect".to_string(),
                params: vec![x, y, width, height],
                fill: fill.and_then(|f| match f { FillStyle::Color(c) => Some(c), _ => None }),
                stroke: stroke.and_then(|s| match s { mermaid_canvas_core::StrokeStyle::Color(c) => Some(c), _ => None }),
                stroke_width: None,
                text_content: None,
                group_depth: depth,
            }]
        }
        DrawCmd::Circle { cx, cy, r, fill, stroke } => {
            vec![WitDrawCmd {
                cmd_type: "circle".to_string(),
                params: vec![cx, cy, r],
                fill: fill.and_then(|f| match f { FillStyle::Color(c) => Some(c), _ => None }),
                stroke: stroke.and_then(|s| match s { mermaid_canvas_core::StrokeStyle::Color(c) => Some(c), _ => None }),
                stroke_width: None,
                text_content: None,
                group_depth: depth,
            }]
        }
        DrawCmd::Text { x, y, content, style, anchor, baseline } => {
            let anchor_code = match anchor { mermaid_canvas_core::TextAnchor::Start => 0.0, mermaid_canvas_core::TextAnchor::Middle => 1.0, mermaid_canvas_core::TextAnchor::End => 2.0 };
            let baseline_code = match baseline { mermaid_canvas_core::TextBaseline::Top => 0.0, mermaid_canvas_core::TextBaseline::Middle => 1.0, mermaid_canvas_core::TextBaseline::Bottom => 2.0, mermaid_canvas_core::TextBaseline::Alphabetic => 3.0 };
            vec![WitDrawCmd {
                cmd_type: "text".to_string(),
                params: vec![x, y, style.font_size, anchor_code, baseline_code],
                fill: Some(match style.fill { FillStyle::Color(c) => c, _ => "#000".to_string() }),
                stroke: None,
                stroke_width: None,
                text_content: Some(content),
                group_depth: depth,
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
                fill: fill.and_then(|f| match f { FillStyle::Color(c) => Some(c), _ => None }),
                stroke: stroke.and_then(|s| match s { mermaid_canvas_core::StrokeStyle::Color(c) => Some(c), _ => None }),
                stroke_width: None,
                text_content: None,
                group_depth: depth,
            }]
        }
        DrawCmd::Group { label: _, items } => {
            items.into_iter().flat_map(|c| draw_cmd_to_wit_draw_cmd_flat(c, depth + 1)).collect()
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
        hit_regions: Vec::new(),
    }
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
        assert_eq!(w.fill.as_deref(), Some("#ff0000"));
        assert_eq!(w.stroke.as_deref(), Some("#000000"));
        assert!(w.text_content.is_none());
        assert_eq!(w.group_depth, 0);
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
        assert_eq!(w.cmd_type, "rect");
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
        assert_eq!(w.fill.as_deref(), Some("#00ff00"));
        assert_eq!(w.stroke.as_deref(), Some("#111"));
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
        assert_eq!(w.fill.as_deref(), Some("#fff"));
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
        assert_eq!(result[0].stroke.as_deref(), Some("#333"));
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
        assert!(wit.hit_regions.is_empty());
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

    // ─── Gradient fill ignored in conversion ───────────────

    #[test]
    fn test_rect_gradient_fill_becomes_none() {
        let gradient = FillStyle::Gradient(mermaid_canvas_core::Gradient {
            kind: mermaid_canvas_core::GradientKind::Linear {
                x0: 0.0, y0: 0.0, x1: 100.0, y1: 0.0,
            },
            stops: vec![mermaid_canvas_core::GradientStop::new(0.0, "#fff")],
        });
        let cmd = DrawCmd::Rect {
            x: 0.0, y: 0.0, width: 10.0, height: 10.0,
            fill: Some(gradient), stroke: None, corner_radius: None,
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert_eq!(result.len(), 1);
        // Gradient fill is not a Color variant → becomes None in WitDrawCmd
        assert!(result[0].fill.is_none());
    }

    #[test]
    fn test_circle_gradient_stroke_becomes_none() {
        let stroke = StrokeStyle::None;
        let cmd = DrawCmd::Circle {
            cx: 0.0, cy: 0.0, r: 5.0,
            fill: Some(FillStyle::None), stroke: Some(stroke),
        };
        let result = draw_cmd_to_wit_draw_cmd_flat(cmd, 0);
        assert!(result[0].fill.is_none());
        assert!(result[0].stroke.is_none());
    }
}
