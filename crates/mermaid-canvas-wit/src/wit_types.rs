//! WIT 类型定义（v2 — 与 world.wit / canvas.wit 中的 record 一一对应）

use mermaid_canvas_component::Margin;

// ─── echodawn:canvas/draw 共享词汇表 ────────────────────────

/// WIT `anim-property` — 可动画属性
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WitAnimProperty {
    /// 不透明度
    Opacity,
}

/// WIT `loop-mode` — 循环模式
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WitLoopMode {
    /// 单次
    Once,
    /// 循环
    Loop,
    /// 往返
    PingPong,
}

/// WIT `keyframe` — 关键帧 (t, value)，t ∈ [0,1]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitKeyframe {
    /// 相位 t
    pub t: f32,
    /// 值
    pub value: f32,
    /// 缓动名称
    pub easing: String,
}

/// WIT `anim-desc` — Tier 2 参数动画描述（宿主本地插值）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitAnimDesc {
    /// 可动画属性
    pub property: WitAnimProperty,
    /// 关键帧列表
    pub keyframes: Vec<WitKeyframe>,
    /// 时长（毫秒）
    pub duration_ms: u32,
    /// 延迟（毫秒）
    pub delay_ms: u32,
    /// 循环模式
    pub loop_mode: WitLoopMode,
}

/// WIT `font-desc` — 字体描述
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitFontDesc {
    /// 字体族
    pub family: Option<String>,
    /// 字重（CSS 数值 100-900；Bold ≙ 700）
    pub weight: Option<u16>,
    /// 斜体
    pub italic: bool,
}

/// WIT `gradient-stop` — 渐变停止点
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitGradientStop {
    /// 偏移位置
    pub pos: f64,
    /// 颜色
    pub color: String,
}

/// WIT `linear-gradient` — 线性渐变
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitLinearGradient {
    /// 起点 x
    pub x0: f64,
    /// 起点 y
    pub y0: f64,
    /// 终点 x
    pub x1: f64,
    /// 终点 y
    pub y1: f64,
    /// 停止点列表
    pub stops: Vec<WitGradientStop>,
}

/// WIT `paint` — 填充/描边 paint
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WitPaint {
    /// 纯色（CSS 颜色字符串）
    Solid(String),
    /// 线性渐变
    Gradient(WitLinearGradient),
}

/// WIT `draw-cmd` — 无损绘制指令（展平结构，不支持递归类型）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitDrawCmd {
    /// 指令类型："rect" | "path" | "circle" | "text"
    pub cmd_type: String,
    /// 参数列表（text: [x, y, font-size, anchor, baseline]；path: 段前缀拼接）
    pub params: Vec<f64>,
    /// 填充 paint
    pub fill: Option<WitPaint>,
    /// 描边 paint
    pub stroke: Option<WitPaint>,
    /// 描边宽度
    pub stroke_width: Option<f64>,
    /// 圆角半径
    pub corner_radius: Option<f64>,
    /// 文本内容
    pub text_content: Option<String>,
    /// 字体描述
    pub font: Option<WitFontDesc>,
    /// 分组深度
    pub group_depth: u32,
    /// Tier 2 动画附着（D9：协议带通道，native 不附着 — 恒 None）
    pub anim: Option<WitAnimDesc>,
}

/// WIT `layer` — 渲染层
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitLayer {
    /// 层类型
    pub kind: String,
    /// 是否脏（与上次返回结果相比是否变化）
    pub dirty: bool,
    /// z-index
    pub z_index: u32,
    /// 绘图指令
    pub commands: Vec<WitDrawCmd>,
}

// ─── mermaid:viz/diagram-renderer 域内类型 ──────────────────

/// WIT `margin` — 边距
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitMargin {
    /// 上
    pub top: f64,
    /// 右
    pub right: f64,
    /// 下
    pub bottom: f64,
    /// 左
    pub left: f64,
}

impl From<Margin> for WitMargin {
    fn from(m: Margin) -> Self {
        Self { top: m.top, right: m.right, bottom: m.bottom, left: m.left }
    }
}

impl From<WitMargin> for Margin {
    fn from(m: WitMargin) -> Self {
        Self { top: m.top, right: m.right, bottom: m.bottom, left: m.left }
    }
}

/// WIT `diagram-theme` — 主题注入记录（6 语义色槽 = shape_slot 体系）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitDiagramTheme {
    /// 背景色
    pub background: String,
    /// 前景/文本色
    pub foreground: String,
    /// 边线条色
    pub edge_color: String,
    /// 边标签背景色
    pub edge_label_background: String,
    /// 6 项节点色槽（primary/secondary/accent/info/data/special）
    pub node_colors: Vec<String>,
    /// 节点边框色
    pub node_stroke: String,
    /// 标题文本色
    pub title_color: String,
    /// 字体族
    pub font_family: String,
    /// 基础字体大小
    pub base_font_size: f64,
    /// 标题字体大小
    pub title_font_size: f64,
    /// 边距
    pub margin: WitMargin,
}

/// WIT `interaction-state` — 交互状态回注
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitInteractionState {
    /// hover 的命中区索引
    pub hovered: Option<u32>,
    /// 选中的命中区索引集合
    pub selected: Vec<u32>,
}

/// WIT `animation-config` — 入场编排配置
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitAnimationConfig {
    /// 入场总时长（毫秒，缺省 500）
    pub enter_duration_ms: Option<u32>,
    /// 每项级联相位偏移（毫秒，缺省 24，级联总量 cap 400）
    pub stagger_ms: Option<f64>,
    /// 缓动名称（缺省 "cubic-out"）
    pub easing: Option<String>,
    /// 禁用动画（ReducucedMotion / 静态场景 — 任意 t 渲染稳态）
    pub disable: bool,
}

/// WIT `diagram-options` — 构造选项
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitDiagramOptions {
    /// 宽度约束（fit-to-width，仅收缩）
    pub width: Option<f64>,
    /// 内置主题名（default/dark/forest/nordic/cappuccino）
    pub theme: Option<String>,
    /// 入场编排
    pub animation: Option<WitAnimationConfig>,
}

/// WIT `render-result` — 渲染结果（内容自适应尺寸后验）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitRenderResult {
    /// 渲染层列表
    pub layers: Vec<WitLayer>,
    /// 画布宽度（像素）
    pub width: f64,
    /// 画布高度（像素）
    pub height: f64,
}

/// WIT `hit-region` — 命中区（payload = 节点 id）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitHitRegion {
    /// 区域索引（与 interaction-state 的索引一致）
    pub index: u32,
    /// 节点 ID（宿主 tooltip 用）
    pub node_id: Option<String>,
    /// 包围盒 x
    pub bounds_x: f64,
    /// 包围盒 y
    pub bounds_y: f64,
    /// 包围盒宽度
    pub bounds_w: f64,
    /// 包围盒高度
    pub bounds_h: f64,
}
