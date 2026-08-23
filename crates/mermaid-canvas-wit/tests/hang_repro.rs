//! 挂死复现探针(echodawn demo wedge 定位):
//! demo 侧首帧 advance 顺序 = resize(布局宽, 300) → render(t<1)(enter 相位)。
//! 本测试扫描宽度 × 图源,任何组合死循环即命中根因。
//! 运行:cargo test -p mermaid-canvas-wit --lib hang_repro -- --nocapture
//! (外壳 timeout 兜底;最后一个打印的组合即挂点)

use mermaid_canvas_wit::session::DiagramSession;

const PLAYGROUND_SRC: &str = "flowchart TD\n\
    Event[Input Event] --> Router{Router}\n\
    Router -->|pointer| HitTest[Hit Test]\n\
    Router -->|keyboard| Keymap[Keymap]\n\
    HitTest --> Dispatch[Capability Dispatch]\n\
    Keymap --> Dispatch\n\
    Dispatch --> Pipeline[Frame Pipeline]\n\
    Pipeline --> Render[Render Tree]";

const THEME_SRC: &str = "flowchart LR\n\
    A[primary] --> B[/secondary/\n\
    B --> C{accent}\n\
    C --> D[(info)]\n\
    D --> E((data))\n\
    E --> F[[special]]";

#[test]
fn hang_repro_width_sweep_enter_phase() {
    let sources: &[(&str, &str)] = &[
        ("playground", PLAYGROUND_SRC),
        ("theme", THEME_SRC),
        ("g-flowchart", "flowchart LR\n  Init[start] --> Layout[layout]\n  Layout --> Paint[paint]\n  Paint --> Present[present]"),
        ("g-class", "classDiagram\n  class EntityView {\n    <<trait>>\n    +render() AnyElement\n  }\n  class PluginInteractive {\n    <<trait>>\n    +plugin_hit(pos)\n  }\n  EntityView <|.. PluginInteractive"),
        ("g-state", "stateDiagram-v2\n  [*] --> Loading\n  Loading --> Ready : mounted\n  Ready --> Transition : interact\n  Transition --> Ready\n  Ready --> [*]"),
        ("g-er", "erDiagram\n  ENTITY ||--o{ ELEMENT : has\n  PROVIDER ||--o{ SESSION : creates\n  SESSION ||--|| FRAME : renders"),
        ("g-requirement", "requirementDiagram\n  requirement frame_loop {\n    id: 1\n    text: sweep route finalize paint\n  }\n  requirement zero_wasm {\n    id: 2\n    text: steady state zero wasm calls\n  }\n  frame_loop -> zero_wasm"),
        ("g-packet", "packet-beta\n0-15: \"Source Port\"\n16-31: \"Destination Port\"\n32-47: \"Length\"\n48-63: \"Checksum\""),
        ("g-sequence", "sequenceDiagram\n  participant App\n  participant Pipeline\n  participant Diagram\n  App->>Pipeline: advance(dt)\n  Pipeline->>Diagram: render(t)\n  Diagram-->>Pipeline: CanvasFrame\n  Pipeline->>Diagram: set_state(hover)"),
    ];
    // demo 卡片内宽量级:viewport800 - sidebar240 - 卡片padding ≈ 440..560
    for w in [300.0, 360.0, 400.0, 440.0, 460.0, 480.0, 500.0, 520.0, 540.0, 560.0, 600.0, 800.0] {
        for (name, src) in sources {
            eprintln!(">> resize({w:.0}, 300) + render(0.1) on {name}");
            let mut s = DiagramSession::new(src.to_string(), None);
            s.resize(w, 300.0);
            let r = s.render(0.1).expect("render ok");
            eprintln!("   ok: {}x{}", r.width, r.height);
            let _ = s.render(1.0).expect("steady ok");
        }
    }
    // 对照:不 resize 直接 render
    for (name, src) in sources {
        eprintln!(">> no-resize render(0.1) on {name}");
        let mut s = DiagramSession::new(src.to_string(), None);
        let _ = s.render(0.1).expect("ok");
        eprintln!("   ok");
    }
}
