//! 箭头/装饰 path 族 — 边端点的矢量符号（T11）
//!
//! 按路由末段（首段）方向向量在端点生成箭头 path：
//! - `EdgeArrowhead::Arrow` 实心三角（fill）
//! - `EdgeArrowhead::OpenTriangle` 开放 V（stroke，round 端帽）
//! - `EdgeArrowhead::Circle` 实心圆头（fill）
//! - `EdgeArrowhead::Cross` 叉号（stroke）
//! - `EdgeArrowhead::Diamond` 菱形头（fill）
//! 装饰（`o--o` / `x--x`）为端点旁的空心圆/叉号。尺寸基准 9px（规格 8-10px）。

use mermaid_canvas_core::{
    instruction::{CmdDecor, DrawCmd, PathSegment},
    style::{FillStyle, StrokeStyle},
    EdgeArrowhead, EdgeDecoration,
};

/// 箭头基准尺寸（px）
pub const ARROW_SIZE: f64 = 9.0;

/// 单位方向向量 from → to（零向量退化为 +x）
fn unit_dir(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

/// 端点便捷元组：`(tip, dir)` — dir 为指向端点的单位向量
pub fn end_tip_dir(points: &[(f64, f64)]) -> ((f64, f64), (f64, f64)) {
    let tip = points.last().copied().unwrap_or((0.0, 0.0));
    let n = points.len();
    let dir = if n >= 2 {
        unit_dir(points[n - 2], points[n - 1])
    } else {
        (1.0, 0.0)
    };
    (tip, dir)
}

/// 起点便捷元组：`(tip, dir)` — dir 为指向起点的单位向量
pub fn start_tip_dir(points: &[(f64, f64)]) -> ((f64, f64), (f64, f64)) {
    let tip = points.first().copied().unwrap_or((0.0, 0.0));
    let dir = if points.len() >= 2 {
        unit_dir(points[1], points[0])
    } else {
        (-1.0, 0.0)
    };
    (tip, dir)
}

/// 垂直向量（顺时针旋转 90°）
fn perp(d: (f64, f64)) -> (f64, f64) {
    (-d.1, d.0)
}

fn add(p: (f64, f64), d: (f64, f64), k: f64) -> (f64, f64) {
    (p.0 + d.0 * k, p.1 + d.1 * k)
}

/// 三角/菱形/开放 V 的角点
fn arrow_corners(tip: (f64, f64), dir: (f64, f64), size: f64) -> ((f64, f64), (f64, f64)) {
    let p = perp(dir);
    let half = size * 0.42;
    let base = add(tip, dir, -size);
    (add(base, p, half), add(base, p, -half))
}

/// 箭头指令 — 端点 tip、方向 dir（指向 tip）、线色；thick 时加粗描边
pub fn arrowhead_cmd(
    tip: (f64, f64),
    dir: (f64, f64),
    kind: EdgeArrowhead,
    color: &str,
    thick: bool,
) -> DrawCmd {
    let decor = if thick {
        CmdDecor { stroke_width: Some(2.5), ..Default::default() }
    } else {
        CmdDecor::default()
    };
    let stroke = StrokeStyle::Color(color.to_string());

    match kind {
        EdgeArrowhead::Arrow => {
            // 实心三角
            let (p1, p2) = arrow_corners(tip, dir, ARROW_SIZE);
            DrawCmd::Decorated {
                inner: Box::new(DrawCmd::Path {
                    segments: vec![
                        PathSegment::MoveTo(tip.0, tip.1),
                        PathSegment::LineTo(p1.0, p1.1),
                        PathSegment::LineTo(p2.0, p2.1),
                        PathSegment::Close,
                    ],
                    fill: Some(FillStyle::Color(color.to_string())),
                    stroke: None,
                }),
                decor: CmdDecor::default(),
            }
        }
        EdgeArrowhead::OpenTriangle => {
            // 开放 V（stroke + round 端帽）
            let (p1, p2) = arrow_corners(tip, dir, ARROW_SIZE);
            DrawCmd::Decorated {
                inner: Box::new(DrawCmd::Path {
                    segments: vec![
                        PathSegment::MoveTo(p1.0, p1.1),
                        PathSegment::LineTo(tip.0, tip.1),
                        PathSegment::LineTo(p2.0, p2.1),
                    ],
                    fill: None,
                    stroke: Some(stroke),
                }),
                decor: CmdDecor {
                    line_cap: Some("round".to_string()),
                    ..decor
                },
            }
        }
        EdgeArrowhead::Circle => {
            // 实心圆头（中心沿方向内缩，圆缘贴端点）
            let center = add(tip, dir, -ARROW_SIZE * 0.35);
            DrawCmd::Decorated {
                inner: Box::new(DrawCmd::Circle {
                    cx: center.0,
                    cy: center.1,
                    r: ARROW_SIZE * 0.32,
                    fill: Some(FillStyle::Color(color.to_string())),
                    stroke: None,
                }),
                decor: CmdDecor::default(),
            }
        }
        EdgeArrowhead::Cross => {
            // 叉号（沿方向轴 + 垂直轴两笔）
            let center = add(tip, dir, -ARROW_SIZE * 0.5);
            let k = ARROW_SIZE * 0.32;
            let p = perp(dir);
            let (a1, a2) = (add(center, dir, k), add(center, dir, -k));
            let (b1, b2) = (add(center, p, k), add(center, p, -k));
            DrawCmd::Decorated {
                inner: Box::new(DrawCmd::Path {
                    segments: vec![
                        PathSegment::MoveTo(a1.0, a1.1),
                        PathSegment::LineTo(a2.0, a2.1),
                        PathSegment::MoveTo(b1.0, b1.1),
                        PathSegment::LineTo(b2.0, b2.1),
                    ],
                    fill: None,
                    stroke: Some(stroke),
                }),
                decor: CmdDecor {
                    line_cap: Some("round".to_string()),
                    ..decor
                },
            }
        }
        EdgeArrowhead::Diamond => {
            // 菱形头（tip → 侧点 → 尾 → 侧点）
            let p = perp(dir);
            let half = ARROW_SIZE * 0.42;
            let side1 = add(add(tip, dir, -ARROW_SIZE * 0.5), p, half);
            let side2 = add(add(tip, dir, -ARROW_SIZE * 0.5), p, -half);
            let tail = add(tip, dir, -ARROW_SIZE);
            DrawCmd::Decorated {
                inner: Box::new(DrawCmd::Path {
                    segments: vec![
                        PathSegment::MoveTo(tip.0, tip.1),
                        PathSegment::LineTo(side1.0, side1.1),
                        PathSegment::LineTo(tail.0, tail.1),
                        PathSegment::LineTo(side2.0, side2.1),
                        PathSegment::Close,
                    ],
                    fill: Some(FillStyle::Color(color.to_string())),
                    stroke: None,
                }),
                decor: CmdDecor::default(),
            }
        }
    }
}

/// 端点装饰（`o--o` 空心圆 / `x--x` 叉号）— 紧贴端点内侧
pub fn decoration_cmd(
    tip: (f64, f64),
    dir: (f64, f64),
    deco: EdgeDecoration,
    color: &str,
) -> DrawCmd {
    match deco {
        EdgeDecoration::Circle => {
            let center = add(tip, dir, -5.0);
            DrawCmd::Circle {
                cx: center.0,
                cy: center.1,
                r: 4.0,
                fill: None,
                stroke: Some(StrokeStyle::Color(color.to_string())),
            }
        }
        EdgeDecoration::Cross => {
            let center = add(tip, dir, -5.0);
            let k = 3.0;
            let p = perp(dir);
            let (a1, a2) = (add(center, dir, k), add(center, dir, -k));
            let (b1, b2) = (add(center, p, k), add(center, p, -k));
            DrawCmd::Path {
                segments: vec![
                    PathSegment::MoveTo(a1.0, a1.1),
                    PathSegment::LineTo(a2.0, a2.1),
                    PathSegment::MoveTo(b1.0, b1.1),
                    PathSegment::LineTo(b2.0, b2.1),
                ],
                fill: None,
                stroke: Some(StrokeStyle::Color(color.to_string())),
            }
        }
    }
}

/// EdgeStyle → 装饰通道（T12 — Dashed/Dotted/Thick 线型消费）
pub fn edge_style_decor(style: mermaid_canvas_core::EdgeStyle) -> CmdDecor {
    use mermaid_canvas_core::EdgeStyle;
    match style {
        EdgeStyle::Solid => CmdDecor::default(),
        EdgeStyle::Dashed => CmdDecor {
            dash: Some(vec![6.0, 4.0]),
            ..Default::default()
        },
        EdgeStyle::Dotted => CmdDecor {
            dash: Some(vec![2.0, 3.0]),
            line_cap: Some("round".to_string()),
            ..Default::default()
        },
        EdgeStyle::Thick => CmdDecor {
            stroke_width: Some(2.5),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solid_arrow_triangle_points_forward() {
        // tip=(100,0)，dir=+x：三角在 tip 左侧展开
        let cmd = arrowhead_cmd((100.0, 0.0), (1.0, 0.0), EdgeArrowhead::Arrow, "#333", false);
        match cmd {
            DrawCmd::Decorated { inner, .. } => match *inner {
                DrawCmd::Path { segments, fill, .. } => {
                    assert!(matches!(segments[0], PathSegment::MoveTo(x, y) if (x - 100.0).abs() < 1e-9 && y.abs() < 1e-9));
                    // 基点在 tip - 9px
                    match segments[1] {
                        PathSegment::LineTo(x, y) => {
                            assert!((x - 91.0).abs() < 1e-9, "base x = {}", x);
                            assert!((y - 3.78).abs() < 0.01, "half width = {}", y);
                        }
                        ref other => panic!("expected LineTo, got {:?}", other),
                    }
                    assert!(fill.is_some(), "实心箭头带 fill");
                }
                other => panic!("expected Path, got {:?}", other),
            },
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_open_triangle_is_stroke_only() {
        let cmd = arrowhead_cmd((0.0, 0.0), (0.0, 1.0), EdgeArrowhead::OpenTriangle, "#666", false);
        match cmd {
            DrawCmd::Decorated { inner, decor } => {
                assert!(decor.line_cap.as_deref() == Some("round"), "开放箭头 round 端帽");
                match *inner {
                    DrawCmd::Path { segments, fill, stroke } => {
                        assert!(fill.is_none());
                        assert!(stroke.is_some());
                        assert_eq!(segments.len(), 3, "V 形三段");
                        assert!(!segments.iter().any(|s| matches!(s, PathSegment::Close)));
                    }
                    other => panic!("expected Path, got {:?}", other),
                }
            }
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_thick_arrow_gets_wider_stroke() {
        let cmd = arrowhead_cmd((0.0, 0.0), (1.0, 0.0), EdgeArrowhead::OpenTriangle, "#333", true);
        match cmd {
            DrawCmd::Decorated { decor, .. } => assert_eq!(decor.stroke_width, Some(2.5)),
            other => panic!("expected Decorated, got {:?}", other),
        }
    }

    #[test]
    fn test_end_direction_from_polyline() {
        let (tip, dir) = end_tip_dir(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert_eq!(tip, (10.0, 10.0));
        assert!((dir.0 - 0.0).abs() < 1e-9 && (dir.1 - 1.0).abs() < 1e-9, "末段竖直向下");
    }

    #[test]
    fn test_start_direction_points_into_start() {
        let (tip, dir) = start_tip_dir(&[(5.0, 5.0), (15.0, 5.0)]);
        assert_eq!(tip, (5.0, 5.0));
        assert!((dir.0 - (-1.0)).abs() < 1e-9, "指回起点（-x）");
    }

    #[test]
    fn test_edge_style_decor_mapping() {
        use mermaid_canvas_core::EdgeStyle;
        assert!(edge_style_decor(EdgeStyle::Solid) == CmdDecor::default());
        assert_eq!(edge_style_decor(EdgeStyle::Dashed).dash, Some(vec![6.0, 4.0]));
        let dotted = edge_style_decor(EdgeStyle::Dotted);
        assert_eq!(dotted.dash, Some(vec![2.0, 3.0]));
        assert_eq!(dotted.line_cap.as_deref(), Some("round"));
        assert_eq!(edge_style_decor(EdgeStyle::Thick).stroke_width, Some(2.5));
    }
}
