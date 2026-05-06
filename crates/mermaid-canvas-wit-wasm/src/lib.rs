//! mermaid-canvas-wit-wasm: WASI Component Model 导出层
//!
//! 使用 wit-bindgen 0.57 从 world.wit 生成 guest 绑定，
//! 将 mermaid-canvas-wit 的功能导出为标准 WASI Component。

wit_bindgen::generate!({
    path: "../mermaid-canvas-wit/wit",
    world: "mermaid-canvas-viz",
    generate_all,
});

use mermaid_canvas_wit::wit_types::*;

use exports::mermaid_canvas::viz::diagram_parser::{
    Guest as DiagramParserGuest, DiagramAst as BgDiagramAst,
    DiagramNode as BgDiagramNode, DiagramEdge as BgDiagramEdge,
};
use exports::mermaid_canvas::viz::diagram_renderer::{
    Guest as DiagramRendererGuest, DrawCmd as BgDrawCmd, HitRegion as BgHitRegion,
    Layer as BgLayer, RenderResult as BgRenderResult,
};

struct MermaidCanvasComponent;

impl DiagramParserGuest for MermaidCanvasComponent {
    fn parse(source: String) -> Result<BgDiagramAst, String> {
        let ast = mermaid_canvas_core::parse_mermaid(&source)
            .map_err(|e| e.to_string())?;

        let kind = format!("{:?}", ast.kind).to_lowercase();
        let direction = format!("{:?}", ast.direction).to_lowercase();

        let nodes: Vec<BgDiagramNode> = ast
            .node_order
            .iter()
            .filter_map(|id| ast.nodes.get(id))
            .map(|n| BgDiagramNode {
                id: n.id.clone(),
                label: n.label.clone(),
                shape: format!("{:?}", n.shape).to_lowercase(),
                fill: None,
                stroke: None,
            })
            .collect();

        let edges: Vec<BgDiagramEdge> = ast
            .edges
            .into_iter()
            .map(|e| BgDiagramEdge {
                source: e.from,
                target: e.to,
                label: e.label,
                directed: e.directed,
            })
            .collect();

        Ok(BgDiagramAst {
            kind,
            direction,
            nodes,
            edges,
            title: ast.title,
        })
    }
}

impl DiagramRendererGuest for MermaidCanvasComponent {
    fn render(source: String, theme: Option<String>) -> Result<BgRenderResult, String> {
        let wit_result = mermaid_canvas_wit::render(&source, theme.as_deref())?;
        Ok(wit_render_result_to_bindgen(wit_result))
    }

    fn hit_test(render_data: BgRenderResult, x: f64, y: f64, tolerance: f64) -> Option<u32> {
        let wit = bindgen_render_result_to_wit(render_data);
        mermaid_canvas_wit::hit_test(&wit, x, y, tolerance)
    }
}

// ─── WitXxx → wit-bindgen 导出类型 ────────────────────────────

fn wit_render_result_to_bindgen(r: WitRenderResult) -> BgRenderResult {
    BgRenderResult {
        layers: r.layers.into_iter().map(wit_layer_to_bindgen).collect(),
        width: r.width,
        height: r.height,
    }
}

fn wit_layer_to_bindgen(l: WitLayer) -> BgLayer {
    BgLayer {
        kind: l.kind,
        dirty: l.dirty,
        z_index: l.z_index,
        commands: l.commands.into_iter().map(wit_draw_cmd_to_bindgen).collect(),
        hit_regions: l.hit_regions.into_iter().map(|r| BgHitRegion {
            index: r.index,
            bounds_x: r.bounds_x,
            bounds_y: r.bounds_y,
            bounds_w: r.bounds_w,
            bounds_h: r.bounds_h,
        }).collect(),
    }
}

fn wit_draw_cmd_to_bindgen(c: WitDrawCmd) -> BgDrawCmd {
    BgDrawCmd {
        cmd_type: c.cmd_type,
        params: c.params,
        fill: c.fill,
        stroke: c.stroke,
        stroke_width: c.stroke_width,
        text_content: c.text_content,
        group_depth: c.group_depth,
    }
}

// ─── wit-bindgen 导出类型 → WitXxx ────────────────────────────

fn bindgen_render_result_to_wit(r: BgRenderResult) -> WitRenderResult {
    WitRenderResult {
        layers: r.layers.into_iter().map(bindgen_layer_to_wit).collect(),
        width: r.width,
        height: r.height,
    }
}

fn bindgen_layer_to_wit(l: BgLayer) -> WitLayer {
    WitLayer {
        kind: l.kind,
        dirty: l.dirty,
        z_index: l.z_index,
        commands: l.commands.into_iter().map(bindgen_draw_cmd_to_wit).collect(),
        hit_regions: l.hit_regions.into_iter().map(|r| WitHitRegion {
            index: r.index,
            bounds_x: r.bounds_x,
            bounds_y: r.bounds_y,
            bounds_w: r.bounds_w,
            bounds_h: r.bounds_h,
        }).collect(),
    }
}

fn bindgen_draw_cmd_to_wit(c: BgDrawCmd) -> WitDrawCmd {
    WitDrawCmd {
        cmd_type: c.cmd_type,
        params: c.params,
        fill: c.fill,
        stroke: c.stroke,
        stroke_width: c.stroke_width,
        text_content: c.text_content,
        group_depth: c.group_depth,
    }
}

export!(MermaidCanvasComponent);
