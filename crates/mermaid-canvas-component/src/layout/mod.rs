//! 布局引擎 — Sugiyama-style layered graph layout
//!
//! Public API: `compute_layout(ast, theme, config) -> Layout`
//! Dispatches by `DiagramKind` to the appropriate layout strategy.

mod ranking;
mod positioning;
mod routing;
pub mod sequence_layout;

use std::collections::BTreeMap;
use mermaid_canvas_core::{DiagramAst, DiagramKind, EdgeArrowhead, EdgeDecoration, EdgeStyle, interaction::BoundingBox};
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

impl TextBlock {
    /// 空占位块（无文本的内部结构使用）
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            font_size: 12.0,
        }
    }
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
    /// 起始端箭头（T11 — 按首段方向绘制）
    pub arrow_start: Option<EdgeArrowhead>,
    /// 结束端箭头（按路由末段方向绘制）
    pub arrow_end: Option<EdgeArrowhead>,
    /// 起始端装饰（circle/cross）
    pub start_decoration: Option<EdgeDecoration>,
    /// 结束端装饰
    pub end_decoration: Option<EdgeDecoration>,
    /// 边样式（T12 — Dashed/Dotted/Thick → dash 节律/线宽）
    pub style: EdgeStyle,
}

impl EdgeLayout {
    /// 以 Solid 实线构造基础边（序列图生命线等内部结构使用）
    pub fn plain(from: String, to: String, points: Vec<(f64, f64)>, directed: bool) -> Self {
        Self {
            from,
            to,
            points,
            label: None,
            label_anchor: None,
            directed,
            arrow_start: None,
            arrow_end: None,
            start_decoration: None,
            end_decoration: None,
            style: EdgeStyle::Solid,
        }
    }
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
    /// 图表标题（T15 — title 层载体；None = 无标题带）
    pub title: Option<TextBlock>,
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
            title: None,
        },
    }
}

/// 子图边界框内边距（水平/垂直）
const SUBGRAPH_PADDING: f64 = 16.0;
/// 子图标签预留带高（标题文字 + 呼吸空间）
const SUBGRAPH_LABEL_BAND: f64 = 24.0;
/// 标题带与内容的间距
const TITLE_GAP: f64 = 10.0;

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
            title: None,
        };
    }

    // Step 1: Assign ranks (layers) to nodes
    let (ranks, rank_map) = ranking::assign_ranks(ast);

    // Step 2: Order nodes within ranks using barycenter heuristic
    let mut ranks = ranks;
    ranking::order_nodes(&mut ranks, &ast.edges, config.ranking_passes, &rank_map);
    // 同 subgraph 节点在秩内相邻排布（稳定排序保持 barycenter 序）
    group_ranks_by_subgraph(&mut ranks, ast);

    // Step 3: Assign x,y positions（含标题带预留）
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

    // Step 5: 子图边界框（成员节点包围盒 + 内边距；嵌套外框包含内框）
    let subgraphs = compute_subgraph_layouts(ast, &nodes);
    let (total_w, total_h) = expand_for_subgraphs(total_w, total_h, &subgraphs);

    // 标题带（顶部居中；Title 层渲染）
    let title = ast.title.as_ref().map(|text| {
        let fs = theme.title_font_size();
        TextBlock {
            text: text.clone(),
            x: total_w / 2.0,
            y: positioning::CANVAS_MARGIN / 2.0 + fs / 2.0,
            width: text.chars().count() as f64 * fs * 0.6,
            height: fs * 1.4,
            font_size: fs,
        }
    });

    Layout {
        width: total_w,
        height: total_h,
        nodes,
        edges,
        subgraphs,
        title,
    }
}

/// 秩内按 subgraph 成员关系稳定重排：同 subgraph 的节点相邻（key = 最小 subgraph 序）
fn group_ranks_by_subgraph(ranks: &mut [Vec<String>], ast: &DiagramAst) {
    if ast.subgraphs.is_empty() {
        return;
    }
    let mut node_key: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (idx, sg) in ast.subgraphs.iter().enumerate() {
        for nid in &sg.nodes {
            let e = node_key.entry(nid.as_str()).or_insert(usize::MAX);
            if idx < *e {
                *e = idx;
            }
        }
    }
    for rank in ranks.iter_mut() {
        // 稳定排序：同 key 保持 barycenter 顺序，无 subgraph（MAX）沉底
        rank.sort_by_key(|id| node_key.get(id.as_str()).copied().unwrap_or(usize::MAX));
    }
}

