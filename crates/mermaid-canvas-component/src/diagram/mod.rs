//! 图表渲染器
//!
//! 对标 deneb-component 的 `chart/` 模块。
//! 负责将 Layout 转换为 Canvas 2D 指令 (DrawCmd)，生成 DiagramOutput。

pub mod flowchart;
pub mod sequence;

use mermaid_canvas_core::{interaction::HitRegion, layer::RenderLayers};

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
