//! mermaid-canvas-wit-wasm: WASI Component Model 导出层（v2 — resource session 协议）
//!
//! 使用 wit-bindgen 0.57 从 world.wit 生成 guest 绑定，
//! 将 mermaid-canvas-wit 的功能导出为标准 WASI Component。
//!
//! v2：`diagram` resource 有状态会话（constructor/update-source/resize/set-state/
//! set-theme/render(t)/hit-regions），取代 v1 的自由函数 render/hit-test；
//! draw-cmd 经 echodawn:canvas/draw 共享词汇表无损投影。

wit_bindgen::generate!({
    path: "../mermaid-canvas-wit/wit",
    world: "mermaid-canvas-viz",
    generate_all,
});

use mermaid_canvas_wit::session::DiagramSession;
use mermaid_canvas_wit::wit_types::*;

use exports::mermaid::viz::diagram_renderer::{
    DiagramOptions as BgDiagramOptions,
    DiagramTheme as BgDiagramTheme, HitRegion as BgHitRegion, InteractionState as BgInteractionState,
    Margin as BgMargin, RenderResult as BgRenderResult, Guest, GuestDiagram,
};
// echodawn:canvas/draw 被 diagram-renderer `use` — bindgen 将其类型生成在 crate 根
use echodawn::canvas::draw::{
    AnimDesc as BgAnimDesc, AnimProperty as BgAnimProperty, DrawCmd as BgDrawCmd,
    FontDesc as BgFontDesc, GradientStop as BgGradientStop, Keyframe as BgKeyframe,
    Layer as BgLayer, LinearGradient as BgLinearGradient, LoopMode as BgLoopMode, Paint as BgPaint,
};

// diagram-renderer 接口仅含 resource — 接口级 Guest 为空标记，
// resource 本体在 GuestDiagram（constructor + 六方法，单 trait）。

/// `diagram` resource 实现体 — 持有 mermaid-canvas-wit DiagramSession
///
/// trait 方法为 &self（bindgen 约定），会话可变性经 RefCell。
pub struct DiagramResource {
    session: std::cell::RefCell<DiagramSession>,
}

impl GuestDiagram for DiagramResource {
    fn new(source: String, opts: Option<BgDiagramOptions>) -> Self {
        let wit_opts = opts.map(bg_to_wit_options);
        // constructor 无错误通道：解析失败延迟到 render 报告（与 deneb 降级策略一致）
        Self {
            session: std::cell::RefCell::new(DiagramSession::new(source, wit_opts)),
        }
    }

    fn update_source(&self, source: String) -> Result<(), String> {
        self.session.borrow_mut().update_source(source)
    }

    fn resize(&self, width: f64, height: f64) {
        self.session.borrow_mut().resize(width, height);
    }

    fn set_state(&self, state: BgInteractionState) {
        self.session.borrow_mut().set_state(WitInteractionState {
            hovered: state.hovered,
            selected: state.selected,
        });
    }

    fn set_theme(&self, theme: BgDiagramTheme) {
        self.session.borrow_mut().set_theme(bg_to_wit_theme(theme));
    }

    fn render(&self, t: f64) -> Result<BgRenderResult, String> {
        let wit = self.session.borrow_mut().render(t)?;
        Ok(wit_render_result_to_bindgen(wit))
    }

    fn hit_regions(&self) -> Vec<BgHitRegion> {
        self.session
            .borrow_mut()
            .hit_regions()
            .into_iter()
            .map(wit_hit_region_to_bindgen)
            .collect()
    }
}

impl Guest for MermaidCanvasVizComponent {
    type Diagram = DiagramResource;
}

struct MermaidCanvasVizComponent;

// ─── bindgen 导出类型 → WitXxx ────────────────────────────────

fn bg_to_wit_options(o: BgDiagramOptions) -> WitDiagramOptions {
    WitDiagramOptions {
        width: o.width,
        theme: o.theme,
        animation: o.animation.map(|a| WitAnimationConfig {
            enter_duration_ms: a.enter_duration_ms,
            stagger_ms: a.stagger_ms,
            easing: a.easing,
            disable: a.disable,
        }),
    }
}

fn bg_margin_to_wit(m: BgMargin) -> WitMargin {
    WitMargin { top: m.top, right: m.right, bottom: m.bottom, left: m.left }
}

fn bg_to_wit_theme(t: BgDiagramTheme) -> WitDiagramTheme {
    WitDiagramTheme {
        background: t.background,
        foreground: t.foreground,
        edge_color: t.edge_color,
        edge_label_background: t.edge_label_background,
        node_colors: t.node_colors,
        node_stroke: t.node_stroke,
        title_color: t.title_color,
        font_family: t.font_family,
        base_font_size: t.base_font_size,
        title_font_size: t.title_font_size,
        margin: bg_margin_to_wit(t.margin),
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
    }
}

fn wit_draw_cmd_to_bindgen(c: WitDrawCmd) -> BgDrawCmd {
    BgDrawCmd {
        cmd_type: c.cmd_type,
        params: c.params,
        fill: c.fill.map(wit_paint_to_bindgen),
        stroke: c.stroke.map(wit_paint_to_bindgen),
        stroke_width: c.stroke_width,
        corner_radius: c.corner_radius,
        text_content: c.text_content,
        font: c.font.map(|f| BgFontDesc {
            family: f.family,
            weight: f.weight,
            italic: f.italic,
        }),
        group_depth: c.group_depth,
        anim: c.anim.map(wit_anim_to_bindgen),
    }
}

fn wit_paint_to_bindgen(p: WitPaint) -> BgPaint {
    match p {
        WitPaint::Solid(c) => BgPaint::Solid(c),
        WitPaint::Gradient(g) => BgPaint::Gradient(BgLinearGradient {
            x0: g.x0,
            y0: g.y0,
            x1: g.x1,
            y1: g.y1,
            stops: g.stops.into_iter().map(|s| BgGradientStop {
                pos: s.pos,
                color: s.color,
            }).collect(),
        }),
    }
}

fn wit_anim_to_bindgen(a: WitAnimDesc) -> BgAnimDesc {
    BgAnimDesc {
        property: match a.property {
            WitAnimProperty::Opacity => BgAnimProperty::Opacity,
        },
        keyframes: a.keyframes.into_iter().map(|k| BgKeyframe {
            t: k.t,
            value: k.value,
            easing: k.easing,
        }).collect(),
        duration_ms: a.duration_ms,
        delay_ms: a.delay_ms,
        loop_: match a.loop_mode {
            WitLoopMode::Once => BgLoopMode::Once,
            WitLoopMode::Loop => BgLoopMode::Loop,
            WitLoopMode::PingPong => BgLoopMode::PingPong,
        },
    }
}

fn wit_hit_region_to_bindgen(r: WitHitRegion) -> BgHitRegion {
    BgHitRegion {
        index: r.index,
        node_id: r.node_id,
        bounds_x: r.bounds_x,
        bounds_y: r.bounds_y,
        bounds_w: r.bounds_w,
        bounds_h: r.bounds_h,
    }
}

export!(MermaidCanvasVizComponent);