/// 子图边界框族：成员节点包围盒 + 内边距 + 标签带；嵌套（成员集为超集）外框扩张包含内框
fn compute_subgraph_layouts(
    ast: &DiagramAst,
    nodes: &BTreeMap<String, NodeLayout>,
) -> Vec<SubgraphLayout> {
    let mut layouts: Vec<SubgraphLayout> = Vec::new();
    for sg in &ast.subgraphs {
        let members: Vec<&NodeLayout> = sg.nodes.iter()
            .filter_map(|id| nodes.get(id))
            .collect();
        if members.is_empty() {
            continue;
        }
        let min_x = members.iter().map(|m| m.x).fold(f64::INFINITY, f64::min);
        let min_y = members.iter().map(|m| m.y).fold(f64::INFINITY, f64::min);
        let max_x = members.iter().map(|m| m.x + m.width).fold(f64::NEG_INFINITY, f64::max);
        let max_y = members.iter().map(|m| m.y + m.height).fold(f64::NEG_INFINITY, f64::max);

        let x = min_x - SUBGRAPH_PADDING;
        let y = min_y - SUBGRAPH_PADDING - SUBGRAPH_LABEL_BAND;
        let width = (max_x + SUBGRAPH_PADDING) - x;
        let height = (max_y + SUBGRAPH_PADDING) - y;

        layouts.push(SubgraphLayout {
            id: sg.id.clone(),
            label: TextBlock {
                text: sg.label.clone(),
                x: x + SUBGRAPH_PADDING,
                y: y + SUBGRAPH_LABEL_BAND / 2.0,
                width: sg.label.chars().count() as f64 * 8.0,
                height: 14.0,
                font_size: 12.0,
            },
            x,
            y,
            width,
            height,
        });
    }

    // 嵌套包含：成员集为超集的外框扩张到包含内框（迭代至稳定 — 嵌套层数有限）
    let member_sets: Vec<std::collections::HashSet<&String>> = ast.subgraphs.iter()
        .map(|sg| sg.nodes.iter().collect())
        .collect();
    for _ in 0..layouts.len() {
        let mut changed = false;
        for outer in 0..layouts.len() {
            for inner in 0..layouts.len() {
                if outer == inner {
                    continue;
                }
                // outer 成员集严格包含 inner 成员集 → outer 框须包含 inner 框
                let is_superset = member_sets[outer].len() > member_sets[inner].len()
                    && member_sets[inner].iter().all(|n| member_sets[outer].contains(n));
                if !is_superset {
                    continue;
                }
                let o = layouts[outer].clone();
                let i = &layouts[inner];
                let new_x = o.x.min(i.x - SUBGRAPH_PADDING);
                let new_y = o.y.min(i.y - SUBGRAPH_PADDING);
                let new_right = (o.x + o.width).max(i.x + i.width + SUBGRAPH_PADDING);
                let new_bottom = (o.y + o.height).max(i.y + i.height + SUBGRAPH_PADDING);
                if new_x != o.x || new_y != o.y
                    || new_right != o.x + o.width || new_bottom != o.y + o.height {
                    let l = &mut layouts[outer];
                    l.label.x += new_x - l.x;
                    l.label.y += new_y - l.y;
                    l.x = new_x;
                    l.y = new_y;
                    l.width = new_right - new_x;
                    l.height = new_bottom - new_y;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    layouts
}

/// 画布尺寸扩张到覆盖全部子图框
fn expand_for_subgraphs(total_w: f64, total_h: f64, subgraphs: &[SubgraphLayout]) -> (f64, f64) {
    let mut w = total_w;
    let mut h = total_h;
    for sg in subgraphs {
        w = w.max(sg.x + sg.width);
        h = h.max(sg.y + sg.height);
    }
    (w, h)
}
