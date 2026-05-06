//! 布局引擎 — Sugiyama-style layered graph layout
//!
//! Public API: `compute_layout(ast, theme, config) -> Layout`
//! Dispatches by `DiagramKind` to the appropriate layout strategy.

mod ranking;
mod positioning;
mod routing;
pub mod sequence_layout;

use std::collections::BTreeMap;
use mermaid_canvas_core::{DiagramAst, DiagramKind, interaction::BoundingBox};
use crate::theme::Theme;
use crate::config::LayoutConfig;

/// 文本块
#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    /// 文本内容
    pub text: String,
    /// x 坐标
    pub x: f64,
    /// y 坐标
    pub y: f64,
    /// 宽度
    pub width: f64,
    /// 高度
    pub height: f64,
    /// 字体大小
    pub font_size: f64,
}

/// 节点布局结果
#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    /// 节点 ID
    pub id: String,
    /// x 坐标
    pub x: f64,
    /// y 坐标
    pub y: f64,
    /// 宽度
    pub width: f64,
    /// 高度
    pub height: f64,
    /// 标签
    pub label: TextBlock,
    /// 节点形状
    pub shape: mermaid_canvas_core::NodeShape,
    /// 包围盒
    pub bounds: BoundingBox,
}

/// 边布局结果
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLayout {
    /// 起始节点 ID
    pub from: String,
    /// 目标节点 ID
    pub to: String,
    /// 路由点
    pub points: Vec<(f64, f64)>,
    /// 边标签
    pub label: Option<TextBlock>,
    /// 标签锚点
    pub label_anchor: Option<(f64, f64)>,
    /// 是否有向
    pub directed: bool,
}

/// 子图布局结果
#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphLayout {
    /// 子图 ID
    pub id: String,
    /// 标签
    pub label: TextBlock,
    /// x 坐标
    pub x: f64,
    /// y 坐标
    pub y: f64,
    /// 宽度
    pub width: f64,
    /// 高度
    pub height: f64,
}

/// 布局结果
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// 整体宽度
    pub width: f64,
    /// 整体高度
    pub height: f64,
    /// 节点布局
    pub nodes: BTreeMap<String, NodeLayout>,
    /// 边布局
    pub edges: Vec<EdgeLayout>,
    /// 子图布局
    pub subgraphs: Vec<SubgraphLayout>,
}

/// 计算布局
pub fn compute_layout<T: Theme>(
    ast: &DiagramAst,
    theme: &T,
    config: &LayoutConfig,
) -> Layout {
    match ast.kind {
        DiagramKind::Flowchart
        | DiagramKind::Class
        | DiagramKind::State
        | DiagramKind::Er
        | DiagramKind::Requirement
        | DiagramKind::Packet => compute_graph_layout(ast, theme, config),
        DiagramKind::Sequence => {
            sequence_layout::compute_sequence_layout(ast, theme, config)
        }
        // 其他图表类型暂返回空布局
        _ => Layout {
            width: 800.0,
            height: 600.0,
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
        },
    }
}

/// Sugiyama-style layered graph layout for flowchart-family diagrams
fn compute_graph_layout<T: Theme>(
    ast: &DiagramAst,
    theme: &T,
    config: &LayoutConfig,
) -> Layout {
    // Handle empty graphs
    if ast.nodes.is_empty() {
        return Layout {
            width: 100.0,
            height: 100.0,
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
        };
    }

    // Step 1: Assign ranks (layers) to nodes
    let (ranks, rank_map) = ranking::assign_ranks(ast);

    // Step 2: Order nodes within ranks using barycenter heuristic
    let mut ranks = ranks;
    ranking::order_nodes(&mut ranks, &ast.edges, config.ranking_passes, &rank_map);

    // Step 3: Assign x,y positions
    let (nodes, total_w, total_h) =
        positioning::assign_positions(&ranks, &rank_map, ast, config, theme);

    // Step 4: Route edges
    let edges = routing::route_edges(
        &ast.edges,
        &nodes,
        &ranks,
        &rank_map,
        ast.direction,
    );

    Layout {
        width: total_w,
        height: total_h,
        nodes,
        edges,
        subgraphs: Vec::new(),
    }
}
