//! Mermaid 语法解析器
//!
//! 将 Mermaid 文本解析为 DiagramAst。
//! 与 deneb-core 中 parser/ 模块定位一致：core 定义类型，core 实现解析。

pub mod class;
pub mod er;
pub mod flowchart;
pub mod packet;
pub mod requirement;
pub mod sequence;
pub mod state;

use crate::diagram::{DiagramAst, DiagramKind};
use crate::error::CoreError;

/// 解析 Mermaid 源码为图表 AST
pub fn parse_mermaid(input: &str) -> Result<DiagramAst, CoreError> {
    let kind = detect_diagram_kind(input)
        .ok_or_else(|| CoreError::parse_error("无法识别图表类型"))?;

    match kind {
        DiagramKind::Flowchart => flowchart::parse_flowchart(input),
        DiagramKind::Sequence => sequence::parse_sequence(input),
        DiagramKind::Class => class::parse_class(input),
        DiagramKind::State => state::parse_state(input),
        DiagramKind::Er => er::parse_er(input),
        DiagramKind::Requirement => requirement::parse_requirement(input),
        DiagramKind::Packet => packet::parse_packet(input),
        DiagramKind::Pie | DiagramKind::Mindmap
        | DiagramKind::Journey | DiagramKind::Timeline | DiagramKind::Gantt
        | DiagramKind::GitGraph | DiagramKind::C4
        | DiagramKind::Sankey | DiagramKind::Quadrant | DiagramKind::Block
        | DiagramKind::Kanban | DiagramKind::Architecture
        | DiagramKind::Radar | DiagramKind::Treemap | DiagramKind::XYChart => {
            Err(CoreError::parse_error(format!(
                "图表类型 {:?} 尚未实现",
                kind
            )))
        }
    }
}

