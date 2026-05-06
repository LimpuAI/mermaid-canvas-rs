//! Sequence 解析器 (placeholder)

use crate::diagram::{DiagramAst, DiagramKind};
use crate::error::CoreError;

/// 解析序列图语法
pub fn parse_sequence(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::Sequence);
    let _ = input;
    // TODO: 实现序列图解析
    Ok(ast)
}
