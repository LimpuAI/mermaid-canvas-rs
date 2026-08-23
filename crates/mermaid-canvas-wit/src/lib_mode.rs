//! 库调用模式 API（v2：经 DiagramSession 的向后兼容单发入口）

use crate::session::DiagramSession;
use crate::wit_types::*;

/// 渲染 Mermaid 源码为 Canvas 2D 指令（单发语义 — 内部建立会话渲染 t=1 稳态）
///
/// `theme` 可选值: `"default"`, `"dark"`, `"forest"`, `"nordic"`, `"cappuccino"`,
/// `None` 等同 `"default"`
pub fn render(source: &str, theme: Option<&str>) -> Result<WitRenderResult, String> {
    let opts = WitDiagramOptions {
        width: None,
        theme: theme.map(str::to_string),
        animation: None,
    };
    let mut session = DiagramSession::new(source.to_string(), Some(opts));
    session.render(1.0)
}

/// 命中区列表（v2：会话方法替代 v1 的 render-result 内嵌区域；宿主侧 AABB 命中）
pub fn hit_regions(source: &str, theme: Option<&str>) -> Result<Vec<WitHitRegion>, String> {
    let opts = WitDiagramOptions {
        width: None,
        theme: theme.map(str::to_string),
        animation: None,
    };
    let mut session = DiagramSession::new(source.to_string(), Some(opts));
    if let Some(e) = session.parse_error() {
        return Err(e.to_string());
    }
    Ok(session.hit_regions())
}

/// 宿主侧 AABB 命中测试（零 wasm 调用路径的参考实现）
pub fn hit_test(regions: &[WitHitRegion], x: f64, y: f64, tolerance: f64) -> Option<u32> {
    for region in regions {
        let bounds = mermaid_canvas_core::BoundingBox::new(
            region.bounds_x, region.bounds_y, region.bounds_w, region.bounds_h,
        );
        if bounds.expand(tolerance).contains(x, y) {
            return Some(region.index);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: assert render succeeds and has non-empty layers with actual commands.
    fn assert_valid_render(result: &WitRenderResult) {
        assert!(!result.layers.is_empty(), "render result should have layers");
        let has_commands = result.layers.iter().any(|l| !l.commands.is_empty());
        assert!(has_commands, "at least one layer should contain draw commands");
    }

    // ─── Flowchart ─────────────────────────────────────────

    #[test]
    fn test_render_simple_flowchart() {
        let result = render("flowchart TD\n    A --> B", None)
            .expect("simple flowchart should render");
        assert_valid_render(&result);
    }

    #[test]
    fn test_render_complex_flowchart() {
        let src = "flowchart TD\n    A[Start] -->|go| B{Choice?} --> C[(DB)]";
        let result = render(src, None)
            .expect("complex flowchart should render");
        assert_valid_render(&result);
    }

    #[test]
    fn test_render_flowchart_lr() {
        let result = render("flowchart LR\n    X --> Y --> Z", None)
            .expect("LR flowchart should render");
        assert_valid_render(&result);
    }

    // ─── Class Diagram ─────────────────────────────────────

    #[test]
    fn test_render_class_diagram() {
        let result = render("classDiagram\n    Animal <|-- Dog", None)
            .expect("class diagram should render");
        assert_valid_render(&result);
    }

    // ─── State Diagram ─────────────────────────────────────

    #[test]
    fn test_render_state_diagram() {
        let result = render("stateDiagram-v2\n    [*] --> Idle", None)
            .expect("state diagram should render");
        assert_valid_render(&result);
    }

    // ─── ER Diagram ────────────────────────────────────────

    #[test]
    fn test_render_er_diagram() {
        let result = render("erDiagram\n    A ||--o{ B : has", None)
            .expect("ER diagram should render");
        assert_valid_render(&result);
    }

    // ─── Requirement Diagram ───────────────────────────────

    #[test]
    fn test_render_requirement_diagram() {
        let src = "requirementDiagram\n    requirement req1 {\n        id: 1\n        text: the text\n    }";
        let result = render(src, None);
        // requirement diagram may or may not be fully supported; just ensure no panic
        let _ = result;
    }

    // ─── Packet Diagram ────────────────────────────────────

    #[test]
    fn test_render_packet_diagram() {
        let result = render("packet\n    0-7 : src", None);
        // packet diagram may or may not be fully supported; just ensure no panic
        let _ = result;
    }

    // ─── Sequence Diagram ────────────────────────────────────

    #[test]
    fn test_render_sequence_diagram() {
        let src = "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Hello\n    B-->>A: Hi";
        let result = render(src, None)
            .expect("sequence diagram should render");
        assert_valid_render(&result);
        // Should have non-zero dimensions
        assert!(result.width > 0.0, "width should be positive");
        assert!(result.height > 0.0, "height should be positive");
    }

    #[test]
    fn test_render_sequence_with_theme() {
        let src = "sequenceDiagram\n    A->>B: Test";
        let result = render(src, Some("dark"))
            .expect("sequence diagram with dark theme should render");
        assert_valid_render(&result);
    }

    // ─── Error Handling ────────────────────────────────────

    #[test]
    fn test_render_invalid_input_returns_error() {
        let result = render("this is not valid mermaid at all $$$", None);
        assert!(result.is_err(), "invalid input should return an error");
    }

    // ─── Layer Structure ───────────────────────────────────

    #[test]
    fn test_render_layers_have_valid_kinds() {
        let result = render("flowchart TD\n    A --> B", None)
            .expect("should render");
        let valid_kinds = ["background", "subgraphs", "edges", "nodes", "labels", "title", "annotations"];
        for layer in &result.layers {
            assert!(
                valid_kinds.contains(&layer.kind.as_str()),
                "layer kind '{}' should be one of {:?}",
                layer.kind, valid_kinds
            );
        }
    }

    #[test]
    fn test_render_wit_draw_cmd_types_are_valid() {
        let result = render("flowchart TD\n    A --> B", None)
            .expect("should render");
        let valid_types = ["rect", "circle", "path", "text"];
        for layer in &result.layers {
            for cmd in &layer.commands {
                assert!(
                    valid_types.contains(&cmd.cmd_type.as_str()),
                    "cmd_type '{}' should be one of {:?}",
                    cmd.cmd_type, valid_types
                );
            }
        }
    }

    #[test]
    fn test_render_result_is_serializable() {
        let result = render("flowchart TD\n    A --> B", None)
            .expect("should render");
        // WitRenderResult derives Serialize — verify it can be serialized to JSON
        let json = serde_json::to_string(&result);
        assert!(json.is_ok(), "WitRenderResult should serialize to JSON");
        assert!(!json.unwrap().is_empty());
    }

    // ─── 命中区（v2 会话）──────────────────────────────────

    #[test]
    fn test_hit_regions_and_host_side_hit_test() {
        let src = "flowchart TD\n    A[Alpha] --> B[Beta]";
        let regions = hit_regions(src, None).expect("hit regions");
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].node_id.as_deref(), Some("A"));

        // 宿主侧 AABB：命中 A 的包围盒中心
        let hit = hit_test(&regions, regions[0].bounds_x + regions[0].bounds_w / 2.0,
                           regions[0].bounds_y + regions[0].bounds_h / 2.0, 0.0);
        assert_eq!(hit, Some(0));

        // 远处未命中
        assert_eq!(hit_test(&regions, -1000.0, -1000.0, 10.0), None);
    }
}
