//! Canvas 2D 指令类型定义
//!
//! 提供语义化的绘图指令和底层的 Canvas 操作指令。

use crate::style::{FillStyle, StrokeStyle, TextStyle, TextAnchor, TextBaseline};

/// 外阴影描述 — WIT `shadow-desc` 载体（宿主 SDF 高斯软阴影 pass 渲染；
/// 忌以实体描边/填充模拟——硬边阴影在入场缩放下呈厚重黑框）
#[derive(Clone, Debug, PartialEq)]
pub struct CmdShadow {
    /// x 偏移（px；正值向右）
    pub offset_x: f64,
    /// y 偏移（px；正值向下）
    pub offset_y: f64,
    /// 模糊半径（px；0 = 硬边）
    pub blur: f64,
    /// 扩散（px；正值扩张阴影轮廓，负值收缩）
    pub spread: f64,
    /// 阴影基色（hex/rgb/rgba 字符串；透明度以 alpha 字段为准）
    pub color: String,
    /// 阴影不透明度（0-1；最终 alpha = color 自身 alpha × alpha）
    pub alpha: f64,
    /// 阴影形状宽（px；绕宿主形状中心）。path 指令缺省 (0,0) = 包围盒
    pub width: f64,
    /// 阴影形状高（px；语义同 width）
    pub height: f64,
    /// 阴影形状旋转角（度；顺时针，绕宿主形状中心）
    pub rotation: f64,
}

/// 装饰通道 — WIT `echodawn:canvas@2.0.0/draw` 新字段（dash/line-cap/线宽/命令 id/shadow）的内部载体
///
/// 经 [`DrawCmd::Decorated`] 包装附加到任意指令；WIT 投影时并入展平指令，
/// 原生 canvas 路径消费 `stroke_width`（dash/line-cap/shadow 无对应 CanvasOp，忽略）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CmdDecor {
    /// 描边宽度（px）
    pub stroke_width: Option<f64>,
    /// 虚线节律（线段/间隙交替，px）；None = 实线
    pub dash: Option<Vec<f64>>,
    /// 线端帽："butt" | "round" | "square"；None = butt
    pub line_cap: Option<String>,
    /// 命令身份 = 所属 hit-region index（一对多；宿主 per-item 效果关联键）
    pub id: Option<u32>,
    /// 外阴影（None = 无）
    pub shadow: Option<CmdShadow>,
}

/// 绘图指令
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCmd {
    /// 矩形
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        fill: Option<FillStyle>,
        stroke: Option<StrokeStyle>,
        corner_radius: Option<f64>,
    },
    /// 路径
    Path {
        segments: Vec<PathSegment>,
        fill: Option<FillStyle>,
        stroke: Option<StrokeStyle>,
    },
    /// 圆形
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: Option<FillStyle>,
        stroke: Option<StrokeStyle>,
    },
    /// 文本
    Text {
        x: f64,
        y: f64,
        content: String,
        style: TextStyle,
        anchor: TextAnchor,
        baseline: TextBaseline,
    },
    /// 分组
    Group {
        label: Option<String>,
        items: Vec<DrawCmd>,
    },
    /// 装饰指令 — 附加 dash/line-cap/线宽/命令 id 等装饰通道到内层指令
    Decorated {
        /// 内层指令
        inner: Box<DrawCmd>,
        /// 装饰通道
        decor: CmdDecor,
    },
}

