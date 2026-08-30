//! 形轴 preset — 形态学参数表（T20 + R7 光影扩展）
//!
//! 四档 preset 只管**形**（圆角/边宽/辉光/网格/动效档位/sigil/光影）；
//! **色彩恒来自 theme 槽位**（形轴分离铁律）。
//!
//! | preset | 圆角 | 描边 | 边辉光 | 网格 | stagger | 渐变 | 柔影 | 内高光 |
//! |--------|------|------|--------|------|---------|------|------|--------|
//! | Classic | 8px | 1.5 | — | — | 1.0x | 微 5% | 轻 | ✓ |
//! | SignalFlow | 6px | 1.4 | 4 层 | 微格 | 0.8x | 10% | 明显 | ✓ |
//! | Blueprint | 2px | 1.0 | — | 双尺度 | 0x（无） | — | — | — |
//! | Editorial | 10px | 1.25 | — | — | 1.0x | 6% | 轻 | ✓ |

/// 形轴 preset（diagram-theme.style-preset 字符串的解析结果）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StylePreset {
    /// 经典 — 标准形态
    Classic,
    /// 信号流 — 辉光 + 渐变 + 加强动效
    SignalFlow,
    /// 蓝图 — 近直角 + 网格化 + 无 stagger
    Blueprint,
    /// 社论 — 大圆角 + 衬线感（字号 +1，暖色由宿主注入）
    Editorial,
}

impl Default for StylePreset {
    fn default() -> Self {
        StylePreset::Classic
    }
}

/// hover 效果参数（kind 与 echodawn:canvas hover-effect 词汇一致）
#[derive(Clone, Debug, PartialEq)]
pub struct HoverEffectSpec {
    /// "brighten" | "scale" | "lift" | "outline" | "glow"
    pub kind: &'static str,
    /// kind 语义参数
    pub params: Vec<f64>,
}

/// 柔影档位：(垂直偏移 px, 扩散 px, 强度)
/// (offset-y, blur, spread, alpha) — CSS box-shadow 外阴影子集（R10 软阴影）
pub type ShadowSpec = (f64, f64, f64, f64);

impl StylePreset {
    /// 从 style-preset 字符串解析（None/未知 → Classic）
    pub fn parse(s: &Option<String>) -> Self {
        match s.as_deref() {
            Some("signal-flow") => StylePreset::SignalFlow,
            Some("blueprint") => StylePreset::Blueprint,
            Some("editorial") => StylePreset::Editorial,
            _ => StylePreset::Classic,
        }
    }

    /// 圆角半径（px）— RoundRect / 子图框 / 参与头
    pub fn corner_radius(&self) -> f64 {
        match self {
            StylePreset::Classic => 8.0,
            StylePreset::SignalFlow => 6.0,
            StylePreset::Blueprint => 2.0,
            StylePreset::Editorial => 10.0,
        }
    }

    /// 基础描边宽度（px）— R9:SignalFlow 边线收敛(1.75→1.4,辉光已提供
    /// 分量感,主线过粗压迫图表留白)
    pub fn stroke_width(&self) -> f64 {
        match self {
            StylePreset::Classic => 1.5,
            StylePreset::SignalFlow => 1.4,
            StylePreset::Blueprint => 1.0,
            StylePreset::Editorial => 1.25,
        }
    }

    /// 边辉光（沿边路径多层半透明描边模拟）
    pub fn edge_glow(&self) -> bool {
        matches!(self, StylePreset::SignalFlow)
    }

