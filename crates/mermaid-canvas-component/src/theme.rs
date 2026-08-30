//! 主题系统
//!
//! 按 **节点形状 (NodeShape)** 分配颜色 — 同类型同色，颜色传递语义。
//! 每个主题定义 7 个语义色槽，形状按语义分组映射：
//!
//! | 色槽 | 形状 | 语义 |
//! |------|------|------|
//! | primary | Rectangle, RoundRect, Stadium | 普通流程节点 |
//! | secondary | Subroutine | 子流程 |
//! | accent | Diamond | 判断/分支 |
//! | info | Circle, DoubleCircle | 起止/连接 |
//! | data | Cylinder | 数据存储 |
//! | special | Hexagon, Parallelogram, Trapezoid | 特殊处理 |
//! | external | Asymmetric | 外部实体/手工操作 |

use mermaid_canvas_core::style::{FillStyle, Gradient, GradientKind, GradientStop};
use mermaid_canvas_core::NodeShape;

use crate::preset::StylePreset;

/// 边距定义
#[derive(Debug, Clone, PartialEq)]
pub struct Margin {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Margin {
    pub fn all(value: f64) -> Self {
        Self { top: value, right: value, bottom: value, left: value }
    }

    pub fn none() -> Self {
        Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 }
    }
}

impl Default for Margin {
    fn default() -> Self {
        Self::all(20.0)
    }
}

// ── 颜色工具（hex 解析 / 混合 / 提亮 — T17 tint 语义色）──────────

