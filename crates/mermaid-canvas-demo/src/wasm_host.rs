//! WASM host — 通过 wasmtime 加载 mermaid-canvas WASI Component 并调用渲染

use mermaid_canvas_wit::wit_types::*;

// wasmtime bindgen! 从 WIT 生成 host 端绑定
wasmtime::component::bindgen!({
    path: "../mermaid-canvas-wit/wit",
    world: "mermaid-canvas-viz",
});

use exports::mermaid_canvas::viz::diagram_renderer::{
    DrawCmd as BgDrawCmd, HitRegion as BgHitRegion,
    Layer as BgLayer, RenderResult as BgRenderResult,
};

/// WASM 组件加载或调用错误
#[derive(Debug)]
pub enum WasmHostError {
    Engine(String),
    ComponentLoad(String),
    Instantiate(String),
    Call(String),
}

impl std::fmt::Display for WasmHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WasmHostError::Engine(s) => write!(f, "Engine error: {}", s),
            WasmHostError::ComponentLoad(s) => write!(f, "Component load error: {}", s),
            WasmHostError::Instantiate(s) => write!(f, "Instantiate error: {}", s),
            WasmHostError::Call(s) => write!(f, "Call error: {}", s),
        }
    }
}

impl std::error::Error for WasmHostError {}

/// WASI 状态
struct WasiState {
    ctx: wasmtime_wasi::WasiCtx,
    table: wasmtime::component::ResourceTable,
}

impl wasmtime_wasi::WasiView for WasiState {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        wasmtime_wasi::WasiCtxView { ctx: &mut self.ctx, table: &mut self.table }
    }
}

/// WASM host — 管理 mermaid-canvas WASI Component 生命周期
pub struct WasmHost {
    store: wasmtime::Store<WasiState>,
    bindings: MermaidCanvasViz,
}

impl WasmHost {
    /// 从 .wasm 文件加载并实例化
    pub fn from_file(wasm_path: &str) -> Result<Self, WasmHostError> {
        let engine = wasmtime::Engine::new(
            wasmtime::Config::new().wasm_component_model(true),
        ).map_err(|e| WasmHostError::Engine(e.to_string()))?;

        let component = wasmtime::component::Component::from_file(
            &engine, wasm_path,
        ).map_err(|e| WasmHostError::ComponentLoad(e.to_string()))?;

        let mut linker = wasmtime::component::Linker::<WasiState>::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|e| WasmHostError::Instantiate(format!("WASI linker: {}", e)))?;

        let wasi_ctx = wasmtime_wasi::WasiCtx::builder()
            .inherit_stdio()
            .build();

        let mut store = wasmtime::Store::new(&engine, WasiState {
            ctx: wasi_ctx,
            table: wasmtime::component::ResourceTable::new(),
        });

        let bindings = MermaidCanvasViz::instantiate(&mut store, &component, &linker)
            .map_err(|e| WasmHostError::Instantiate(format!("Instantiate: {}", e)))?;

        Ok(Self { store, bindings })
    }

    /// 调用组件的 render 函数
    pub fn render(
        &mut self,
        source: &str,
        theme: Option<&str>,
    ) -> Result<WitRenderResult, WasmHostError> {
        let result = self.bindings
            .mermaid_canvas_viz_diagram_renderer()
            .call_render(&mut self.store, source, theme)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(e.to_string()))?
            .map_err(|e: String| WasmHostError::Call(format!("render: {}", e)))?;

        Ok(bg_to_wit_render_result(result))
    }

    /// 调用组件的 hit-test 函数
    pub fn hit_test(
        &mut self,
        result: &WitRenderResult,
        x: f64,
        y: f64,
        tolerance: f64,
    ) -> Result<Option<u32>, WasmHostError> {
        let bg_result = wit_to_bg_render_result(result);
        self.bindings
            .mermaid_canvas_viz_diagram_renderer()
            .call_hit_test(&mut self.store, &bg_result, x, y, tolerance)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(e.to_string()))
    }
}

// ─── bindgen 类型 → WitXxx ──────────────────────────────────

fn bg_to_wit_render_result(r: BgRenderResult) -> WitRenderResult {
    WitRenderResult {
        layers: r.layers.into_iter().map(bg_to_wit_layer).collect(),
        width: r.width,
        height: r.height,
    }
}

fn bg_to_wit_layer(l: BgLayer) -> WitLayer {
    WitLayer {
        kind: l.kind,
        dirty: l.dirty,
        z_index: l.z_index,
        commands: l.commands.into_iter().map(bg_to_wit_draw_cmd).collect(),
        hit_regions: l.hit_regions.into_iter().map(|r| WitHitRegion {
            index: r.index,
            bounds_x: r.bounds_x,
            bounds_y: r.bounds_y,
            bounds_w: r.bounds_w,
            bounds_h: r.bounds_h,
        }).collect(),
    }
}

fn bg_to_wit_draw_cmd(c: BgDrawCmd) -> WitDrawCmd {
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

// ─── WitXxx → bindgen 类型 ──────────────────────────────────

fn wit_to_bg_render_result(r: &WitRenderResult) -> BgRenderResult {
    BgRenderResult {
        layers: r.layers.iter().map(wit_to_bg_layer).collect(),
        width: r.width,
        height: r.height,
    }
}

fn wit_to_bg_layer(l: &WitLayer) -> BgLayer {
    BgLayer {
        kind: l.kind.clone(),
        dirty: l.dirty,
        z_index: l.z_index,
        commands: l.commands.iter().map(wit_to_bg_draw_cmd).collect(),
        hit_regions: l.hit_regions.iter().map(|r| BgHitRegion {
            index: r.index,
            bounds_x: r.bounds_x,
            bounds_y: r.bounds_y,
            bounds_w: r.bounds_w,
            bounds_h: r.bounds_h,
        }).collect(),
    }
}

fn wit_to_bg_draw_cmd(c: &WitDrawCmd) -> BgDrawCmd {
    BgDrawCmd {
        cmd_type: c.cmd_type.clone(),
        params: c.params.clone(),
        fill: c.fill.clone(),
        stroke: c.stroke.clone(),
        stroke_width: c.stroke_width,
        text_content: c.text_content.clone(),
        group_depth: c.group_depth,
    }
}
