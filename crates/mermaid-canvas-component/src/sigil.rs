//! 节点语义 sigil — 小矢量图标 path 族（T18）
//!
//! 按 NodeShape 生成 ~10px 矢量标记（数据库符号 / 六边形标记 / 判定角标…），
//! 绘制在节点左上角，颜色 = 节点描边色（语义槽位对比色）。
//! 由 preset 档位门控（SignalFlow / Blueprint 绘制；Classic / Editorial 不绘制）。
//!
//! 纯展示形状（Circle/DoubleCircle 自身即图标）与普通流程形状
//! （Rectangle/RoundRect/Stadium）无 sigil — 返回 None。

use mermaid_canvas_core::{
    instruction::{CmdDecor, DrawCmd, PathSegment},
    style::{FillStyle, StrokeStyle},
    NodeShape,
};

/// sigil 基准尺寸（px）
pub const SIGIL_SIZE: f64 = 10.0;
/// sigil 相对节点左上角的内缩（px）
pub const SIGIL_INSET: f64 = 6.0;

/// 按 NodeShape 生成 sigil 指令（无 sigil 的形状返回 None）
///
/// `origin` = sigil 绘制原点（节点左上角内缩后）；`color` = 描边色；
/// `hit_id` = 命中区索引（与节点形状指令同 id — 命令身份接线）。
pub fn sigil_cmd(
    shape: NodeShape,
    origin: (f64, f64),
    color: &str,
    hit_id: Option<u32>,
) -> Option<DrawCmd> {
    let stroke = StrokeStyle::Color(color.to_string());
    let path = |points: &[(f64, f64)]| -> DrawCmd {
        let mut segments = Vec::with_capacity(points.len() + 1);
        if let Some(&(x, y)) = points.first() {
            segments.push(PathSegment::MoveTo(x, y));
            for &(x, y) in &points[1..] {
                segments.push(PathSegment::LineTo(x, y));
            }
            segments.push(PathSegment::Close);
        }
        DrawCmd::Decorated {
            inner: Box::new(DrawCmd::Path {
                segments,
                fill: None,
                stroke: Some(stroke.clone()),
            }),
            decor: CmdDecor {
                line_cap: Some("round".to_string()),
                id: hit_id,
                ..Default::default()
            },
        }
    };

    let (x, y) = origin;
    let s = SIGIL_SIZE;
    match shape {
        // 数据库符号：椭圆柱（上椭圆 + 两侧线 + 下弧）
        NodeShape::Cylinder => {
            let ry = s * 0.22;
            let k = 0.552_284_749_830_898 * ry;
            let rx = s / 2.0;
            let cx = x + rx;
            Some(DrawCmd::Decorated {
                inner: Box::new(DrawCmd::Path {
                    segments: vec![
                        PathSegment::MoveTo(x, y + ry),
                        PathSegment::BezierTo(x, y + ry - k, cx - rx * 0.5523, y, cx, y),
                        PathSegment::BezierTo(cx + rx * 0.5523, y, x + s, y + ry - k, x + s, y + ry),
                        PathSegment::LineTo(x + s, y + s - ry),
                        PathSegment::BezierTo(x + s, y + s - ry + k, cx + rx * 0.5523, y + s, cx, y + s),
                        PathSegment::BezierTo(cx - rx * 0.5523, y + s, x, y + s - ry + k, x, y + s - ry),
                    ],
                    fill: None,
                    stroke: Some(stroke),
                }),
                decor: CmdDecor {
                    line_cap: Some("round".to_string()),
                    id: hit_id,
                    ..Default::default()
                },
            })
        }
        // 六边形标记
        NodeShape::Hexagon => {
            let inset = s * 0.3;
            let cy = y + s / 2.0;
            Some(path(&[
                (x, cy),
                (x + inset, y),
                (x + s - inset, y),
                (x + s, cy),
                (x + s - inset, y + s),
                (x + inset, y + s),
            ]))
        }
        // 判定菱形角标
        NodeShape::Diamond => Some(path(&[
            (x + s / 2.0, y),
            (x + s, y + s / 2.0),
            (x + s / 2.0, y + s),
            (x, y + s / 2.0),
        ])),
        // 平行四边形标记
        NodeShape::Parallelogram => {
            let slant = s * 0.25;
            Some(path(&[
                (x + slant, y),
                (x + s, y),
                (x + s - slant, y + s),
                (x, y + s),
            ]))
        }
        // 梯形标记
        NodeShape::Trapezoid => {
            let slant = s * 0.25;
            Some(path(&[
                (x + slant, y),
                (x + s - slant, y),
                (x + s, y + s),
                (x, y + s),
            ]))
        }
        // 旗形标记（外部实体）
        NodeShape::Asymmetric => {
            let slant = s * 0.3;
            Some(path(&[(x, y), (x + s, y), (x + s - slant, y + s), (x, y + s)]))
        }
        // 子程序标记：双竖线
        NodeShape::Subroutine => Some(DrawCmd::Decorated {
            inner: Box::new(DrawCmd::Path {
                segments: vec![
                    PathSegment::MoveTo(x + s * 0.3, y),
                    PathSegment::LineTo(x + s * 0.3, y + s),
                    PathSegment::MoveTo(x + s * 0.7, y),
                    PathSegment::LineTo(x + s * 0.7, y + s),
                ],
                fill: None,
                stroke: Some(stroke),
            }),
            decor: CmdDecor {
                line_cap: Some("round".to_string()),
                id: hit_id,
                ..Default::default()
            },
        }),
        // 纯展示形状 / 普通流程形状无 sigil
        NodeShape::Rectangle
        | NodeShape::RoundRect
        | NodeShape::Stadium
        | NodeShape::Circle
        | NodeShape::DoubleCircle => None,
    }
}

