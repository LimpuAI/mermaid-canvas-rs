//! WIT 类型定义

/// WIT 图表类型
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitDiagramNode {
    /// 节点 ID
    pub id: String,
    /// 节点标签
    pub label: String,
    /// 节点形状
    pub shape: String,
    /// 节点填充色
    pub fill: Option<String>,
    /// 节点边框色
    pub stroke: Option<String>,
}

/// WIT 边定义
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitDiagramEdge {
    /// 起始节点 ID
    pub from: String,
    /// 目标节点 ID
    pub to: String,
    /// 边标签
    pub label: Option<String>,
    /// 是否有箭头
    pub directed: bool,
}

/// WIT 绘图指令定义（展平结构，不支持递归类型）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitDrawCmd {
    /// 指令类型
    pub cmd_type: String,
    /// 参数列表
    pub params: Vec<f64>,
    /// 填充色
    pub fill: Option<String>,
    /// 描边色
    pub stroke: Option<String>,
    /// 描边宽度
    pub stroke_width: Option<f64>,
    /// 文本内容
    pub text_content: Option<String>,
    /// 分组深度
    pub group_depth: u32,
}

/// WIT 命中区域定义
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitHitRegion {
    /// 索引
    pub index: u32,
    /// 包围盒 x
    pub bounds_x: f64,
    /// 包围盒 y
    pub bounds_y: f64,
    /// 包围盒宽度
    pub bounds_w: f64,
    /// 包围盒高度
    pub bounds_h: f64,
}

/// WIT 渲染层定义
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitLayer {
    /// 层类型
    pub kind: String,
    /// 是否脏
    pub dirty: bool,
    /// z-index
    pub z_index: u32,
    /// 绘图指令
    pub commands: Vec<WitDrawCmd>,
    /// 命中区域
    pub hit_regions: Vec<WitHitRegion>,
}

/// WIT 渲染结果定义
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitRenderResult {
    /// 渲染层列表
    pub layers: Vec<WitLayer>,
    /// 画布宽度（像素）
    pub width: f64,
    /// 画布高度（像素）
    pub height: f64,
}