    /// 辉光层参数：(宽度, alpha) 四层 — 宽度递增、alpha 递减（从内到外）
    pub fn glow_layers(&self) -> &'static [(f64, f64)] {
        // SignalFlow: R9 收紧 — 主线 1.4 起步,四级扩散更贴线(避免光柱化)
        &[(2.0, 0.24), (3.2, 0.12), (4.8, 0.06), (6.5, 0.03)]
    }

    /// 箭头辉光（SignalFlow — 箭头本体前置半透明放大副本）
    pub fn arrow_glow(&self) -> bool {
        matches!(self, StylePreset::SignalFlow)
    }

    /// 背景细网格：(间距 px, alpha)；None = 无
    pub fn fine_grid(&self) -> Option<(f64, f64)> {
        match self {
            StylePreset::Blueprint => Some((Self::GRID_SPACING, 0.05)),
            StylePreset::SignalFlow => Some((32.0, 0.035)),
            _ => None,
        }
    }

    /// 背景主网格（Blueprint 双尺度工程感 — 细格之上再叠 5x 主格）
    pub fn major_grid(&self) -> Option<(f64, f64)> {
        match self {
            StylePreset::Blueprint => Some((Self::GRID_SPACING * 5.0, 0.09)),
            _ => None,
        }
    }

    /// 网格间距（px）
    pub const GRID_SPACING: f64 = 24.0;

    /// 背景顶部提光（垂直渐变叠加 alpha；0 = 无 — "光自上来"的层次感）
    pub fn top_light(&self) -> f64 {
        match self {
            StylePreset::Classic => 0.035,
            StylePreset::SignalFlow => 0.05,
            StylePreset::Editorial => 0.03,
            StylePreset::Blueprint => 0.0,
        }
    }

    /// 入场 stagger 档位（乘在 stagger-ms 上；0 = 无 stagger）
    pub fn stagger_factor(&self) -> f64 {
        match self {
            StylePreset::Classic => 1.0,
            StylePreset::SignalFlow => 0.8,
            StylePreset::Blueprint => 0.0,
            StylePreset::Editorial => 1.0,
        }
    }

    /// 节点渐变填充（垂直线性 — 顶部提亮 → 底部微暗）
    pub fn gradient_fill(&self) -> bool {
        self.gradient_range() > 0.0
    }

    /// 渐变强度（顶部提亮比例；0 = 平涂）
    pub fn gradient_range(&self) -> f64 {
        match self {
            StylePreset::SignalFlow => 0.10,
            StylePreset::Classic => 0.05,
            StylePreset::Editorial => 0.06,
            StylePreset::Blueprint => 0.0,
        }
    }

    /// 节点软阴影（None = 平面风）。高模糊 + 低 alpha = 环境光遮蔽式柔影
    /// （宿主 SDF 高斯 pass 渲染），入场缩放下随主体淡入，无硬边黑框
    pub fn node_shadow(&self) -> Option<ShadowSpec> {
        match self {
            StylePreset::Classic => Some((2.5, 7.0, 0.5, 0.16)),
            StylePreset::SignalFlow => Some((3.5, 10.0, 0.5, 0.20)),
            StylePreset::Editorial => Some((2.0, 6.0, 0.0, 0.14)),
            StylePreset::Blueprint => None,
        }
    }

    /// 节点内侧高光 bevel（1px 内缩描边 — 卡片 crisp 边）
    pub fn inset_highlight(&self) -> bool {
        !matches!(self, StylePreset::Blueprint)
    }

    /// 节点语义 sigil（T18）
    pub fn sigils(&self) -> bool {
        matches!(self, StylePreset::SignalFlow | StylePreset::Blueprint)
    }

    /// 字号增量（px）— Editorial 衬线感（字号 +1）
    pub fn font_boost(&self) -> f64 {
        match self {
            StylePreset::Editorial => 1.0,
            _ => 0.0,
        }
    }

    /// hover 效果档位（T23 — hit-region 声明式 hover）
    pub fn hover_effect(&self) -> HoverEffectSpec {
        match self {
            StylePreset::Classic => HoverEffectSpec { kind: "brighten", params: vec![0.12] },
            StylePreset::SignalFlow => HoverEffectSpec { kind: "glow", params: vec![0.6] },
            StylePreset::Blueprint => HoverEffectSpec { kind: "outline", params: vec![1.5] },
            // Editorial 未指定 — 取 lift 2px（暖调编辑风的轻抬升）
            StylePreset::Editorial => HoverEffectSpec { kind: "lift", params: vec![2.0] },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_four_and_fallbacks() {
        assert_eq!(StylePreset::parse(&None), StylePreset::Classic);
        assert_eq!(StylePreset::parse(&Some("classic".into())), StylePreset::Classic);
        assert_eq!(StylePreset::parse(&Some("signal-flow".into())), StylePreset::SignalFlow);
        assert_eq!(StylePreset::parse(&Some("blueprint".into())), StylePreset::Blueprint);
        assert_eq!(StylePreset::parse(&Some("editorial".into())), StylePreset::Editorial);
        assert_eq!(StylePreset::parse(&Some("unknown".into())), StylePreset::Classic, "未知回落 classic");
        assert_eq!(StylePreset::parse(&Some("".into())), StylePreset::Classic);
    }

    #[test]
    fn test_morphology_table() {
        assert_eq!(StylePreset::Classic.corner_radius(), 8.0);
        assert_eq!(StylePreset::SignalFlow.corner_radius(), 6.0);
        assert_eq!(StylePreset::Blueprint.corner_radius(), 2.0);
        assert_eq!(StylePreset::Editorial.corner_radius(), 10.0);

        assert_eq!(StylePreset::Blueprint.stroke_width(), 1.0);
        assert_eq!(StylePreset::SignalFlow.stroke_width(), 1.4);

        assert!(StylePreset::SignalFlow.edge_glow());
        assert!(!StylePreset::Classic.edge_glow());
        assert!(!StylePreset::Blueprint.edge_glow());

        assert!(StylePreset::Blueprint.fine_grid().is_some());
        assert!(StylePreset::Blueprint.major_grid().is_some(), "双尺度主格");
        assert!(StylePreset::SignalFlow.fine_grid().is_some(), "信号流微格");
        assert!(StylePreset::Classic.fine_grid().is_none());
        assert_eq!(StylePreset::GRID_SPACING, 24.0);

        assert_eq!(StylePreset::SignalFlow.stagger_factor(), 0.8);
        assert_eq!(StylePreset::Blueprint.stagger_factor(), 0.0);
        assert_eq!(StylePreset::Classic.stagger_factor(), 1.0);

        assert!(StylePreset::SignalFlow.gradient_fill());
        assert!(StylePreset::Classic.gradient_fill(), "R7: classic 也带微渐变");
        assert!(!StylePreset::Blueprint.gradient_fill());
        assert!(StylePreset::SignalFlow.gradient_range() > StylePreset::Classic.gradient_range());

        assert!(StylePreset::SignalFlow.node_shadow().is_some());
        assert!(StylePreset::Classic.node_shadow().is_some());
        assert!(StylePreset::Blueprint.node_shadow().is_none(), "蓝图平面风无柔影");
        assert!(StylePreset::Classic.inset_highlight());
        assert!(!StylePreset::Blueprint.inset_highlight());
        assert!(StylePreset::SignalFlow.arrow_glow());
        assert!(StylePreset::Blueprint.top_light() == 0.0);
        assert!(StylePreset::Classic.top_light() > 0.0);

        assert!(StylePreset::SignalFlow.sigils() && StylePreset::Blueprint.sigils());
        assert!(!StylePreset::Classic.sigils() && !StylePreset::Editorial.sigils());

        assert_eq!(StylePreset::Editorial.font_boost(), 1.0);
    }

    #[test]
    fn test_hover_effect_specs() {
        assert_eq!(StylePreset::Classic.hover_effect(), HoverEffectSpec { kind: "brighten", params: vec![0.12] });
        assert_eq!(StylePreset::SignalFlow.hover_effect(), HoverEffectSpec { kind: "glow", params: vec![0.6] });
        assert_eq!(StylePreset::Blueprint.hover_effect(), HoverEffectSpec { kind: "outline", params: vec![1.5] });
        // 全部 kind 均在 hover-effect 词汇表内
        for p in [StylePreset::Classic, StylePreset::SignalFlow, StylePreset::Blueprint, StylePreset::Editorial] {
            assert!(matches!(p.hover_effect().kind, "brighten" | "scale" | "lift" | "outline" | "glow"));
        }
    }
}
