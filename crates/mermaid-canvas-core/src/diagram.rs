//! 图表数据模型 — Mermaid 图表的中间表示 (AST)

use std::collections::BTreeMap;

/// 图表类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagramKind {
    /// 流程图
    Flowchart,
    /// 序列图
    Sequence,
    /// 类图
    Class,
    /// 状态图
    State,
    /// ER 图
    Er,
    /// 饼图
    Pie,
    /// 思维导图
    Mindmap,
    /// 用户旅程图
    Journey,
    /// 时间线
    Timeline,
    /// 甘特图
    Gantt,
    /// 需求图
    Requirement,
    /// Git 图
    GitGraph,
    /// C4 架构图
    C4,
    /// 桑基图
    Sankey,
    /// 象限图
    Quadrant,
    /// 块图
    Block,
    /// 报文图
    Packet,
    /// 看板
    Kanban,
    /// 架构图
    Architecture,
    /// 雷达图
    Radar,
    /// 树图
    Treemap,
    /// XY 图表
    XYChart,
}

impl DiagramKind {
    /// 所有支持的图表类型名称
    pub fn all_names() -> &'static [&'static str] {
        &[
            "flowchart", "sequenceDiagram", "classDiagram", "stateDiagram-v2",
            "erDiagram", "pie", "mindmap", "journey", "timeline", "gantt",
            "requirementDiagram", "gitGraph", "C4Context", "sankey",
            "quadrantChart", "block", "packet", "kanban", "architecture",
            "radar", "treemap", "xychart",
        ]
    }
}

/// 流图方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// 自上而下
    TopDown,
    /// 自下而上
    BottomUp,
    /// 从左到右
    LeftToRight,
    /// 从右到左
    RightToLeft,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::TopDown
    }
}

/// 节点形状
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeShape {
    /// 矩形 [label]
    Rectangle,
    /// 圆角矩形 (label)
    RoundRect,
    /// 体育场形 ([label])
    Stadium,
    /// 圆形 ((label))
    Circle,
    /// 双圆 (((label)))
    DoubleCircle,
    /// 菱形 {label}
    Diamond,
    /// 六边形 {{label}}
    Hexagon,
    /// 圆柱 [(label)]
    Cylinder,
    /// 子程序 [[label]]
    Subroutine,
    /// 平行四边形 [/label/]
    Parallelogram,
    /// 梯形 [\label\]
    Trapezoid,
    /// 不对称形 >label]
    Asymmetric,
}

impl Default for NodeShape {
    fn default() -> Self {
        NodeShape::RoundRect
    }
}

/// 边箭头类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeArrowhead {
    /// 标准箭头
    Arrow,
    /// 开放箭头
    OpenTriangle,
    /// 圆圈
    Circle,
    /// 叉号
    Cross,
    /// 菱形
    Diamond,
}

/// 边装饰
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeDecoration {
    /// 圆圈
    Circle,
    /// 叉号
    Cross,
}

/// 边样式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeStyle {
    /// 实线
    Solid,
    /// 虚线
    Dashed,
    /// 点线
    Dotted,
    /// 粗线
    Thick,
}

impl Default for EdgeStyle {
    fn default() -> Self {
        EdgeStyle::Solid
    }
}

/// 节点样式覆盖
#[derive(Debug, Clone, PartialEq)]
pub struct NodeStyle {
    /// 填充色
    pub fill: Option<String>,
    /// 边框色
    pub stroke: Option<String>,
    /// 边框宽度
    pub stroke_width: Option<f64>,
    /// 文本色
    pub color: Option<String>,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: None,
            stroke_width: None,
            color: None,
        }
    }
}

/// 节点链接
#[derive(Debug, Clone, PartialEq)]
pub struct NodeLink {
    /// URL
    pub url: String,
    /// target 属性
    pub target: String,
}

/// 图表节点
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramNode {
    /// 节点唯一 ID
    pub id: String,
    /// 节点标签
    pub label: String,
    /// 节点形状
    pub shape: NodeShape,
    /// 节点样式
    pub style: NodeStyle,
    /// 链接
    pub link: Option<NodeLink>,
    /// 所属子图索引
    pub subgraph: Option<usize>,
}

/// 图表边
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramEdge {
    /// 起始节点 ID
    pub from: String,
    /// 目标节点 ID
    pub to: String,
    /// 边标签
    pub label: Option<String>,
    /// 起始端标签
    pub start_label: Option<String>,
    /// 结束端标签
    pub end_label: Option<String>,
    /// 是否有向
    pub directed: bool,
    /// 起始箭头
    pub arrow_start: Option<EdgeArrowhead>,
    /// 结束箭头
    pub arrow_end: Option<EdgeArrowhead>,
    /// 起始装饰
    pub start_decoration: Option<EdgeDecoration>,
    /// 结束装饰
    pub end_decoration: Option<EdgeDecoration>,
    /// 边样式
    pub style: EdgeStyle,
}