impl DrawCmd {
    /// 转换为 Canvas 操作序列
    pub fn to_canvas_ops(&self) -> Vec<CanvasOp> {
        match self {
            DrawCmd::Rect { x, y, width, height, fill, stroke, corner_radius } => {
                let mut ops = Vec::new();
                if let Some(radius) = corner_radius {
                    ops.extend(Self::rounded_rect_path(*x, *y, *width, *height, *radius, fill, stroke));
                } else {
                    if let Some(FillStyle::Color(color)) = fill {
                        ops.push(CanvasOp::SetFillStyle(color.clone()));
                        ops.push(CanvasOp::FillRect(*x, *y, *width, *height));
                    }
                    if let Some(StrokeStyle::Color(color)) = stroke {
                        ops.push(CanvasOp::SetStrokeStyle(color.clone()));
                        ops.push(CanvasOp::StrokeRect(*x, *y, *width, *height));
                    }
                }
                ops
            }
            DrawCmd::Path { segments, fill, stroke } => {
                let mut ops = Vec::new();
                if segments.is_empty() { return ops; }
                ops.push(CanvasOp::BeginPath);
                for segment in segments {
                    ops.extend(segment.to_canvas_ops());
                }
                if let Some(FillStyle::Color(color)) = fill {
                    ops.push(CanvasOp::SetFillStyle(color.clone()));
                    ops.push(CanvasOp::Fill);
                }
                if let Some(StrokeStyle::Color(color)) = stroke {
                    ops.push(CanvasOp::SetStrokeStyle(color.clone()));
                    ops.push(CanvasOp::Stroke);
                }
                ops
            }
            DrawCmd::Circle { cx, cy, r, fill, stroke } => {
                let mut ops = Vec::new();
                ops.push(CanvasOp::BeginPath);
                ops.push(CanvasOp::Arc(*cx, *cy, *r, 0.0, 2.0 * std::f64::consts::PI, false));
                if let Some(FillStyle::Color(color)) = fill {
                    ops.push(CanvasOp::SetFillStyle(color.clone()));
                    ops.push(CanvasOp::Fill);
                }
                if let Some(StrokeStyle::Color(color)) = stroke {
                    ops.push(CanvasOp::SetStrokeStyle(color.clone()));
                    ops.push(CanvasOp::Stroke);
                }
                ops
            }
            DrawCmd::Text { x, y, content, style, anchor, baseline } => {
                let mut ops = Vec::new();
                ops.push(CanvasOp::SetFont(style.to_css_font()));
                ops.push(CanvasOp::SetTextAlign(anchor.to_string()));
                ops.push(CanvasOp::SetTextBaseline(baseline.to_string()));
                if let FillStyle::Color(color) = &style.fill {
                    ops.push(CanvasOp::SetFillStyle(color.clone()));
                    ops.push(CanvasOp::FillText(content.clone(), *x, *y));
                }
                ops
            }
            DrawCmd::Group { label: _, items } => {
                let mut ops = Vec::new();
                ops.push(CanvasOp::Save);
                for item in items {
                    ops.extend(item.to_canvas_ops());
                }
                ops.push(CanvasOp::Restore);
                ops
            }
            DrawCmd::Decorated { inner, decor } => {
                // 线宽先行设置（canvas 状态机语义：描边取当下线宽）；
                // dash/line_cap 无对应 CanvasOp，仅 WIT 投影消费
                let mut ops = Vec::new();
                if let Some(w) = decor.stroke_width {
                    ops.push(CanvasOp::SetLineWidth(w));
                }
                ops.extend(inner.to_canvas_ops());
                ops
            }
        }
    }

    fn rounded_rect_path(
        x: f64, y: f64, width: f64, height: f64, radius: f64,
        fill: &Option<FillStyle>, stroke: &Option<StrokeStyle>,
    ) -> Vec<CanvasOp> {
        let mut ops = Vec::new();
        let r = radius.min(width / 2.0).min(height / 2.0);
        ops.push(CanvasOp::BeginPath);
        ops.push(CanvasOp::MoveTo(x + r, y));
        ops.push(CanvasOp::LineTo(x + width - r, y));
        ops.push(CanvasOp::QuadraticCurveTo(x + width, y, x + width, y + r));
        ops.push(CanvasOp::LineTo(x + width, y + height - r));
        ops.push(CanvasOp::QuadraticCurveTo(x + width, y + height, x + width - r, y + height));
        ops.push(CanvasOp::LineTo(x + r, y + height));
        ops.push(CanvasOp::QuadraticCurveTo(x, y + height, x, y + height - r));
        ops.push(CanvasOp::LineTo(x, y + r));
        ops.push(CanvasOp::QuadraticCurveTo(x, y, x + r, y));
        ops.push(CanvasOp::ClosePath);
        if let Some(FillStyle::Color(color)) = fill {
            ops.push(CanvasOp::SetFillStyle(color.clone()));
            ops.push(CanvasOp::Fill);
        }
        if let Some(StrokeStyle::Color(color)) = stroke {
            ops.push(CanvasOp::SetStrokeStyle(color.clone()));
            ops.push(CanvasOp::Stroke);
        }
        ops
    }
}