/// 检测图表类型
pub fn detect_diagram_kind(input: &str) -> Option<DiagramKind> {
    // 先检查是否有 YAML frontmatter（--- ... ---），跳过它
    let mut lines = input.lines();
    let mut first_content_line: Option<&str> = None;

    if let Some(first) = lines.next() {
        let trimmed = first.trim();
        if trimmed == "---" {
            // 跳过 frontmatter 内容，直到遇到结束的 ---
            for line in lines.by_ref() {
                if line.trim() == "---" {
                    break;
                }
            }
            // frontmatter 结束后，用同一迭代器继续取第一个非空行
            for line in lines.by_ref() {
                let t = line.trim();
                if !t.is_empty() {
                    first_content_line = Some(t);
                    break;
                }
            }
        } else {
            first_content_line = Some(trimmed);
        }
    }

    // 如果 frontmatter 后没有内容，回退到原始首行
    let header = first_content_line
        .or_else(|| input.lines().next().map(|l| l.trim()))
        .unwrap_or("");

    if header.starts_with("flowchart") || header.starts_with("graph") {
        Some(DiagramKind::Flowchart)
    } else if header.starts_with("sequenceDiagram") {
        Some(DiagramKind::Sequence)
    } else if header.starts_with("classDiagram") {
        Some(DiagramKind::Class)
    } else if header.starts_with("stateDiagram") {
        Some(DiagramKind::State)
    } else if header.starts_with("erDiagram") {
        Some(DiagramKind::Er)
    } else if header.starts_with("pie") {
        Some(DiagramKind::Pie)
    } else if header.starts_with("mindmap") {
        Some(DiagramKind::Mindmap)
    } else if header.starts_with("journey") {
        Some(DiagramKind::Journey)
    } else if header.starts_with("timeline") {
        Some(DiagramKind::Timeline)
    } else if header.starts_with("gantt") {
        Some(DiagramKind::Gantt)
    } else if header.starts_with("requirementDiagram") {
        Some(DiagramKind::Requirement)
    } else if header.starts_with("gitGraph") {
        Some(DiagramKind::GitGraph)
    } else if header.starts_with("C4Context")
        || header.starts_with("C4Container")
        || header.starts_with("C4Component")
        || header.starts_with("C4Dynamic")
        || header.starts_with("C4Deployment")
    {
        Some(DiagramKind::C4)
    } else if header.starts_with("sankey") {
        Some(DiagramKind::Sankey)
    } else if header.starts_with("quadrantChart") {
        Some(DiagramKind::Quadrant)
    } else if header.starts_with("block") {
        Some(DiagramKind::Block)
    } else if header.starts_with("packet") {
        Some(DiagramKind::Packet)
    } else if header.starts_with("kanban") {
        Some(DiagramKind::Kanban)
    } else if header.starts_with("architecture") {
        Some(DiagramKind::Architecture)
    } else if header.starts_with("radar") {
        Some(DiagramKind::Radar)
    } else if header.starts_with("treemap") {
        Some(DiagramKind::Treemap)
    } else if header.starts_with("xychart") || header.starts_with("xychart-beta") {
        Some(DiagramKind::XYChart)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_flowchart() {
        assert_eq!(
            detect_diagram_kind("flowchart TD\n    A --> B"),
            Some(DiagramKind::Flowchart)
        );
        assert_eq!(
            detect_diagram_kind("graph LR\n    A --> B"),
            Some(DiagramKind::Flowchart)
        );
    }

    #[test]
    fn test_detect_class() {
        assert_eq!(
            detect_diagram_kind("classDiagram\n    Animal <|-- Dog"),
            Some(DiagramKind::Class)
        );
    }

    #[test]
    fn test_detect_state() {
        assert_eq!(
            detect_diagram_kind("stateDiagram-v2\n    [*] --> Active"),
            Some(DiagramKind::State)
        );
    }

    #[test]
    fn test_detect_sequence() {
        assert_eq!(
            detect_diagram_kind("sequenceDiagram\n    A->>B: hello"),
            Some(DiagramKind::Sequence)
        );
    }

    #[test]
    fn test_detect_all_types() {
        assert_eq!(detect_diagram_kind("erDiagram\n  A ||--o{ B"), Some(DiagramKind::Er));
        assert_eq!(detect_diagram_kind("pie title test\n  \"A\" : 1"), Some(DiagramKind::Pie));
        assert_eq!(detect_diagram_kind("mindmap\n  root"), Some(DiagramKind::Mindmap));
        assert_eq!(detect_diagram_kind("journey\n  title test"), Some(DiagramKind::Journey));
        assert_eq!(detect_diagram_kind("gantt\n  title test"), Some(DiagramKind::Gantt));
        assert_eq!(detect_diagram_kind("gitGraph\n  commit"), Some(DiagramKind::GitGraph));
        assert_eq!(detect_diagram_kind("C4Context\n  title test"), Some(DiagramKind::C4));
        assert_eq!(detect_diagram_kind("sankey\n  A,B,10"), Some(DiagramKind::Sankey));
        assert_eq!(detect_diagram_kind("quadrantChart\n  title test"), Some(DiagramKind::Quadrant));
        assert_eq!(detect_diagram_kind("xychart\n  title test"), Some(DiagramKind::XYChart));
        assert_eq!(detect_diagram_kind("timeline\n  title test"), Some(DiagramKind::Timeline));
        assert_eq!(detect_diagram_kind("requirementDiagram\n  test"), Some(DiagramKind::Requirement));
        assert_eq!(detect_diagram_kind("packet\n  title test"), Some(DiagramKind::Packet));
        assert_eq!(detect_diagram_kind("kanban\n  todo"), Some(DiagramKind::Kanban));
        assert_eq!(detect_diagram_kind("architecture\n  test"), Some(DiagramKind::Architecture));
        assert_eq!(detect_diagram_kind("radar\n  title test"), Some(DiagramKind::Radar));
        assert_eq!(detect_diagram_kind("treemap\n  title test"), Some(DiagramKind::Treemap));
        assert_eq!(detect_diagram_kind("block\n  test"), Some(DiagramKind::Block));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_diagram_kind("something random"), None);
        assert_eq!(detect_diagram_kind(""), None);
    }

    #[test]
    fn test_detect_with_frontmatter() {
        // Frontmatter should be skipped, diagram detected after ---
        let input = "---\ntitle: My Diagram\nauthor: test\n---\nflowchart TD\n    A --> B";
        assert_eq!(detect_diagram_kind(input), Some(DiagramKind::Flowchart));

        let input2 = "---\nconfig: true\n---\nclassDiagram\n    A <|-- B";
        assert_eq!(detect_diagram_kind(input2), Some(DiagramKind::Class));

        // Frontmatter with title field should NOT be mis-detected
        let input3 = "---\ntitle: My State\n---\nstateDiagram-v2\n    [*] --> S";
        assert_eq!(detect_diagram_kind(input3), Some(DiagramKind::State));
    }
}
