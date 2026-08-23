//! WASM v2 会话端到端集成测试 — 真实构件经 wasmtime 驱动完整生命周期
//!
//! 构件缺位时跳过（skip-if-missing）：
//! ```bash
//! cargo build -p mermaid-canvas-wit-wasm --target wasm32-wasip2 --release
//! cargo test -p mermaid-canvas-demo --test wasm_session
//! ```

use mermaid_canvas_demo::wasm_host::WasmHost;
use mermaid_canvas_wit::wit_types::*;

const WASM_PATH: &str = "../target/wasm32-wasip2/release/mermaid_canvas_wit_wasm.wasm";

const FLOWCHART: &str = "flowchart TD\n    A[Start] -->|go| B{Choice?}\n    B -->|yes| C[(DB)]";

fn host() -> Option<WasmHost> {
    match WasmHost::from_file(WASM_PATH) {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("skipping wasm session test (component unavailable at {}): {}", WASM_PATH, e);
            None
        }
    }
}

#[test]
fn wasm_session_render_steady_matches_native_path() {
    let Some(mut host) = host() else { return };

    let wasm_result = host.render(FLOWCHART, Some("dark")).expect("wasm render");
    let native_result = mermaid_canvas_wit::render(FLOWCHART, Some("dark")).expect("native render");

    // wasm 与 native 双路径产出一致（同一会话逻辑）
    assert_eq!(wasm_result.width, native_result.width);
    assert_eq!(wasm_result.height, native_result.height);
    assert_eq!(wasm_result.layers.len(), native_result.layers.len());
    assert_eq!(wasm_result, native_result);
}

#[test]
fn wasm_session_hit_regions_carry_node_ids() {
    let Some(mut host) = host() else { return };

    let regions = host.hit_regions(FLOWCHART, None).expect("wasm hit-regions");
    assert_eq!(regions.len(), 3);
    assert_eq!(regions[0].node_id.as_deref(), Some("A"));
    assert_eq!(regions[1].node_id.as_deref(), Some("B"));
    assert_eq!(regions[2].node_id.as_deref(), Some("C"));
}

#[test]
fn wasm_render_invalid_source_reports_error() {
    let Some(mut host) = host() else { return };
    let err = host.render("not mermaid at all $$$", None).expect_err("parse error surfaces");
    assert!(!err.to_string().is_empty());
}

#[test]
fn wasm_full_session_lifecycle_across_abi() {
    use mermaid_canvas_wit::wit_types::WitPaint;

    let Some(mut host) = host() else { return };

    // constructor
    let diagram = host.create_diagram(FLOWCHART, Some("dark")).expect("constructor");

    // render(t) 入场相位：早期节点为半透明 rgba
    let early = host.session_render(diagram, 0.1).expect("render(0.1)");
    let nodes = early.layers.iter().find(|l| l.kind == "nodes").unwrap();
    assert!(
        nodes.commands.iter().all(|c| match &c.fill {
            Some(WitPaint::Solid(col)) => col.starts_with("rgba"),
            _ => false,
        }),
        "入场早期节点半透明: {:?}",
        nodes.commands[0].fill,
    );

    // update-source：节点数变化
    host.session_update_source(diagram, "flowchart TD\n    A --> B").expect("update-source");
    let steady = host.session_render(diagram, 1.0).expect("render(1.0)");
    let _ = steady;

    // set-state：hover 提亮（rgba 不出现 — hex 提亮），选中 outline 强化
    host.session_set_state(
        diagram,
        &WitInteractionState { hovered: Some(0), selected: vec![1] },
    )
    .expect("set-state");
    let r = host.session_render(diagram, 1.0).expect("render after set-state");
    let nodes = r.layers.iter().find(|l| l.kind == "nodes").unwrap();
    assert_eq!(nodes.commands[1].stroke_width, Some(2.0), "选中 outline 线宽过 ABI");

    // set-theme：背景色切换
    host.session_set_theme(
        diagram,
        &WitDiagramTheme {
            background: "#0a0b0c".to_string(),
            foreground: "#ffffff".to_string(),
            edge_color: "#445566".to_string(),
            edge_label_background: "#0a0b0c".to_string(),
            node_colors: vec!["#123456".to_string(); 6],
            node_stroke: "#abcdef".to_string(),
            title_color: "#ffffff".to_string(),
            font_family: "Mono".to_string(),
            base_font_size: 14.0,
            title_font_size: 18.0,
            margin: WitMargin { top: 20.0, right: 20.0, bottom: 20.0, left: 20.0 },
        },
    )
    .expect("set-theme");
    let r = host.session_render(diagram, 1.0).expect("render after set-theme");
    let bg = r.layers.iter().find(|l| l.kind == "background").unwrap();
    assert!(bg.commands.iter().any(|c| c.fill == Some(WitPaint::Solid("#0a0b0c".to_string()))));

    // resize：fit-to-width 收缩
    let natural_width = r.width;
    host.session_resize(diagram, natural_width / 2.0, 0.0).expect("resize");
    let r = host.session_render(diagram, 1.0).expect("render after resize");
    assert!((r.width - natural_width / 2.0).abs() < 1.0, "wasm resize: {} → {}", natural_width, r.width);

    // hit-regions：节点 id 过 ABI
    let regions = host.session_hit_regions(diagram).expect("hit-regions");
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].node_id.as_deref(), Some("A"));

    host.session_drop(diagram).expect("session drop");
}

#[test]
fn wasm_all_seven_diagram_types_render() {
    let Some(mut host) = host() else { return };
    let sources = [
        ("flowchart", FLOWCHART),
        ("class", "classDiagram\n    Animal <|-- Dog"),
        ("state", "stateDiagram-v2\n    [*] --> Idle"),
        ("er", "erDiagram\n    A ||--o{ B : has"),
        (
            "requirement",
            "requirementDiagram\n    requirement req1 {\n        id: 1\n        text: the text\n    }",
        ),
        ("packet", "packet\n    0-7 : src"),
        (
            "sequence",
            "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Hello\n    B-->>A: Hi",
        ),
    ];
    for (name, src) in sources {
        let r = host.render(src, None)
            .unwrap_or_else(|e| panic!("{}: wasm render failed: {}", name, e));
        assert!(r.width > 0.0 && r.height > 0.0, "{}: positive size", name);
        assert!(
            r.layers.iter().any(|l| !l.commands.is_empty()),
            "{}: non-empty layers",
            name,
        );
    }
}
