//! Core 错误类型定义

use std::fmt;

/// Core 错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    /// 解析错误
    ParseError {
        source: String,
    },
    /// 布局错误
    LayoutError {
        reason: String,
    },
    /// 渲染错误
    RenderError {
        reason: String,
    },
    /// 无效输入
    InvalidInput {
        reason: String,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::ParseError { source } => write!(f, "Parse error: {}", source),
            CoreError::LayoutError { reason } => write!(f, "Layout error: {}", reason),
            CoreError::RenderError { reason } => write!(f, "Render error: {}", reason),
            CoreError::InvalidInput { reason } => write!(f, "Invalid input: {}", reason),
        }
    }
}

impl std::error::Error for CoreError {}

impl CoreError {
    /// 创建解析错误
    pub fn parse_error(source: impl Into<String>) -> Self { CoreError::ParseError { source: source.into() } }
    /// 创建布局错误
    pub fn layout_error(reason: impl Into<String>) -> Self { CoreError::LayoutError { reason: reason.into() } }
    /// 创建渲染错误
    pub fn render_error(reason: impl Into<String>) -> Self { CoreError::RenderError { reason: reason.into() } }
    /// 创建无效输入错误
    pub fn invalid_input(reason: impl Into<String>) -> Self { CoreError::InvalidInput { reason: reason.into() } }
}