/// 路径段
#[derive(Clone, Debug, PartialEq)]
pub enum PathSegment {
    /// 移动到
    MoveTo(f64, f64),
    /// 直线到
    LineTo(f64, f64),
    /// 三次贝塞尔曲线
    BezierTo(f64, f64, f64, f64, f64, f64),
    /// 二次贝塞尔曲线
    QuadraticTo(f64, f64, f64, f64),
    /// 圆弧
    Arc(f64, f64, f64, f64, f64, bool),
    /// 闭合路径
    Close,
}

impl PathSegment {
    /// 转换为 Canvas 操作
    pub fn to_canvas_ops(&self) -> Vec<CanvasOp> {
        match self {
            PathSegment::MoveTo(x, y) => vec![CanvasOp::MoveTo(*x, *y)],
            PathSegment::LineTo(x, y) => vec![CanvasOp::LineTo(*x, *y)],
            PathSegment::BezierTo(cp1x, cp1y, cp2x, cp2y, x, y) =>
                vec![CanvasOp::BezierCurveTo(*cp1x, *cp1y, *cp2x, *cp2y, *x, *y)],
            PathSegment::QuadraticTo(cpx, cpy, x, y) =>
                vec![CanvasOp::QuadraticCurveTo(*cpx, *cpy, *x, *y)],
            PathSegment::Arc(x, y, r, start, end, ccw) =>
                vec![CanvasOp::Arc(*x, *y, *r, *start, *end, *ccw)],
            PathSegment::Close => vec![CanvasOp::ClosePath],
        }
    }
}

/// Canvas 操作
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasOp {
    Save,
    Restore,
    SetFillStyle(String),
    SetStrokeStyle(String),
    SetLineWidth(f64),
    SetFont(String),
    SetTextAlign(String),
    SetTextBaseline(String),
    BeginPath,
    ClosePath,
    MoveTo(f64, f64),
    LineTo(f64, f64),
    BezierCurveTo(f64, f64, f64, f64, f64, f64),
    QuadraticCurveTo(f64, f64, f64, f64),
    Arc(f64, f64, f64, f64, f64, bool),
    Fill,
    Stroke,
    FillRect(f64, f64, f64, f64),
    StrokeRect(f64, f64, f64, f64),
    ClearRect(f64, f64, f64, f64),
    FillText(String, f64, f64),
    StrokeText(String, f64, f64),
}

/// 渲染输出
#[derive(Clone, Debug, PartialEq)]
pub struct RenderOutput {
    /// 语义指令
    pub semantic: Vec<DrawCmd>,
    /// Canvas 操作序列
    pub canvas_ops: Vec<CanvasOp>,
}

impl RenderOutput {
    /// 创建新的渲染输出
    pub fn new() -> Self {
        Self { semantic: Vec::new(), canvas_ops: Vec::new() }
    }

    /// 添加语义指令
    pub fn add_command(&mut self, cmd: DrawCmd) {
        self.semantic.push(cmd.clone());
        self.canvas_ops.extend(cmd.to_canvas_ops());
    }

    /// 扩展多个语义指令
    pub fn extend_commands(&mut self, cmds: impl IntoIterator<Item = DrawCmd>) {
        for cmd in cmds { self.add_command(cmd); }
    }

    /// 清空输出
    pub fn clear(&mut self) {
        self.semantic.clear();
        self.canvas_ops.clear();
    }

    /// 判断是否为空
    pub fn is_empty(&self) -> bool { self.semantic.is_empty() }

    /// 获取指令数量
    pub fn len(&self) -> usize { self.semantic.len() }

    /// 从语义指令构建渲染输出
    pub fn from_commands(commands: Vec<DrawCmd>) -> Self {
        let mut output = Self::new();
        output.extend_commands(commands);
        output
    }
}

impl Default for RenderOutput {
    fn default() -> Self { Self::new() }
}

impl From<Vec<DrawCmd>> for RenderOutput {
    fn from(commands: Vec<DrawCmd>) -> Self { Self::from_commands(commands) }
}