/// sigil 绘制原点 — 节点左上角内缩（Diamond 上顶点区域窄，取上中）
pub fn sigil_origin(nl: &crate::layout::NodeLayout) -> (f64, f64) {
    match nl.shape {
        NodeShape::Diamond => (nl.x + nl.width / 2.0 - SIGIL_SIZE / 2.0, nl.y + SIGIL_INSET),
        _ => (nl.x + SIGIL_INSET, nl.y + SIGIL_INSET),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{NodeLayout, TextBlock};
    use mermaid_canvas_core::interaction::BoundingBox;

    fn node(shape: NodeShape) -> NodeLayout {
        NodeLayout {
            id: "N".to_string(),
            x: 100.0,
            y: 50.0,
            width: 120.0,
            height: 60.0,
            label: TextBlock {
                text: "N".to_string(),
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                font_size: 14.0,
            },
            shape,
            bounds: BoundingBox::new(100.0, 50.0, 120.0, 60.0),
        }
    }

    #[test]
    fn test_semantic_shapes_have_sigils_plain_shapes_dont() {
        let with_sigil = [
            NodeShape::Cylinder,
            NodeShape::Hexagon,
            NodeShape::Diamond,
            NodeShape::Parallelogram,
            NodeShape::Trapezoid,
            NodeShape::Asymmetric,
            NodeShape::Subroutine,
        ];
        for shape in with_sigil {
            assert!(sigil_cmd(shape, (0.0, 0.0), "#333", Some(1)).is_some(), "{:?} 有 sigil", shape);
        }
        let without = [
            NodeShape::Rectangle,
            NodeShape::RoundRect,
            NodeShape::Stadium,
            NodeShape::Circle,
            NodeShape::DoubleCircle,
        ];
        for shape in without {
            assert!(sigil_cmd(shape, (0.0, 0.0), "#333", None).is_none(), "{:?} 无 sigil", shape);
        }
    }

    #[test]
    fn test_sigil_carries_id_and_round_cap() {
        let cmd = sigil_cmd(NodeShape::Hexagon, (10.0, 10.0), "#abc", Some(2)).unwrap();
        match cmd {
            DrawCmd::Decorated { decor, .. } => {
                assert_eq!(decor.id, Some(2), "sigil 与节点形状同 id");
                assert_eq!(decor.line_cap.as_deref(), Some("round"));
            }
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_cylinder_sigil_is_bezier_cylinder() {
        let cmd = sigil_cmd(NodeShape::Cylinder, (0.0, 0.0), "#333", None).unwrap();
        match cmd {
            DrawCmd::Decorated { inner, .. } => match *inner {
                DrawCmd::Path { segments, .. } => {
                    assert_eq!(segments.iter().filter(|s| matches!(s, PathSegment::BezierTo(..))).count(), 4, "上下弧各两段贝塞尔");
                }
                other => panic!("expected Path, got {:?}", other),
            },
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_sigil_origin_top_left_diamond_top_center() {
        let nl = node(NodeShape::Rectangle);
        assert_eq!(sigil_origin(&nl), (nl.x + SIGIL_INSET, nl.y + SIGIL_INSET));
        let d = node(NodeShape::Diamond);
        let (sx, _) = sigil_origin(&d);
        assert!((sx - (d.x + d.width / 2.0 - SIGIL_SIZE / 2.0)).abs() < 1e-9);
    }
}
