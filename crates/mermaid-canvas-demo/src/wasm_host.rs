//! WASM host — 通过 wasmtime 加载 mermaid-canvas WASI Component 并调用渲染
//!
//! v2：组件导出 `diagram` resource 有状态会话。`render` 为单发兼容包装
//! （内部 constructor → render(1.0) → drop）；`render_session` 暴露完整会话
//! 供宿主集成演练。

use mermaid_canvas_wit::wit_types::*;

// wasmtime bindgen! 从 WIT 生成 host 端绑定
wasmtime::component::bindgen!({
    path: "../mermaid-canvas-wit/wit",
    world: "mermaid-canvas-viz",
});

use wasmtime::component::ResourceAny;
use wasmtime_wasi::WasiCtxView;

use exports::mermaid::viz::diagram_renderer::{
    DiagramOptions as BgDiagramOptions, HitRegion as BgHitRegion, RenderResult as BgRenderResult,
    Layer as BgLayer, DrawCmd as BgDrawCmd,
};
use echodawn::canvas::draw::Paint as BgPaint;

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
    fn ctx(&mut self) -> WasiCtxView<'_> {
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

    /// 创建 diagram 会话 resource（v2）— 返回句柄，方法经 diagram() 包装调用
    pub fn create_diagram(
        &mut self,
        source: &str,
        theme: Option<&str>,
    ) -> Result<ResourceAny, WasmHostError> {
        let opts = BgDiagramOptions {
            width: None,
            theme: theme.map(str::to_string),
            animation: None,
        };
        self.bindings
            .mermaid_viz_diagram_renderer()
            .diagram()
            .call_constructor(&mut self.store, source, Some(&opts))
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("diagram constructor: {}", e)))
    }

    /// 渲染（v2 兼容包装：单发语义 — 内部创建会话，渲染 t=1 稳态）
    pub fn render(
        &mut self,
        source: &str,
        theme: Option<&str>,
    ) -> Result<WitRenderResult, WasmHostError> {
        let diagram = self.create_diagram(source, theme)?;
        let result = self.session_render(diagram, 1.0);
        let _ = diagram.resource_drop(&mut self.store);
        result
    }

    /// 命中区（v2：会话方法；宿主侧 AABB 命中）
    pub fn hit_regions(
        &mut self,
        source: &str,
        theme: Option<&str>,
    ) -> Result<Vec<WitHitRegion>, WasmHostError> {
        let diagram = self.create_diagram(source, theme)?;
        let regions = self.session_hit_regions(diagram)?;
        let _ = diagram.resource_drop(&mut self.store);
        Ok(regions)
    }

    // ─── 会话驱动方法（resource 句柄跨调用保持 — 供宿主集成演练）───

    pub fn session_render(&mut self, diagram: ResourceAny, t: f64) -> Result<WitRenderResult, WasmHostError> {
        let result = self.bindings
            .mermaid_viz_diagram_renderer()
            .diagram()
            .call_render(&mut self.store, diagram, t)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("render: {}", e)))?
            .map_err(|e: String| WasmHostError::Call(format!("render: {}", e)))?;
        Ok(bg_to_wit_render_result(result))
    }

    pub fn session_update_source(&mut self, diagram: ResourceAny, source: &str) -> Result<(), WasmHostError> {
        self.bindings
            .mermaid_viz_diagram_renderer()
            .diagram()
            .call_update_source(&mut self.store, diagram, source)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("update-source: {}", e)))?
            .map_err(|e: String| WasmHostError::Call(format!("update-source: {}", e)))
    }

    pub fn session_resize(&mut self, diagram: ResourceAny, width: f64, height: f64) -> Result<(), WasmHostError> {
        self.bindings
            .mermaid_viz_diagram_renderer()
            .diagram()
            .call_resize(&mut self.store, diagram, width, height)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("resize: {}", e)))
    }

    pub fn session_set_state(&mut self, diagram: ResourceAny, state: &WitInteractionState) -> Result<(), WasmHostError> {
        let bg = exports::mermaid::viz::diagram_renderer::InteractionState {
            hovered: state.hovered,
            selected: state.selected.clone(),
        };
        self.bindings
            .mermaid_viz_diagram_renderer()
            .diagram()
            .call_set_state(&mut self.store, diagram, &bg)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("set-state: {}", e)))
    }

    pub fn session_set_theme(&mut self, diagram: ResourceAny, theme: &WitDiagramTheme) -> Result<(), WasmHostError> {
        let bg = exports::mermaid::viz::diagram_renderer::DiagramTheme {
            background: theme.background.clone(),
            foreground: theme.foreground.clone(),
            edge_color: theme.edge_color.clone(),
            edge_label_background: theme.edge_label_background.clone(),
            node_colors: theme.node_colors.clone(),
            node_stroke: theme.node_stroke.clone(),
            title_color: theme.title_color.clone(),
            font_family: theme.font_family.clone(),
            base_font_size: theme.base_font_size,
            title_font_size: theme.title_font_size,
            margin: exports::mermaid::viz::diagram_renderer::Margin {
                top: theme.margin.top,
                right: theme.margin.right,
                bottom: theme.margin.bottom,
                left: theme.margin.left,
            },
        };
        self.bindings
            .mermaid_viz_diagram_renderer()
            .diagram()
            .call_set_theme(&mut self.store, diagram, &bg)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("set-theme: {}", e)))
    }

    pub fn session_hit_regions(&mut self, diagram: ResourceAny) -> Result<Vec<WitHitRegion>, WasmHostError> {
        let regions = self.bindings
            .mermaid_viz_diagram_renderer()
            .diagram()
            .call_hit_regions(&mut self.store, diagram)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("hit-regions: {}", e)))?;
        Ok(regions.into_iter().map(bg_to_wit_hit_region).collect())
    }

    pub fn session_drop(&mut self, diagram: ResourceAny) -> Result<(), WasmHostError> {
        diagram
            .resource_drop(&mut self.store)
            .map_err(|e: wasmtime::Error| WasmHostError::Call(format!("drop: {}", e)))
    }
}

// ─── bindgen 生成类型 → WitXxx ──────────────────────────────

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
    }
}

fn bg_to_wit_paint(p: BgPaint) -> WitPaint {
    match p {
        BgPaint::Solid(c) => WitPaint::Solid(c),
        BgPaint::Gradient(g) => WitPaint::Gradient(WitLinearGradient {
            x0: g.x0,
            y0: g.y0,
            x1: g.x1,
            y1: g.y1,
            stops: g.stops.into_iter().map(|s| WitGradientStop {
                pos: s.pos,
                color: s.color,
            }).collect(),
        }),
    }
}

fn bg_to_wit_draw_cmd(c: BgDrawCmd) -> WitDrawCmd {
    WitDrawCmd {
        cmd_type: c.cmd_type,
        params: c.params,
        fill: c.fill.map(bg_to_wit_paint),
        stroke: c.stroke.map(bg_to_wit_paint),
        stroke_width: c.stroke_width,
        corner_radius: c.corner_radius,
        text_content: c.text_content,
        font: c.font.map(|f| WitFontDesc {
            family: f.family,
            weight: f.weight,
            italic: f.italic,
        }),
        group_depth: c.group_depth,
        anim: None,
    }
}

fn bg_to_wit_hit_region(r: BgHitRegion) -> WitHitRegion {
    WitHitRegion {
        index: r.index,
        node_id: r.node_id,
        bounds_x: r.bounds_x,
        bounds_y: r.bounds_y,
        bounds_w: r.bounds_w,
        bounds_h: r.bounds_h,
    }
}