/// 子图
#[derive(Debug, Clone, PartialEq)]
pub struct Subgraph {
    /// 子图 ID
    pub id: String,
    /// 子图标签
    pub label: String,
    /// 子图方向覆盖
    pub direction: Option<Direction>,
    /// 包含的节点 ID
    pub nodes: Vec<String>,
    /// 子图样式
    pub style: NodeStyle,
}

/// 序列图激活记录
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceActivation {
    /// 参与者 ID
    pub participant_id: String,
    /// 起始步骤索引
    pub start_step: usize,
    /// 结束步骤索引 (None = 仍在激活)
    pub end_step: Option<usize>,
    /// 嵌套深度
    pub depth: usize,
}

/// 笔记位置
#[derive(Debug, Clone, PartialEq)]
pub enum NotePosition {
    /// 在某个参与者左侧
    LeftOf(String),
    /// 在某个参与者右侧
    RightOf(String),
    /// 横跨多个参与者
    Over(Vec<String>),
}

/// 序列图笔记
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceNote {
    /// 笔记文本
    pub text: String,
    /// 笔记位置
    pub position: NotePosition,
    /// 所在步骤索引
    pub step: usize,
}

/// 序列图背景矩形块
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceRect {
    /// 背景色
    pub color: String,
    /// 起始步骤索引
    pub start_step: usize,
    /// 结束步骤索引
    pub end_step: usize,
}

/// 序列图控制块类型
#[derive(Debug, Clone, PartialEq)]
pub enum ControlBlockKind {
    /// loop
    Loop,
    /// alt
    Alt,
    /// opt
    Opt,
    /// par
    Par,
    /// critical
    Critical,
    /// break
    Break,
}

/// 序列图控制块
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceControlBlock {
    /// 控制块类型
    pub kind: ControlBlockKind,
    /// 标签 (loop 条件、alt 条件等)
    pub label: String,
    /// 起始步骤索引
    pub start_step: usize,
    /// 结束步骤索引
    pub end_step: usize,
    /// 分组标签列表 (alt 的 else, par 的 and 等)
    pub groups: Vec<(String, usize)>,
}

/// 序列图专用元数据
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SequenceMeta {
    /// 有序参与者 ID 列表 (从左到右)
    pub participant_order: Vec<String>,
    /// 参与者别名映射 (alias → id)
    pub aliases: std::collections::HashMap<String, String>,
    /// 参与者类型 (id → true 表示 actor, false 表示 participant)
    pub is_actor: std::collections::HashMap<String, bool>,
    /// 激活记录列表
    pub activations: Vec<SequenceActivation>,
    /// 笔记列表
    pub notes: Vec<SequenceNote>,
    /// 背景矩形块列表
    pub rects: Vec<SequenceRect>,
    /// 控制块列表
    pub control_blocks: Vec<SequenceControlBlock>,
    /// 是否启用自动编号
    pub autonumber: bool,
    /// 消息计数 (用于自动编号)
    pub message_counter: usize,
    /// 总步骤数
    pub total_steps: usize,
}

/// 图表中间表示 (AST)
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramAst {
    /// 图表类型
    pub kind: DiagramKind,
    /// 流图方向
    pub direction: Direction,
    /// 节点映射
    pub nodes: BTreeMap<String, DiagramNode>,
    /// 节点顺序（用于保持声明顺序）
    pub node_order: Vec<String>,
    /// 边列表
    pub edges: Vec<DiagramEdge>,
    /// 子图列表
    pub subgraphs: Vec<Subgraph>,
    /// 图表标题
    pub title: Option<String>,
    /// 序列图专用元数据
    pub sequence_meta: Option<SequenceMeta>,
}

impl DiagramAst {
    /// 创建新的空图表
    pub fn new(kind: DiagramKind) -> Self {
        Self {
            kind,
            direction: Direction::default(),
            nodes: BTreeMap::new(),
            node_order: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            title: None,
            sequence_meta: None,
        }
    }

    /// 添加节点
    pub fn add_node(&mut self, node: DiagramNode) {
        let id = node.id.clone();
        if !self.nodes.contains_key(&id) {
            self.node_order.push(id.clone());
        }
        self.nodes.insert(id, node);
    }

    /// 添加边
    pub fn add_edge(&mut self, edge: DiagramEdge) {
        self.edges.push(edge);
    }

    /// 添加子图
    pub fn add_subgraph(&mut self, subgraph: Subgraph) {
        self.subgraphs.push(subgraph);
    }

    /// 获取节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取边数量
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}
