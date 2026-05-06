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
}
