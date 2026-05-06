// mermaid-canvas-component: 图表实现层
//
// 本 crate 基于 mermaid-canvas-core 提供的基础类型，实现布局计算和图表渲染。
// 解析职责在 core 中（与 deneb-rs 中 deneb-core 拥有 parser 一致）。

#![warn(missing_docs)]
#![warn(clippy::all)]

//! # mermaid-canvas-component
//!
//! 图表实现层。
//!
//! 本 crate 基于 mermaid-canvas-core 提供的基础类型，实现布局计算和图表渲染。
//! 职责对标 deneb-component：layout + diagram renderer（无 parser）。

pub mod error;
pub mod theme;
pub mod config;
pub mod layout;
pub mod diagram;

pub use error::ComponentError;
pub use theme::{Theme, Margin, DefaultTheme, DarkTheme, ForestTheme, NordicTheme, CappuccinoTheme};
pub use config::LayoutConfig;
pub use layout::{Layout, NodeLayout, EdgeLayout, SubgraphLayout, TextBlock, compute_layout};
pub use diagram::{DiagramOutput, FlowchartRenderer, SequenceRenderer};

// 重新导出 mermaid-canvas-core 的类型
pub use mermaid_canvas_core;
