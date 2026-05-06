//! mermaid-canvas-wit: WASI 集成层
//!
//! 提供两种使用模式：
//! 1. 库调用模式：宿主直接调用 Rust API
//! 2. 独立组件模式：通过 WIT 接口作为 WASI 组件运行

#![warn(clippy::all)]
#![allow(missing_docs)] // WIT type fields are self-documenting

pub use mermaid_canvas_core;
pub use mermaid_canvas_component;

/// WIT 类型定义 — 与 world.wit 中的 record 一一对应
pub mod wit_types;

/// 类型转换层 — WIT types ↔ internal types
pub mod convert;

/// 库调用模式 API
pub mod lib_mode;

/// 独立组件模式
pub mod component_mode;

// 重新导出 WIT 类型
pub use wit_types::*;
pub use lib_mode::*;
