//! Style 类型定义
//!
//! 提供可视化中使用的各种样式类型，包括填充、描边、渐变、文本样式等。

use std::fmt;

/// 填充样式
#[derive(Clone, Debug, PartialEq)]
pub enum FillStyle {
    /// 纯色填充，CSS 颜色字符串: "#fff", "rgb(255,255,255)", "rgba(...)"
    Color(String),
    /// 渐变填充
    Gradient(Gradient),
    /// 无填充
    None,
}

impl FillStyle {
    /// 提取颜色字符串（如果是纯色填充）
    pub fn to_color_string(&self) -> String {
        match self {
            FillStyle::Color(c) => c.clone(),
            FillStyle::Gradient(_) => "#000".to_string(),
            FillStyle::None => "transparent".to_string(),
        }
    }
}

/// 描边样式
#[derive(Clone, Debug, PartialEq)]
pub enum StrokeStyle {
    /// 纯色描边，CSS 颜色字符串
    Color(String),
    /// 无描边
    None,
}

/// 渐变定义
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// 渐变类型
    pub kind: GradientKind,
    /// 渐变停止点
    pub stops: Vec<GradientStop>,
}

/// 渐变类型
#[derive(Clone, Debug, PartialEq)]
pub enum GradientKind {
    /// 线性渐变
    Linear {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    },
    /// 径向渐变
    Radial {
        x0: f64,
        y0: f64,
        r0: f64,
        x1: f64,
        y1: f64,
        r1: f64,
    },
}

/// 渐变停止点
#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    /// 偏移量 (0.0 - 1.0)
    pub offset: f64,
    /// 颜色字符串
    pub color: String,
}

impl GradientStop {
    /// 创建新的渐变停止点
    pub fn new(offset: f64, color: impl Into<String>) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            color: color.into(),
        }
    }
}

/// 文本样式
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// 字体家族
    pub font_family: String,
    /// 字体大小
    pub font_size: f64,
    /// 字体粗细
    pub font_weight: FontWeight,
    /// 字体样式
    pub font_style: FontStyle,
    /// 填充样式
    pub fill: FillStyle,
}

impl TextStyle {
    /// 创建默认文本样式
    pub fn new() -> Self {
        Self {
            font_family: "sans-serif".to_string(),
            font_size: 12.0,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            fill: FillStyle::Color("#000".to_string()),
        }
    }

    /// 设置字体家族
    pub fn with_font_family(mut self, font_family: impl Into<String>) -> Self {
        self.font_family = font_family.into();
        self
    }

    /// 设置字体大小
    pub fn with_font_size(mut self, font_size: f64) -> Self {
        self.font_size = font_size;
        self
    }

    /// 设置填充样式
    pub fn with_fill(mut self, fill: FillStyle) -> Self {
        self.fill = fill;
        self
    }

    /// 生成 CSS 字体字符串
    pub fn to_css_font(&self) -> String {
        let style = match self.font_style {
            FontStyle::Normal => "",
            FontStyle::Italic => "italic ",
        };
        let weight = match self.font_weight {
            FontWeight::Normal => "normal ",
            FontWeight::Bold => "bold ",
            FontWeight::Number(n) => {
                if n >= 100 && n <= 900 && n % 100 == 0 {
                    &format!("{} ", n)
                } else {
                    "400 "
                }
            }
        };
        format!("{}{}{}px {}", style, weight, self.font_size, self.font_family)
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::new()
    }
}

/// 字体粗细
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontWeight {
    /// 正常
    Normal,
    /// 粗体
    Bold,
    /// 数字值 (100-900, 100 的倍数)
    Number(u16),
}

/// 字体样式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    /// 正常
    Normal,
    /// 斜体
    Italic,
}

/// 文本锚点
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAnchor {
    /// 起点
    Start,
    /// 中间
    Middle,
    /// 终点
    End,
}

impl fmt::Display for TextAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextAnchor::Start => write!(f, "start"),
            TextAnchor::Middle => write!(f, "middle"),
            TextAnchor::End => write!(f, "end"),
        }
    }
}

/// 文本基线
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextBaseline {
    /// 顶部
    Top,
    /// 中间
    Middle,
    /// 底部
    Bottom,
    /// 字母基线（默认）
    Alphabetic,
}

impl fmt::Display for TextBaseline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextBaseline::Top => write!(f, "top"),
            TextBaseline::Middle => write!(f, "middle"),
            TextBaseline::Bottom => write!(f, "bottom"),
            TextBaseline::Alphabetic => write!(f, "alphabetic"),
        }
    }
}
