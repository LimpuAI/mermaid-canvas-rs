//! WIT 类型定义（v2 — 与 world.wit / canvas.wit 中的 record 一一对应）

use mermaid_canvas_component::Margin;

// ─── echodawn:canvas/draw 共享词汇表（v2 — 七通道/多轨道）────

/// WIT `anim-property` — 可动画属性（v2 七通道：transform 组 + color + stroke-width）
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WitAnimProperty {
    /// 不透明度
    Opacity,
    /// x 平移（px）
    TranslateX,
    /// y 平移（px）
    TranslateY,
    /// 缩放（倍率）
    Scale,
    /// 旋转（度）
    Rotate,
    /// 描边宽度（px）
    StrokeWidth,
    /// 颜色（标量因子：0=基色 1=alt-color）
    Color,
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
    /// color 属性专用：因子 0=指令基色 1=alt-color；其余属性忽略
    pub alt_color: Option<String>,
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
    /// OpenType features（"tabular-nums" 等；宿主文本布局消费）
    pub features: Option<Vec<String>>,
}

/// WIT `hover-effect` — 声明式 hover 效果（宿主对 draw-cmd.id == region.index 的指令采样渲染）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitHoverEffect {
    /// "brighten" | "scale" | "lift" | "outline" | "glow" — 未知 kind 宿主 warn + 跳过
    pub kind: String,
    /// kind 语义参数；时长固定走宿主 hover 过渡档（150ms）
    pub params: Vec<f64>,
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

/// WIT `shadow-desc` — 外阴影描述（CSS box-shadow 外阴影子集，宿主 SDF
/// 高斯软阴影 pass 渲染；忌以实体描边/填充模拟——硬边阴影在入场缩放下呈厚重黑框）
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitShadowDesc {
    /// x 偏移（px；正值向右）
    pub offset_x: f64,
    /// y 偏移（px；正值向下）
    pub offset_y: f64,
    /// 模糊半径（px；0 = 硬边）
    pub blur: f64,
    /// 扩散（px；正值扩张阴影轮廓，负值收缩）
    pub spread: f64,
    /// 阴影基色（hex/rgb/rgba 字符串；透明度以 alpha 字段为准）
    pub color: String,
    /// 阴影不透明度（0-1；最终 alpha = color 自身 alpha × alpha）
    pub alpha: f64,
    /// 阴影形状宽（px；绕宿主形状中心）。path 指令缺省 (0,0) = 包围盒；
    /// 菱形等对角形状显式声明内接正方形 + rotation 45 即得真实轮廓阴影
    pub width: f64,
    /// 阴影形状高（px；语义同 width）
    pub height: f64,
    /// 阴影形状旋转角（度；顺时针，绕宿主形状中心）
    pub rotation: f64,
}

/// WIT `draw-cmd` — 无损绘制指令（展平结构，不支持递归类型；v2 多轨道/装饰通道）
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
    /// per-corner 圆角（TL, TR, BR, BL）；与 corner-radius 同存时优先
    pub corner_radii: Option<(f64, f64, f64, f64)>,
    /// 虚线节律（线段/间隙交替，px）；缺省或空 = 实线
    pub dash: Option<Vec<f64>>,
    /// 线端帽："butt" | "round" | "square"；缺省 butt
    pub line_cap: Option<String>,
    /// 外阴影（rect/circle/path 支持；path 以包围盒近似；None = 无阴影）。
    /// 阴影统一绘制于全部形状之前的专用 shadow pass（z 序 = 所有形状之下）
    #[serde(default)]
    pub shadow: Option<WitShadowDesc>,
    /// 文本内容
    pub text_content: Option<String>,
    /// 字体描述
    pub font: Option<WitFontDesc>,
    /// 分组深度
    pub group_depth: u32,
    /// 命令身份：所属 hit-region index（一对多；宿主 per-item 效果关联键）
    pub id: Option<u32>,
    /// Tier 2 多轨道动画（v2 起替代 v1 的单 anim 字段；SignalFlow preset 呼吸动效附着）
    pub anims: Vec<WitAnimDesc>,
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
    /// hover 提亮语义色（guest set-state 提亮的基准色；缺省用宿主近似）
    pub hover_color: Option<String>,
    /// 形轴 preset："classic" | "signal-flow" | "blueprint" | "editorial"（缺省 classic；
    /// 只管形态学参数，色彩恒由 node-colors 等 token 槽位提供）
    pub style_preset: Option<String>,
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
    /// 声明式 hover 效果（宿主对 draw-cmd.id == index 的指令采样渲染，零 wasm 调用）
    pub hover: Option<WitHoverEffect>,
}
