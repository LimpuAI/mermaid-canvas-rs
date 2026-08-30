//! 图表渲染器
//!
//! 对标 deneb-component 的 `chart/` 模块。
//! 负责将 Layout 转换为 Canvas 2D 指令 (DrawCmd)，生成 DiagramOutput。

pub mod arrow;
pub mod flowchart;
pub mod sequence;

use mermaid_canvas_core::{
    interaction::HitRegion,
    layer::RenderLayers,
    style::{FillStyle, Gradient, GradientKind, GradientStop, StrokeStyle},
    CmdDecor, DrawCmd, PathSegment,
};

use crate::layout::{Layout, TextBlock};
use crate::theme::{lighten_color, with_color_alpha, Theme};

/// 背景网格（竖横线族单 path，给定间距/alpha — R7 双尺度网格的共享构件）
pub fn grid_cmd<T: Theme>(layout: &Layout, theme: &T, spacing: f64, alpha: f64) -> DrawCmd {
    let color = with_color_alpha(theme.node_text_color(), alpha);
    let mut segments = Vec::new();
    let mut x = spacing;
    while x < layout.width {
        segments.push(PathSegment::MoveTo(x, 0.0));
        segments.push(PathSegment::LineTo(x, layout.height));
        x += spacing;
    }
    let mut y = spacing;
    while y < layout.height {
        segments.push(PathSegment::MoveTo(0.0, y));
        segments.push(PathSegment::LineTo(layout.width, y));
        y += spacing;
    }
    DrawCmd::Decorated {
        inner: Box::new(DrawCmd::Path {
            segments,
            fill: None,
            stroke: Some(StrokeStyle::Color(color)),
        }),
        decor: CmdDecor { stroke_width: Some(1.0), ..Default::default() },
    }
}

/// 背景顶部提光（"光自上来" — 底色提亮 45% 的垂直渐变叠加）
pub fn top_light_cmd<T: Theme>(layout: &Layout, theme: &T, alpha: f64) -> DrawCmd {
    let light = lighten_color(theme.background_color(), 0.45);
    DrawCmd::Rect {
        x: 0.0,
        y: 0.0,
        width: layout.width,
        height: layout.height,
        fill: Some(FillStyle::Gradient(Gradient {
            kind: GradientKind::Linear { x0: 0.0, y0: 0.0, x1: 0.0, y1: layout.height * 0.85 },
            stops: vec![
                GradientStop::new(0.0, with_color_alpha(&light, alpha)),
                GradientStop::new(1.0, with_color_alpha(&light, 0.0)),
            ],
        })),
        stroke: None,
        corner_radius: None,
    }
}

/// 边标签圆角底盘（TextBlock 测量尺寸 + padding;底色 = edge-label-background）
/// 锚点约定与文字一致：Middle/Bottom → 底盘以 (lx, ly) 为底边中点
pub fn edge_label_plate<T: Theme>(lx: f64, ly: f64, label: &TextBlock, theme: &T) -> DrawCmd {
    let pad_x = 5.0;
    let pad_y = 2.5;
    DrawCmd::Rect {
        x: lx - label.width / 2.0 - pad_x,
        y: ly - label.height - pad_y,
        width: label.width + pad_x * 2.0,
        height: label.height + pad_y * 2.0,
        fill: Some(FillStyle::Color(with_color_alpha(theme.edge_label_background(), 0.85))),
        stroke: None,
        corner_radius: Some(4.0),
    }
}

/// 边指令组归属标记 — 一条边的全部指令（辉光层/主线/箭头/端点装饰）
/// 统一携带布局边索引（`CmdDecor.id`），供会话层关联聚焦按边分组
/// （dim 非相关边 / 相连边脉冲）。会话消费后剥除 id：宿主 hover
/// 效果以节点 hit-index 命中，边指令残留 id 会造成跨命中串扰。
pub fn stamp_edge_id(cmds: Vec<DrawCmd>, edge_idx: u32) -> Vec<DrawCmd> {
    cmds.into_iter()
        .map(|c| match c {
            DrawCmd::Decorated { inner, mut decor } => {
                decor.id = Some(edge_idx);
                DrawCmd::Decorated { inner, decor }
            }
            other => DrawCmd::Decorated {
                inner: Box::new(other),
                decor: CmdDecor { id: Some(edge_idx), ..Default::default() },
            },
        })
        .collect()
}
/// 图表渲染输出 — 对标 deneb-component 的 ChartOutput
///
/// 包含分层渲染输出和命中测试区域，支持增量渲染和交互。
#[derive(Clone, Debug)]
pub struct DiagramOutput {
    /// 分层渲染输出
    pub layers: RenderLayers,
    /// 命中区域列表
    pub hit_regions: Vec<HitRegion>,
}

impl DiagramOutput {
    /// 创建新的 DiagramOutput
    pub fn new() -> Self {
        Self {
            layers: RenderLayers::new(),
            hit_regions: Vec::new(),
        }
    }

    /// 创建带渲染层的 DiagramOutput
    pub fn with_layers(layers: RenderLayers) -> Self {
        Self {
            layers,
            hit_regions: Vec::new(),
        }
    }

    /// 添加命中区域
    pub fn add_hit_region(&mut self, region: HitRegion) {
        self.hit_regions.push(region);
    }
}

impl Default for DiagramOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl From<RenderLayers> for DiagramOutput {
    fn from(layers: RenderLayers) -> Self {
        Self::with_layers(layers)
    }
}

pub use flowchart::FlowchartRenderer;
pub use sequence::SequenceRenderer;
