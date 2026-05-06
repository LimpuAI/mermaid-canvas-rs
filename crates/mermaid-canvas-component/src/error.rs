//! Component 错误类型

use std::fmt;

/// Component 错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentError {
    /// 解析错误
    ParseError {
        /// 错误描述
        message: String,
        /// 行号
        line: Option<u32>,
    },
    /// 布局错误
    LayoutError {
        /// 错误描述
        reason: String,
    },
    /// 渲染错误
    RenderError {
        /// 错误描述
        reason: String,
    },
    /// 不支持的图表类型
    UnsupportedDiagram {
        /// 图表类型名称
        kind: String,
    },
}

impl fmt::Display for ComponentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComponentError::ParseError { message, line } => {
                if let Some(line) = line {
                    write!(f, "Parse error at line {}: {}", line, message)
                } else {
                    write!(f, "Parse error: {}", message)
                }
            }
            ComponentError::LayoutError { reason } => write!(f, "Layout error: {}", reason),
            ComponentError::RenderError { reason } => write!(f, "Render error: {}", reason),
            ComponentError::UnsupportedDiagram { kind } => {
                write!(f, "Unsupported diagram type: {}", kind)
            }
        }
    }
}

impl std::error::Error for ComponentError {}
