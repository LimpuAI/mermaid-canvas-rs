// mermaid-canvas-core: 纯计算核心，提供图表数据模型、Canvas 2D 指令、样式等基础能力
//
// 本 crate 是 mermaid-canvas-rs 图表渲染库的核心，不依赖任何前端框架或运行时，
// 仅负责图表模型转换和 Canvas 2D 指令生成。

#![warn(missing_docs)]
#![warn(clippy::all)]

//! # mermaid-canvas-core
//!
//! 纯计算核心，提供图表数据模型、Canvas 2D 指令、样式等基础能力。
//!
//! 本 crate 是 mermaid-canvas-rs 图表渲染库的核心，不依赖任何前端框架或运行时，
//! 仅负责图表模型转换和 Canvas 2D 指令生成。

pub mod diagram;
pub mod style;
pub mod instruction;
pub mod layer;
pub mod error;
pub mod interaction;
pub mod parser;

// 重新导出常用类型
pub use diagram::{
    DiagramKind, DiagramAst, DiagramNode, DiagramEdge, Subgraph,
    NodeShape, Direction, EdgeStyle, EdgeArrowhead, EdgeDecoration,
};
pub use style::{
    FillStyle, StrokeStyle, Gradient, GradientKind, GradientStop,
    TextStyle, FontWeight, FontStyle, TextAnchor, TextBaseline,
};
pub use instruction::{DrawCmd, PathSegment, CanvasOp, RenderOutput};
pub use layer::{LayerKind, Layer, RenderLayers};
pub use error::CoreError;
pub use interaction::{HitRegion, BoundingBox, HitResult};
pub use parser::parse_mermaid;
