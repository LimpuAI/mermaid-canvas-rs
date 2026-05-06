//! Sequence 渲染器 (placeholder)

use crate::diagram::DiagramOutput;
use crate::error::ComponentError;
use crate::layout::Layout;
use crate::theme::Theme;

/// 序列图渲染器
pub struct SequenceRenderer;

impl SequenceRenderer {
    /// 渲染序列图
    pub fn render<T: Theme>(
        _layout: &Layout,
        _theme: &T,
    ) -> Result<DiagramOutput, ComponentError> {
        // TODO: 实现序列图渲染
        Ok(DiagramOutput::new())
    }
}
