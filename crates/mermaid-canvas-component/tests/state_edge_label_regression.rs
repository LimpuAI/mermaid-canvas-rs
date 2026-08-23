//! 状态图边标签几何回归 — 锁定「带环状态图的边标签锚点不得落入任何节点框」。
//!
//! 回归背景:ranking 的 longest-path 提升分支会把环成员秩反转
//! (Transition→Ready 回边把 Ready 推到 Transition 之下),导致前向边
//! Loading→Ready 跨 band 直线穿过 Transition 节点,边标签取线段中点
//! 落进节点框内部("mounted" 显示在 Transition 框里)。

use mermaid_canvas_component::theme::DefaultTheme;
use mermaid_canvas_component::{compute_layout, LayoutConfig};
use mermaid_canvas_core::parser::parse_mermaid;

#[test]
fn state_diagram_edge_labels_never_inside_nodes() {
    let src = "stateDiagram-v2\n\
               [*] --> Loading\n\
               Loading --> Ready : mounted\n\
               Ready --> Transition : interact\n\
               Transition --> Ready\n\
               Ready --> [*]";
    let ast = parse_mermaid(src).expect("parse state diagram");
    let layout = compute_layout(&ast, &DefaultTheme, &LayoutConfig::default());

    let violations: Vec<String> = layout
        .edges
        .iter()
        .filter_map(|e| {
            let anchor = e.label_anchor?;
            let text = e.label.as_ref().map(|l| l.text.clone())?;
            layout
                .nodes
                .iter()
                .find(|(_, n)| {
                    anchor.0 >= n.x
                        && anchor.0 <= n.x + n.width
                        && anchor.1 >= n.y
                        && anchor.1 <= n.y + n.height
                })
                .map(|(id, n)| {
                    format!("label '{text}' anchor ({:.1},{:.1}) inside node '{id}'", anchor.0, anchor.1)
                })
        })
        .collect();

    assert!(
        violations.is_empty(),
        "edge label anchors must not fall inside node rects: {violations:?}"
    );
}

#[test]
fn state_diagram_cycle_members_keep_source_order() {
    // 环打破后不得秩反转:Loading→Ready 是前向边,Ready 必须在 Transition 之上,
    // 否则 mounted 边跨 band 穿框(即使标签护栏挡住了标签,线仍穿节点)。
    let src = "stateDiagram-v2\n\
               [*] --> Loading\n\
               Loading --> Ready : mounted\n\
               Ready --> Transition : interact\n\
               Transition --> Ready\n\
               Ready --> [*]";
    let ast = parse_mermaid(src).expect("parse state diagram");
    let layout = compute_layout(&ast, &DefaultTheme, &LayoutConfig::default());

    let ready = layout.nodes.get("Ready").expect("Ready node");
    let transition = layout.nodes.get("Transition").expect("Transition node");
    assert!(
        ready.y + ready.height <= transition.y + 1.0,
        "Ready (y={:.1}) must sit above Transition (y={:.1}) — rank inversion regression",
        ready.y,
        transition.y
    );
}