/// "#rgb" / "#rrggbb" / "rgb(r,g,b)" / "rgba(r,g,b,a)" → (r, g, b)；
/// 其余返回 None（rgba 的 alpha 在解析时丢弃 — 主题槽位色不携带半透明语义,
/// 半透明层走 `with_color_alpha` 重组）。防御性兼容:WIT 协议允许任意 CSS
/// 色串,宿主注入格式不受 guest 色彩派生管线约束(R9 — rgba 注入曾让
/// tint/描边/辉光派生整体静默失效)。
pub fn parse_hex_color(color: &str) -> Option<(u8, u8, u8)> {
    if let Some(rest) = color.strip_prefix("rgb") {
        let inner = rest.trim_start_matches(['a', '(']).trim_end_matches(')');
        let mut chans = inner.split(',');
        let r = chans.next()?.trim().parse::<f32>().ok()?;
        let g = chans.next()?.trim().parse::<f32>().ok()?;
        let b = chans.next()?.trim().parse::<f32>().ok()?;
        let ch = |v: f32| -> Option<u8> {
            if (0.0..=255.0).contains(&v) { Some(v.round() as u8) } else { None }
        };
        return Some((ch(r)?, ch(g)?, ch(b)?));
    }
    let hex = color.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// 两色线性混合（amount = b 占比）；任一非 hex 返回 None
pub fn mix_colors(a: &str, b: &str, amount: f64) -> Option<String> {
    let (r1, g1, b1) = parse_hex_color(a)?;
    let (r2, g2, b2) = parse_hex_color(b)?;
    let mix = |x: u8, y: u8| -> u8 {
        (x as f64 + (y as f64 - x as f64) * amount).round() as u8
    };
    Some(to_hex(mix(r1, r2), mix(g1, g2), mix(b1, b2)))
}

/// 向白色提亮；非 hex 原样返回
pub fn lighten_color(color: &str, amount: f64) -> String {
    mix_colors(color, "#ffffff", amount).unwrap_or_else(|| color.to_string())
}

/// 向黑色压暗；非 hex 原样返回
pub fn darken_color(color: &str, amount: f64) -> String {
    mix_colors(color, "#000000", amount).unwrap_or_else(|| color.to_string())
}

/// 相对亮度（0=黑 1=白）；非 hex 返回 0.5（中性）
pub fn color_luma(color: &str) -> f64 {
    match parse_hex_color(color) {
        Some((r, g, b)) => (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0,
        None => 0.5,
    }
}

/// WCAG 相对亮度（sRGB gamma 展开）
fn wcag_relative_luminance(color: &str) -> f64 {
    let (r, g, b) = match parse_hex_color(color) {
        Some(c) => c,
        None => return 0.5,
    };
    let chan = |v: u8| -> f64 {
        let c = v as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)
}

/// WCAG 对比度（1..21；同色 = 1；非 hex 中性 1）
pub fn wcag_contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (wcag_relative_luminance(a), wcag_relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// 带 alpha 的 rgba 字符串；非 hex 原样返回（辉光层/网格线用）
pub fn with_color_alpha(color: &str, alpha: f64) -> String {
    match parse_hex_color(color) {
        Some((r, g, b)) => format!("rgba({},{},{},{:.3})", r, g, b, alpha),
        None => color.to_string(),
    }
}

/// 乘法叠加 alpha（R10）：`rgba(...)` 取原 alpha 相乘，其余（含 hex，视为 1.0）
/// 等价于 with_color_alpha。烘焙动画 alpha（入场淡入/dim）必须叠加而非替换——
/// 替换会把 bevel(0.28) 之类低 alpha 装饰在过渡中顶成一档不透明，动画结束
/// 瞬间跳回基值（相位割裂感的来源之一）。
pub fn mul_color_alpha(color: &str, factor: f64) -> String {
    let existing = match parse_hex_color(color) {
        Some((r, g, b)) => (r, g, b),
        None => return color.to_string(),
    };
    let base_alpha = color
        .to_ascii_lowercase()
        .strip_prefix("rgba(")
        .and_then(|rest| rest.split(',').nth(3))
        .and_then(|a| a.trim().trim_end_matches(')').trim().parse::<f64>().ok())
        .unwrap_or(1.0);
    with_color_alpha(color, (base_alpha * factor).clamp(0.0, 1.0))
}

/// 形状→色槽映射
///
/// 将 NodeShape 归入 7 个语义色槽，主题只需定义 7 个颜色。
pub fn shape_slot(shape: &NodeShape) -> usize {
    match shape {
        // 0: primary — 普通流程节点
        NodeShape::Rectangle | NodeShape::RoundRect | NodeShape::Stadium => 0,
        // 1: secondary — 子流程
        NodeShape::Subroutine => 1,
        // 2: accent — 判断/分支
        NodeShape::Diamond => 2,
        // 3: info — 起止/连接
        NodeShape::Circle | NodeShape::DoubleCircle => 3,
        // 4: data — 数据存储
        NodeShape::Cylinder => 4,
        // 5: special — 特殊处理
        NodeShape::Hexagon | NodeShape::Parallelogram | NodeShape::Trapezoid => 5,
        // 6: external — 外部实体/手工操作
        NodeShape::Asymmetric => 6,
    }
}

/// 节点 tint 填充的混合强度（底色混角色色）— R8 空心化:参考图量化
/// light 8-10%/dark 12-15%,区域区分度主要由 vivid 描边色相承担
const NODE_TINT_ALPHA_DARK: f64 = 0.14;
const NODE_TINT_ALPHA_LIGHT: f64 = 0.09;
/// tint 对比度收缩下限（再低角色色不可感知）
const NODE_TINT_MIN_ALPHA: f64 = 0.06;
/// 子图背景相对底色的前景混入强度
const SUBGRAPH_BG_ALPHA: f64 = 0.04;

/// 主题 trait — 定义图表的视觉风格
pub trait Theme {
    /// 主题名称
    fn name(&self) -> &str;

    /// 背景色
    fn background_color(&self) -> &str;

    /// 字体族
    fn font_family(&self) -> &str { "sans-serif" }

    /// 基础字体大小
    fn font_size(&self) -> f64 { 14.0 }

    /// 标题字体大小（T15/T19 — title 层字号通道）
    fn title_font_size(&self) -> f64 { 18.0 }

    /// 边标签字体大小（T19 — 边标签字号通道，0.85x 层级）
    fn edge_label_font_size(&self) -> f64 { self.font_size() * 0.85 }

    /// 节点填充色（按形状类型 — 槽位原色）
    fn node_fill_color(&self, shape: &NodeShape) -> &str;

    /// 节点填充样式（默认槽位原色；RecordTheme 覆写为 tint 半透明填充）
    fn node_fill(&self, shape: &NodeShape) -> FillStyle {
        FillStyle::Color(self.node_fill_color(shape).to_string())
    }

    /// 节点边框色
    fn node_stroke(&self) -> &str;

    /// 节点边框色（按形状 — 语义槽位对比色；缺省 = 统一 node_stroke）
    fn node_stroke_for(&self, _shape: &NodeShape) -> String {
        self.node_stroke().to_string()
    }

    /// 节点文本色
    fn node_text_color(&self) -> &str;

    /// 边线条色
    fn edge_color(&self) -> &str;

    /// 边标签背景色
    fn edge_label_background(&self) -> &str;

    /// 子图背景色
    fn subgraph_background(&self) -> &str;

    /// 子图边框色
    fn subgraph_border(&self) -> &str;

    /// 标题文本色
    fn title_color(&self) -> &str;

    /// 形轴 preset（T20 — 形态学参数；缺省 Classic）
    fn style_preset(&self) -> StylePreset {
        StylePreset::Classic
    }

    /// 边距
    fn margin(&self) -> Margin {
        Margin::default()
    }
}

// ═══════════════════════════════════════════════════════════════
// 内置主题 — 每个主题 7 语义色槽:
// primary, secondary, accent, info, data, special, external
// ═══════════════════════════════════════════════════════════════

// ─── Default（经典浅色）───────────────────────────────────────

/// 默认浅色主题
pub struct DefaultTheme;

impl DefaultTheme {
    const PALETTE: [&'static str; 7] = [
        "#dae8fc", // primary: 蓝
        "#e1d5e7", // secondary: 紫
        "#fff2cc", // accent: 黄
        "#d5e8d4", // info: 绿
        "#f8cecc", // data: 红
        "#fff2cc", // special: 黄
        "#d8d8e8", // external: 冷灰
    ];
}

impl Theme for DefaultTheme {
    fn name(&self) -> &str { "Default" }
    fn background_color(&self) -> &str { "#ffffff" }
    fn node_fill_color(&self, shape: &NodeShape) -> &str {
        Self::PALETTE[shape_slot(shape)]
    }
    fn node_stroke(&self) -> &str { "#6c8ebf" }
    fn node_text_color(&self) -> &str { "#333333" }
    fn edge_color(&self) -> &str { "#666666" }
    fn edge_label_background(&self) -> &str { "#ffffff" }
    fn subgraph_background(&self) -> &str { "#f5f5f5" }
    fn subgraph_border(&self) -> &str { "#cccccc" }
    fn title_color(&self) -> &str { "#333333" }
}

// ─── Dark（深色冷调）─────────────────────────────────────────

/// 深色主题
pub struct DarkTheme;

impl DarkTheme {
    /// vivid 语义槽位（R7 现代化 — tokyyo-night 系:深底全饱和角色色,
    /// tint/描边明度自适应由 RecordTheme 派生）
    const PALETTE: [&'static str; 7] = [
        "#7aa2f7", // primary: 蓝
        "#bb9af7", // secondary: 紫
        "#e0af68", // accent: 琥珀(判断)
        "#7dcfff", // info: 青(起止)
        "#9ece6a", // data: 绿(存储)
        "#f7768e", // special: 粉(特殊)
        "#c0caf5", // external: 淡蓝灰
    ];
}

impl Theme for DarkTheme {
    fn name(&self) -> &str { "Dark" }
    fn background_color(&self) -> &str { "#1a1b26" }
    fn node_fill_color(&self, shape: &NodeShape) -> &str {
        Self::PALETTE[shape_slot(shape)]
    }
    fn node_stroke(&self) -> &str { "#89b4fa" }
    fn node_text_color(&self) -> &str { "#c0caf5" }
    fn edge_color(&self) -> &str { "#7c87b8" }
    fn edge_label_background(&self) -> &str { "#1a1b26" }
    fn subgraph_background(&self) -> &str { "#16161e" }
    fn subgraph_border(&self) -> &str { "#3b4261" }
    fn title_color(&self) -> &str { "#c0caf5" }
}

// ─── Forest（森林绿）─────────────────────────────────────────

/// 森林主题 — 深绿底 + 多层次绿色
pub struct ForestTheme;

impl ForestTheme {
    const PALETTE: [&'static str; 7] = [
        "#2d5a27", // primary: 深绿
        "#3a6b34", // secondary: 中绿
        "#4a7c3f", // accent: 亮绿
        "#1e4d2b", // info: 暗绿
        "#5a3a27", // data: 棕绿
        "#3a6b34", // special: 中绿
        "#3d4a2d", // external: 橄榄灰
    ];
}

impl Theme for ForestTheme {
    fn name(&self) -> &str { "Forest" }
    fn background_color(&self) -> &str { "#1b2a1b" }
    fn node_fill_color(&self, shape: &NodeShape) -> &str {
        Self::PALETTE[shape_slot(shape)]
    }
    fn node_stroke(&self) -> &str { "#8bc34a" }
    fn node_text_color(&self) -> &str { "#e8f5e9" }
    fn edge_color(&self) -> &str { "#689f38" }
    fn edge_label_background(&self) -> &str { "#1b2a1b" }
    fn subgraph_background(&self) -> &str { "#0d1f0d" }
    fn subgraph_border(&self) -> &str { "#558b2f" }
    fn title_color(&self) -> &str { "#c5e1a5" }
}

// ─── Nordic（北欧极简）────────────────────────────────────────

/// 北欧主题 — 冷灰蓝 + 淡粉点缀
pub struct NordicTheme;

impl NordicTheme {
    const PALETTE: [&'static str; 7] = [
        "#dfe6ed", // primary: 冷蓝灰
        "#e8edf2", // secondary: 浅蓝灰
        "#f0e6ec", // accent: 淡粉
        "#e2e8f0", // info: 蓝灰
        "#e0ddd8", // data: 暖灰
        "#ede9e6", // special: 米灰
        "#e4e4e0", // external: 石灰
    ];
}

impl Theme for NordicTheme {
    fn name(&self) -> &str { "Nordic" }
    fn background_color(&self) -> &str { "#f8f9fb" }
    fn node_fill_color(&self, shape: &NodeShape) -> &str {
        Self::PALETTE[shape_slot(shape)]
    }
    fn node_stroke(&self) -> &str { "#8b9eb0" }
    fn node_text_color(&self) -> &str { "#3d4f5f" }
    fn edge_color(&self) -> &str { "#8b9eb0" }
    fn edge_label_background(&self) -> &str { "#f8f9fb" }
    fn subgraph_background(&self) -> &str { "#eef1f5" }
    fn subgraph_border(&self) -> &str { "#8b9eb0" }
    fn title_color(&self) -> &str { "#3d4f5f" }
}

// ─── Cappuccino（卡布奇诺）────────────────────────────────────

/// 卡布奇诺主题 — 暖棕奶咖色系
pub struct CappuccinoTheme;

impl CappuccinoTheme {
    const PALETTE: [&'static str; 7] = [
        "#e8d5c4", // primary: 奶咖
        "#dcc8b4", // secondary: 浅棕
        "#f0e0d0", // accent: 奶白
        "#d4b896", // info: 焦糖
        "#c9a882", // data: 深焦糖
        "#e0cdc0", // special: 米棕
        "#d9cbbd", // external: 燕麦
    ];
}

impl Theme for CappuccinoTheme {
    fn name(&self) -> &str { "Cappuccino" }
    fn background_color(&self) -> &str { "#faf6f1" }
    fn node_fill_color(&self, shape: &NodeShape) -> &str {
        Self::PALETTE[shape_slot(shape)]
    }
    fn node_stroke(&self) -> &str { "#8b6f4e" }
    fn node_text_color(&self) -> &str { "#3e2c1c" }
    fn edge_color(&self) -> &str { "#a08060" }
    fn edge_label_background(&self) -> &str { "#faf6f1" }
    fn subgraph_background(&self) -> &str { "#f0e8de" }
    fn subgraph_border(&self) -> &str { "#c9a882" }
    fn title_color(&self) -> &str { "#5d3a1a" }
}

// ═══════════════════════════════════════════════════════════════
// 记录主题 — WIT diagram-theme record 的载体（v2 会话协议）
// ═══════════════════════════════════════════════════════════════

/// 主题记录 — 与 WIT `diagram-theme` record 逐字段对应
///
/// 宿主（DesignTokens 模式感知派生）或内置主题名注入；
/// `node_colors` 为语义色槽（primary/secondary/accent/info/data/special/external），
/// 经 `shape_slot` 映射消费。
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeRecord {
    /// 背景色
    pub background: String,
    /// 前景/文本色
    pub foreground: String,
    /// 边线条色
    pub edge_color: String,
    /// 边标签背景色
    pub edge_label_background: String,
    /// 语义节点色槽
    pub node_colors: Vec<String>,
    /// 节点边框色
    pub node_stroke: String,
    /// 标题文本色
    pub title_color: String,
    /// hover 提亮语义色（set-state 提亮的基准色；缺省用 foreground 近似）
    pub hover_color: Option<String>,
    /// 形轴 preset："classic" | "signal-flow" | "blueprint" | "editorial"（缺省 classic）
    pub style_preset: Option<String>,
    /// 字体族
    pub font_family: String,
    /// 基础字体大小
    pub base_font_size: f64,
    /// 标题字体大小
    pub title_font_size: f64,
    /// 边距
    pub margin: Margin,
}

impl Default for ThemeRecord {
    fn default() -> Self {
        builtin_theme_record("default").expect("default theme record must exist")
    }
}

impl ThemeRecord {
    /// 返回字体随缩放因子调整后的记录副本（fit-to-width 缩放时保持视觉一致性）
    pub fn with_scaled_fonts(&self, scale: f64) -> Self {
        let mut r = self.clone();
        r.base_font_size *= scale;
        r.title_font_size *= scale;
        r
    }
}

/// 记录主题 — 由 `ThemeRecord` 驱动的 `Theme` 实现
///
/// - 填充 = tint 半透明语义色（深底 14% / 浅底 9% — luma 感知：R8 空心化
///   参考图量化,区域区分度由 vivid 描边承担）
/// - 描边 = 角色色明度自适应（深底微提亮 / 浅底微压暗）— vivid 描边
///   与 tint 填充形成同色相对比（archify 层次；R7 修复宿主预 tint +
///   guest 二次 tint 的双重变暗：宿主注入全饱和角色色，tint 归 guest）
/// - `node_colors` 少于 7 项时，缺失槽位回落到第 0 槽（primary），
///   保证宿主注入不完整时仍可渲染
pub struct RecordTheme {
    record: ThemeRecord,
    /// 预派生子图背景（trait 借用返回 — 构造期算好）
    subgraph_bg: String,
    /// 背景深浅（构造期算好 — tint 强度/描边明度自适应）
    dark_bg: bool,
}

impl RecordTheme {
    /// 从记录创建
    pub fn new(record: ThemeRecord) -> Self {
        let subgraph_bg = mix_colors(&record.background, &record.foreground, SUBGRAPH_BG_ALPHA)
            .unwrap_or_else(|| record.background.clone());
        let dark_bg = color_luma(&record.background) < 0.5;
        Self { record, subgraph_bg, dark_bg }
    }

    /// 借用底层记录
    pub fn record(&self) -> &ThemeRecord {
        &self.record
    }

    fn slot_color(&self, shape: &NodeShape) -> &str {
        let slot = shape_slot(shape);
        self.record
            .node_colors
            .get(slot)
            .or_else(|| self.record.node_colors.first())
            .map(String::as_str)
            .unwrap_or("#cccccc")
    }
}

impl Theme for RecordTheme {
    fn name(&self) -> &str { "Record" }
    fn background_color(&self) -> &str { &self.record.background }
    fn font_family(&self) -> &str { &self.record.font_family }
    fn font_size(&self) -> f64 { self.record.base_font_size + self.style_preset().font_boost() }
    fn title_font_size(&self) -> f64 { self.record.title_font_size + self.style_preset().font_boost() }
    fn node_fill_color(&self, shape: &NodeShape) -> &str { self.slot_color(shape) }
    /// tint 半透明填充：底色混角色色（深底 14% / 浅底 9% — 底色承载、角色色点染）。
    /// 对比度保障（WCAG AA 文字档）：tint 提升填充着色强度会压低文字对比度,
    /// 超限时按 0.02 步进收缩至下限 0.06（宿主注入全饱和锚色起,R7）。
    fn node_fill(&self, shape: &NodeShape) -> FillStyle {
        let slot = self.slot_color(shape);
        let start = if self.dark_bg { NODE_TINT_ALPHA_DARK } else { NODE_TINT_ALPHA_LIGHT };
        let mut alpha = start;
        while alpha > NODE_TINT_MIN_ALPHA {
            if let Some(tint) = mix_colors(&self.record.background, slot, alpha) {
                if wcag_contrast(&self.record.foreground, &tint) >= 4.5 {
                    break;
                }
            } else {
                break;
            }
            alpha -= 0.02;
        }
        match mix_colors(&self.record.background, slot, alpha) {
            Some(tint) => FillStyle::Color(tint),
            None => FillStyle::Color(slot.to_string()),
        }
    }
    fn node_stroke(&self) -> &str { &self.record.node_stroke }
    /// 节点描边 = 角色色明度自适应（深底提亮 12% / 浅底压暗 8%）
    fn node_stroke_for(&self, shape: &NodeShape) -> String {
        let slot = self.slot_color(shape);
        if self.dark_bg {
            lighten_color(slot, 0.12)
        } else {
            darken_color(slot, 0.08)
        }
    }
    fn node_text_color(&self) -> &str { &self.record.foreground }
    fn edge_color(&self) -> &str { &self.record.edge_color }
    fn edge_label_background(&self) -> &str { &self.record.edge_label_background }
    /// 子图背景 = 底色向前景微混（4% — 可感知的分组底）
    fn subgraph_background(&self) -> &str { &self.subgraph_bg }
    /// 子图边框由节点边框色承担
    fn subgraph_border(&self) -> &str { &self.record.node_stroke }
    fn title_color(&self) -> &str { &self.record.title_color }
    fn style_preset(&self) -> StylePreset {
        StylePreset::parse(&self.record.style_preset)
    }
    fn margin(&self) -> Margin { self.record.margin.clone() }
}

/// 垂直线性渐变填充（T21 — 基色 → 基色提亮 8%）
///
/// 坐标取节点包围盒（渲染器侧调用 — 主题层不知几何）；
/// 非 Color 基色原样返回。
/// 垂直渐变填充 — 顶部提亮 `range` → 底部微暗（"光自上来"的卡片光照）
pub fn vertical_gradient_fill(
    base: &FillStyle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    range: f64,
) -> FillStyle {
    match base {
        FillStyle::Color(c) => FillStyle::Gradient(Gradient {
            kind: GradientKind::Linear { x0: x, y0: y, x1: x, y1: y + height },
            stops: vec![
                GradientStop::new(0.0, lighten_color(c, range)),
                GradientStop::new(1.0, darken_color(c, range * 0.5)),
            ],
        }),
        other => other.clone(),
    }
}

/// 内置主题名 → 主题记录
///
/// 支持的名称：`"default"` / `"dark"` / `"forest"` / `"nordic"` / `"cappuccino"`；
/// 未知名称返回 `None`（调用方决定回落）。
pub fn builtin_theme_record(name: &str) -> Option<ThemeRecord> {
    let (palette, background, node_stroke, node_text, edge_color, edge_label_bg, title_color) = match name {
        "default" => (
            DefaultTheme::PALETTE, DefaultTheme.background_color(), DefaultTheme.node_stroke(),
            DefaultTheme.node_text_color(), DefaultTheme.edge_color(),
            DefaultTheme.edge_label_background(), DefaultTheme.title_color(),
        ),
        "dark" => (
            DarkTheme::PALETTE, DarkTheme.background_color(), DarkTheme.node_stroke(),
            DarkTheme.node_text_color(), DarkTheme.edge_color(),
            DarkTheme.edge_label_background(), DarkTheme.title_color(),
        ),
        "forest" => (
            ForestTheme::PALETTE, ForestTheme.background_color(), ForestTheme.node_stroke(),
            ForestTheme.node_text_color(), ForestTheme.edge_color(),
            ForestTheme.edge_label_background(), ForestTheme.title_color(),
        ),
        "nordic" => (
            NordicTheme::PALETTE, NordicTheme.background_color(), NordicTheme.node_stroke(),
            NordicTheme.node_text_color(), NordicTheme.edge_color(),
            NordicTheme.edge_label_background(), NordicTheme.title_color(),
        ),
        "cappuccino" => (
            CappuccinoTheme::PALETTE, CappuccinoTheme.background_color(), CappuccinoTheme.node_stroke(),
            CappuccinoTheme.node_text_color(), CappuccinoTheme.edge_color(),
            CappuccinoTheme.edge_label_background(), CappuccinoTheme.title_color(),
        ),
        _ => return None,
    };
    Some(ThemeRecord {
        background: background.to_string(),
        foreground: node_text.to_string(),
        edge_color: edge_color.to_string(),
        edge_label_background: edge_label_bg.to_string(),
        node_colors: palette.iter().map(|s| s.to_string()).collect(),
        node_stroke: node_stroke.to_string(),
        title_color: title_color.to_string(),
        hover_color: None,
        style_preset: None,
        font_family: "sans-serif".to_string(),
        base_font_size: 14.0,
        title_font_size: 18.0,
        margin: Margin::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_slot_mapping() {
        // 同语义组 → 同色槽
        assert_eq!(shape_slot(&NodeShape::Rectangle), shape_slot(&NodeShape::RoundRect));
        assert_eq!(shape_slot(&NodeShape::Circle), shape_slot(&NodeShape::DoubleCircle));
        // 不同语义组 → 不同色槽
        assert_ne!(shape_slot(&NodeShape::Rectangle), shape_slot(&NodeShape::Diamond));
        assert_ne!(shape_slot(&NodeShape::Diamond), shape_slot(&NodeShape::Circle));
        assert_ne!(shape_slot(&NodeShape::Cylinder), shape_slot(&NodeShape::Rectangle));
    }

    #[test]
    fn test_all_themes_shape_based_coloring() {
        let themes: Vec<Box<dyn Theme>> = vec![
            Box::new(DefaultTheme),
            Box::new(DarkTheme),
            Box::new(ForestTheme),
            Box::new(NordicTheme),
            Box::new(CappuccinoTheme),
        ];

        for theme in &themes {
            // 菱形和矩形颜色应不同
            assert_ne!(
                theme.node_fill_color(&NodeShape::Rectangle),
                theme.node_fill_color(&NodeShape::Diamond),
                "{}: Rectangle and Diamond should have different colors",
                theme.name(),
            );
            // 圆形和矩形颜色应不同
            assert_ne!(
                theme.node_fill_color(&NodeShape::Rectangle),
                theme.node_fill_color(&NodeShape::Circle),
                "{}: Rectangle and Circle should have different colors",
                theme.name(),
            );
            // 同语义组颜色应相同
            assert_eq!(
                theme.node_fill_color(&NodeShape::Rectangle),
                theme.node_fill_color(&NodeShape::RoundRect),
                "{}: Rectangle and RoundRect should share color",
                theme.name(),
            );
            // 节点填充色不应等于背景色
            assert_ne!(
                theme.background_color(),
                theme.node_fill_color(&NodeShape::Rectangle),
                "{}: node fill should differ from background",
                theme.name(),
            );
        }
    }

    #[test]
    fn test_theme_names() {
        assert_eq!(DefaultTheme.name(), "Default");
        assert_eq!(DarkTheme.name(), "Dark");
        assert_eq!(ForestTheme.name(), "Forest");
        assert_eq!(NordicTheme.name(), "Nordic");
        assert_eq!(CappuccinoTheme.name(), "Cappuccino");
    }

    #[test]
    fn test_forest_theme_dark_background() {
        assert_eq!(ForestTheme.background_color(), "#1b2a1b");
    }

    // ─── ThemeRecord / RecordTheme ──────────────────────────

    #[test]
    fn test_builtin_theme_record_all_five() {
        for name in ["default", "dark", "forest", "nordic", "cappuccino"] {
            let record = builtin_theme_record(name)
                .unwrap_or_else(|| panic!("builtin record must exist for '{}'", name));
            assert_eq!(record.node_colors.len(), 7, "{}: 7 node color slots", name);
            assert!(!record.background.is_empty());
            assert!(!record.foreground.is_empty());
            assert!(record.base_font_size > 0.0);
            assert!(record.title_font_size > 0.0);
        }
        assert!(builtin_theme_record("unknown").is_none());
        assert!(builtin_theme_record("").is_none());
    }

    #[test]
    fn test_builtin_record_matches_static_theme_colors() {
        let record = builtin_theme_record("dark").unwrap();
        let theme = RecordTheme::new(record);
        assert_eq!(theme.background_color(), DarkTheme.background_color());
        assert_eq!(theme.node_stroke(), DarkTheme.node_stroke());
        assert_eq!(theme.edge_color(), DarkTheme.edge_color());
        assert_eq!(theme.node_text_color(), DarkTheme.node_text_color());
        assert_eq!(theme.title_color(), DarkTheme.title_color());
        for shape in [
            NodeShape::Rectangle, NodeShape::Subroutine, NodeShape::Diamond,
            NodeShape::Circle, NodeShape::Cylinder, NodeShape::Hexagon,
        ] {
            assert_eq!(
                theme.node_fill_color(&shape),
                DarkTheme.node_fill_color(&shape),
                "slot color must match static theme for {:?}",
                shape,
            );
        }
    }

    #[test]
    fn test_record_theme_shape_slot_coloring() {
        let theme = RecordTheme::new(builtin_theme_record("default").unwrap());
        // 6 槽语义分组：同组同色、跨组按槽取色
        assert_eq!(theme.node_fill_color(&NodeShape::Rectangle), theme.node_fill_color(&NodeShape::RoundRect));
        assert_eq!(
            theme.node_fill_color(&NodeShape::Rectangle),
            builtin_theme_record("default").unwrap().node_colors[0],
        );
        assert_eq!(
            theme.node_fill_color(&NodeShape::Cylinder),
            builtin_theme_record("default").unwrap().node_colors[4],
        );
    }

    #[test]
    fn test_record_theme_short_palette_falls_back_to_primary() {
        let record = ThemeRecord {
            node_colors: vec!["#111111".to_string()],
            ..builtin_theme_record("default").unwrap()
        };
        let theme = RecordTheme::new(record);
        // 缺失槽位（1-6）回落到第 0 槽
        assert_eq!(theme.node_fill_color(&NodeShape::Diamond), "#111111");
        assert_eq!(theme.node_fill_color(&NodeShape::Cylinder), "#111111");
        assert_eq!(theme.node_fill_color(&NodeShape::Asymmetric), "#111111");
    }

    // ─── T17: tint 填充 / 槽位描边 / 7 槽语义 ────────────────

    #[test]
    fn test_asymmetric_maps_to_external_slot() {
        assert_eq!(shape_slot(&NodeShape::Asymmetric), 6);
        assert_ne!(shape_slot(&NodeShape::Asymmetric), shape_slot(&NodeShape::Hexagon));
    }

    #[test]
    fn test_node_fill_contrast_guaranteed_wcag_aa() {
        // 宿主注入全饱和锚色起(R7):tint 必须保证文字对比度 ≥ 4.5 —
        // 用极端 vivid 槽位在深/浅两底上全槽遍历(对比度收缩守卫)
        let vivid = vec![
            "#4a7dde".to_string(), "#a855f7".to_string(), "#f59e0b".to_string(),
            "#06b6d4".to_string(), "#22c55e".to_string(), "#f43f5e".to_string(),
            "#8b5cf6".to_string(),
        ];
        for (bg, fg) in [("#1a1b26", "#c0caf5"), ("#ffffff", "#1f2328")] {
            let record = ThemeRecord {
                background: bg.to_string(),
                foreground: fg.to_string(),
                node_colors: vivid.clone(),
                ..builtin_theme_record("default").unwrap()
            };
            let theme = RecordTheme::new(record);
            let shapes = [
                NodeShape::Rectangle,
                NodeShape::Subroutine,
                NodeShape::Diamond,
                NodeShape::Circle,
                NodeShape::Cylinder,
                NodeShape::Hexagon,
                NodeShape::Asymmetric,
            ];
            for (i, shape) in shapes.iter().enumerate() {
                match theme.node_fill(shape) {
                    FillStyle::Color(c) => {
                        let ratio = wcag_contrast(fg, &c);
                        assert!(
                            ratio >= 4.5,
                            "slot{i} on {bg}: 文字对比度 {ratio:.2} < 4.5 (fill {c})"
                        );
                    }
                    other => panic!("expected solid tint, got {:?}", other),
                }
            }
        }
    }

    #[test]
    fn test_record_theme_node_fill_is_tint_of_background_and_slot() {
        let record = builtin_theme_record("dark").unwrap();
        let theme = RecordTheme::new(record.clone());
        let fill = theme.node_fill(&NodeShape::Rectangle);
        // R7: luma 感知 tint — 深底 26%（低比例深底几乎不可见）
        let expected = mix_colors(&record.background, &record.node_colors[0], NODE_TINT_ALPHA_DARK).unwrap();
        match fill {
            FillStyle::Color(c) => {
                assert_eq!(c, expected, "fill = 底色混角色色(深底 26%)");
                assert_ne!(c, record.node_colors[0], "非全饱和原色");
                assert_ne!(c, record.background, "非纯底色");
            }
            other => panic!("expected solid tint, got {:?}", other),
        }
        // 描边 = 角色色明度自适应（深底提亮 12% — vivid 对比）
        assert_eq!(
            theme.node_stroke_for(&NodeShape::Rectangle),
            lighten_color(&record.node_colors[0], 0.12)
        );
        assert_eq!(
            theme.node_stroke_for(&NodeShape::Cylinder),
            lighten_color(&record.node_colors[4], 0.12)
        );

        // 浅底路径:tint 16% + 描边压暗 8%
        let light = builtin_theme_record("default").unwrap();
        let light_theme = RecordTheme::new(light.clone());
        let expected_light = mix_colors(&light.background, &light.node_colors[0], NODE_TINT_ALPHA_LIGHT).unwrap();
        assert_eq!(light_theme.node_fill(&NodeShape::Rectangle), FillStyle::Color(expected_light));
        assert_eq!(
            light_theme.node_stroke_for(&NodeShape::Rectangle),
            darken_color(&light.node_colors[0], 0.08)
        );
    }

    #[test]
    fn test_tint_non_hex_colors_fall_back_to_slot() {
        let record = ThemeRecord {
            background: "not-a-color".to_string(),
            node_colors: vec!["#123456".to_string(); 7],
            ..builtin_theme_record("default").unwrap()
        };
        let theme = RecordTheme::new(record);
        assert_eq!(theme.node_fill(&NodeShape::Diamond), FillStyle::Color("#123456".to_string()));
        // 子图背景不可解析色回退到原背景(R9 — rgb()/rgba() 现已可解析,不在此列)
        let record2 = ThemeRecord {
            background: "pale-violet-red".to_string(),
            ..builtin_theme_record("default").unwrap()
        };
        let theme2 = RecordTheme::new(record2);
        assert_eq!(theme2.subgraph_background(), "pale-violet-red");
    }

    #[test]
    fn test_subgraph_background_is_foreground_tint_of_background() {
        let record = builtin_theme_record("dark").unwrap();
        let theme = RecordTheme::new(record.clone());
        let expected = mix_colors(&record.background, &record.foreground, SUBGRAPH_BG_ALPHA).unwrap();
        assert_eq!(theme.subgraph_background(), expected);
        assert_ne!(theme.subgraph_background(), record.background);
    }

    #[test]
    fn test_color_utils() {
        assert_eq!(parse_hex_color("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_color("#000"), Some((0, 0, 0)));
        // R9:rgb()/rgba() CSS 色串可解析(alpha 丢弃 — 槽位色无半透明语义)
        assert_eq!(parse_hex_color("rgba(1,2,3,0.5)"), Some((1, 2, 3)));
        assert_eq!(parse_hex_color("rgb(10, 20, 30)"), Some((10, 20, 30)));
        assert_eq!(parse_hex_color("rgba(300,1,1,1)"), None, "通道越界拒绝");
        assert_eq!(parse_hex_color("not-a-color"), None);
        assert_eq!(mix_colors("#000000", "#ffffff", 0.5).unwrap(), "#808080");
        assert_eq!(mix_colors("#000000", "nope", 0.5), None);
        assert_eq!(lighten_color("#000000", 0.5), "#808080");
        assert_eq!(lighten_color("nope", 0.5), "nope");
    }

    // ─── T20/T21: preset 集成 ────────────────────────────────

    #[test]
    fn test_record_theme_style_preset_parsed() {
        let record = ThemeRecord {
            style_preset: Some("signal-flow".to_string()),
            ..builtin_theme_record("default").unwrap()
        };
        assert_eq!(RecordTheme::new(record).style_preset(), StylePreset::SignalFlow);

        let record = ThemeRecord {
            style_preset: Some("blueprint".to_string()),
            ..builtin_theme_record("default").unwrap()
        };
        assert_eq!(RecordTheme::new(record).style_preset(), StylePreset::Blueprint);

        // 缺省 / 未知 → Classic
        assert_eq!(RecordTheme::new(builtin_theme_record("default").unwrap()).style_preset(), StylePreset::Classic);
    }

    #[test]
    fn test_editorial_preset_boosts_font_sizes() {
        let record = ThemeRecord {
            style_preset: Some("editorial".to_string()),
            base_font_size: 14.0,
            title_font_size: 18.0,
            ..builtin_theme_record("default").unwrap()
        };
        let theme = RecordTheme::new(record);
        assert_eq!(theme.font_size(), 15.0, "Editorial 字号 +1");
        assert_eq!(theme.title_font_size(), 19.0);
    }

    #[test]
    fn test_vertical_gradient_fill() {
        let base = FillStyle::Color("#313244".into());
        let g = vertical_gradient_fill(&base, 10.0, 20.0, 100.0, 50.0, 0.10);
        match g {
            FillStyle::Gradient(grad) => {
                assert!(matches!(grad.kind, GradientKind::Linear { x0, y0, x1, y1 }
                    if (x0 - 10.0).abs() < 1e-9 && (y0 - 20.0).abs() < 1e-9
                        && (x1 - 10.0).abs() < 1e-9 && (y1 - 70.0).abs() < 1e-9));
                assert_eq!(grad.stops.len(), 2);
                // R7: 顶部提亮 → 底部微暗(光自上来)
                assert_eq!(grad.stops[0].color, lighten_color("#313244", 0.10));
                assert_eq!(grad.stops[1].color, darken_color("#313244", 0.05));
            }
            other => panic!("expected gradient, got {:?}", other),
        }
        // 非 Color 基色原样返回
        let none = FillStyle::None;
        assert_eq!(vertical_gradient_fill(&none, 0.0, 0.0, 1.0, 1.0, 0.1), FillStyle::None);
    }

    #[test]
    fn test_record_theme_default_is_default_theme() {
        let record = ThemeRecord::default();
        assert_eq!(record.background, "#ffffff");
        assert_eq!(record.node_colors[0], "#dae8fc");
        assert_eq!(record.margin, Margin::all(20.0));
    }

    #[test]
    fn test_record_theme_margin_and_fonts() {
        let mut record = builtin_theme_record("nordic").unwrap();
        record.font_family = "Serif".to_string();
        record.base_font_size = 16.0;
        record.margin = Margin::all(12.0);
        let theme = RecordTheme::new(record);
        assert_eq!(theme.font_family(), "Serif");
        assert_eq!(theme.font_size(), 16.0);
        assert_eq!(theme.margin(), Margin::all(12.0));
    }

    #[test]
    fn test_scaled_fonts_record() {
        let record = builtin_theme_record("default").unwrap();
        let scaled = record.with_scaled_fonts(0.5);
        assert_eq!(scaled.base_font_size, record.base_font_size * 0.5);
        assert_eq!(scaled.title_font_size, record.title_font_size * 0.5);
        assert_eq!(scaled.node_colors, record.node_colors);
    }
}
