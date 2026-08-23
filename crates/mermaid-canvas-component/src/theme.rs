//! 主题系统
//!
//! 按 **节点形状 (NodeShape)** 分配颜色 — 同类型同色，颜色传递语义。
//! 每个主题定义 6 个色槽，形状按语义分组映射：
//!
//! | 色槽 | 形状 | 语义 |
//! |------|------|------|
//! | primary | Rectangle, RoundRect, Stadium | 普通流程节点 |
//! | secondary | Subroutine | 子流程 |
//! | accent | Diamond | 判断/分支 |
//! | info | Circle, DoubleCircle | 起止/连接 |
//! | data | Cylinder | 数据存储 |
//! | special | Hexagon, Parallelogram, Trapezoid, Asymmetric | 特殊处理 |

use mermaid_canvas_core::style::FillStyle;
use mermaid_canvas_core::NodeShape;

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

/// 形状→色槽映射
///
/// 将 NodeShape 归入 6 个语义色槽，主题只需定义 6 个颜色。
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
        NodeShape::Hexagon | NodeShape::Parallelogram
        | NodeShape::Trapezoid | NodeShape::Asymmetric => 5,
    }
}

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

    /// 节点填充色（按形状类型）
    fn node_fill_color(&self, shape: &NodeShape) -> &str;

    /// 节点填充样式
    fn node_fill(&self, shape: &NodeShape) -> FillStyle {
        FillStyle::Color(self.node_fill_color(shape).to_string())
    }

    /// 节点边框色
    fn node_stroke(&self) -> &str;

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

    /// 边距
    fn margin(&self) -> Margin {
        Margin::default()
    }
}

// ═══════════════════════════════════════════════════════════════
// 内置主题 — 每个主题 6 色槽: primary, secondary, accent, info, data, special
// ═══════════════════════════════════════════════════════════════

// ─── Default（经典浅色）───────────────────────────────────────

/// 默认浅色主题
pub struct DefaultTheme;

impl DefaultTheme {
    const PALETTE: [&'static str; 6] = [
        "#dae8fc", // primary: 蓝
        "#e1d5e7", // secondary: 紫
        "#fff2cc", // accent: 黄
        "#d5e8d4", // info: 绿
        "#f8cecc", // data: 红
        "#fff2cc", // special: 黄
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
    const PALETTE: [&'static str; 6] = [
        "#313244", // primary: 深蓝灰
        "#45475a", // secondary: 中灰
        "#3b3b55", // accent: 深紫灰
        "#2a3a4a", // info: 深蓝
        "#3a2a2a", // data: 深红棕
        "#3b3b55", // special: 深紫灰
    ];
}

impl Theme for DarkTheme {
    fn name(&self) -> &str { "Dark" }
    fn background_color(&self) -> &str { "#1e1e2e" }
    fn node_fill_color(&self, shape: &NodeShape) -> &str {
        Self::PALETTE[shape_slot(shape)]
    }
    fn node_stroke(&self) -> &str { "#89b4fa" }
    fn node_text_color(&self) -> &str { "#cdd6f4" }
    fn edge_color(&self) -> &str { "#6c7086" }
    fn edge_label_background(&self) -> &str { "#1e1e2e" }
    fn subgraph_background(&self) -> &str { "#181825" }
    fn subgraph_border(&self) -> &str { "#585b70" }
    fn title_color(&self) -> &str { "#cdd6f4" }
}

// ─── Forest（森林绿）─────────────────────────────────────────

/// 森林主题 — 深绿底 + 多层次绿色
pub struct ForestTheme;

impl ForestTheme {
    const PALETTE: [&'static str; 6] = [
        "#2d5a27", // primary: 深绿
        "#3a6b34", // secondary: 中绿
        "#4a7c3f", // accent: 亮绿
        "#1e4d2b", // info: 暗绿
        "#5a3a27", // data: 棕绿
        "#3a6b34", // special: 中绿
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
    const PALETTE: [&'static str; 6] = [
        "#dfe6ed", // primary: 冷蓝灰
        "#e8edf2", // secondary: 浅蓝灰
        "#f0e6ec", // accent: 淡粉
        "#e2e8f0", // info: 蓝灰
        "#e0ddd8", // data: 暖灰
        "#ede9e6", // special: 米灰
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
    const PALETTE: [&'static str; 6] = [
        "#e8d5c4", // primary: 奶咖
        "#dcc8b4", // secondary: 浅棕
        "#f0e0d0", // accent: 奶白
        "#d4b896", // info: 焦糖
        "#c9a882", // data: 深焦糖
        "#e0cdc0", // special: 米棕
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
/// `node_colors` 为 6 语义色槽（primary/secondary/accent/info/data/special），
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
    /// 6 语义节点色槽
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
/// `node_colors` 少于 6 项时，缺失槽位回落到第 0 槽（primary），
/// 保证宿主注入不完整时仍可渲染。
pub struct RecordTheme {
    record: ThemeRecord,
}

impl RecordTheme {
    /// 从记录创建
    pub fn new(record: ThemeRecord) -> Self {
        Self { record }
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
    fn font_size(&self) -> f64 { self.record.base_font_size }
    fn node_fill_color(&self, shape: &NodeShape) -> &str { self.slot_color(shape) }
    fn node_stroke(&self) -> &str { &self.record.node_stroke }
    fn node_text_color(&self) -> &str { &self.record.foreground }
    fn edge_color(&self) -> &str { &self.record.edge_color }
    fn edge_label_background(&self) -> &str { &self.record.edge_label_background }
    /// 子图背景不在 WIT record 中（当前布局未产出子图），由背景色承担
    fn subgraph_background(&self) -> &str { &self.record.background }
    /// 子图边框由节点边框色承担
    fn subgraph_border(&self) -> &str { &self.record.node_stroke }
    fn title_color(&self) -> &str { &self.record.title_color }
    fn margin(&self) -> Margin { self.record.margin.clone() }
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
            assert_eq!(record.node_colors.len(), 6, "{}: 6 node color slots", name);
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
        // 缺失槽位（1-5）回落到第 0 槽
        assert_eq!(theme.node_fill_color(&NodeShape::Diamond), "#111111");
        assert_eq!(theme.node_fill_color(&NodeShape::Cylinder), "#111111");
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
