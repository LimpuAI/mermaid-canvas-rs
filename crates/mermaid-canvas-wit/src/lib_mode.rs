//! 库调用模式 API

use super::wit_types::*;
use super::convert::*;

/// 渲染 Mermaid 源码为 Canvas 2D 指令
///
/// `theme` 可选值: `"default"`, `"dark"`, `"forest"`, `"nordic"`, `"cappuccino"`,
/// `None` 等同 `"default"`
pub fn render(source: &str, theme: Option<&str>) -> Result<WitRenderResult, String> {
    // 1. core: 解析 Mermaid 源码 → DiagramAst
    let ast = mermaid_canvas_core::parse_mermaid(source)
        .map_err(|e| e.to_string())?;

    // 2. 选择主题并分发（泛型需要编译期确定具体类型）
    let theme_name = theme.unwrap_or("default");
    let config = mermaid_canvas_component::LayoutConfig::default();

    match theme_name {
        "dark" => render_with_theme(&ast, &mermaid_canvas_component::DarkTheme, &config),
        "forest" => render_with_theme(&ast, &mermaid_canvas_component::ForestTheme, &config),
        "nordic" => render_with_theme(&ast, &mermaid_canvas_component::NordicTheme, &config),
        "cappuccino" => render_with_theme(&ast, &mermaid_canvas_component::CappuccinoTheme, &config),
        _ => render_with_theme(&ast, &mermaid_canvas_component::DefaultTheme, &config),
    }
}

/// 带具体主题类型的渲染
fn render_with_theme<T: mermaid_canvas_component::Theme>(
    ast: &mermaid_canvas_core::DiagramAst,
    theme: &T,
    config: &mermaid_canvas_component::LayoutConfig,
) -> Result<WitRenderResult, String> {
    let layout = mermaid_canvas_component::compute_layout(ast, theme, config);
    let width = layout.width;
    let height = layout.height;

    let output = match ast.kind {
        mermaid_canvas_core::DiagramKind::Sequence => {
            mermaid_canvas_component::SequenceRenderer::render(&layout, theme)
                .map_err(|e| e.to_string())?
        }
        _ => {
            mermaid_canvas_component::FlowchartRenderer::render(&layout, theme)
                .map_err(|e| e.to_string())?
        }
    };

    Ok(diagram_output_to_wit_render_result(output, width, height))
}

/// DiagramOutput 转换为 WitRenderResult
fn diagram_output_to_wit_render_result(
    output: mermaid_canvas_component::DiagramOutput,
    width: f64,
    height: f64,
) -> WitRenderResult {
    let layers: Vec<WitLayer> = output.layers.all()
        .iter()
        .map(|layer| layer_to_wit_layer(layer.clone()))
        .collect();

    WitRenderResult { layers, width, height }
}

/// 命中测试
pub fn hit_test(result: &WitRenderResult, x: f64, y: f64, tolerance: f64) -> Option<u32> {
    for layer in &result.layers {
        for region in &layer.hit_regions {
            let bounds = mermaid_canvas_core::BoundingBox::new(
                region.bounds_x, region.bounds_y, region.bounds_w, region.bounds_h,
            );
            if bounds.expand(tolerance).contains(x, y) {
                return Some(region.index);
            }
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
        let valid_types = ["rect", "circle", "path", "text", "group"];
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

    #[test]
    fn test_hit_test_empty_regions() {
        let result = render("flowchart TD\n    A --> B", None)
            .expect("should render");
        // No hit regions are populated by default
        let hit = hit_test(&result, 0.0, 0.0, 10.0);
        assert!(hit.is_none(), "no hit regions → should return None");
    }
}
