//! 有状态图会话 — WIT v2 `resource diagram` 的实现核心
//!
//! 会话持有源码/主题/交互态/宽度约束，宿主经 resource 方法驱动：
//! - `update-source` → 重解析 + 重布局 + 入场重播（t 从 0 重播）；解析失败返回 Err 且保留旧图
//! - `resize` → fit-to-width：宽度约束仅收缩（内容自适应尺寸是后验的；高度由内容派生，忽略）
//! - `set-state` → hover 提亮 / 选中 outline（即时生效）
//! - `set-theme` → 记录应用（6 色槽经 shape_slot），重布局（字体影响尺寸），不重播入场
//! - `render(t)` → Tier 1 语义相位：t=1 为精确稳态；t∈[0,1) 入场 stagger
//!   （节点 fade+grow、边/标签 fade，按指令序级联）；disable 时任意 t 渲染稳态
//! - `hit-regions()` → 节点 AABB + node-id（宿主侧命中，零 wasm 调用）
//!
//! Tier 2（D9）：协议携带 anim 通道，native 不附着 — 所有 anim-desc 恒 None。

use crate::convert::{
    layer_to_wit_layer, layout_to_hit_regions, record_to_wit_theme, wit_theme_to_record,
};
use crate::wit_types::*;
use mermaid_canvas_component::theme::ThemeRecord;
use mermaid_canvas_component::{
    builtin_theme_record, compute_layout, FlowchartRenderer, Layout, LayoutConfig, RecordTheme,
    SequenceRenderer,
};
use mermaid_canvas_core::{DiagramAst, DiagramKind};

/// 级联相位偏移总量上限（毫秒）— 与 deneb 入场编排语义对齐
const STAGGER_CAP_MS: f64 = 400.0;
/// 级联延迟占入场时长的最大比例（item 窗口恒为 0.6，保证 t=1 全部完成）
const DELAY_FRAC_CAP: f64 = 0.4;
/// 静态层（背景/子图）纯淡入相位窗上界
const STATIC_FADE_UNTIL: f64 = 0.3;
/// 选中 outline 线宽
const SELECTED_STROKE_WIDTH: f64 = 2.0;
/// hover 提亮强度（向白色收敛比例）
const HOVER_LIGHTEN: f64 = 0.18;

/// 入场编排配置（缺省与 deneb §9.4 语义对齐）
#[derive(Clone, Debug)]
struct AnimConfig {
    enter_duration_ms: f64,
    stagger_ms: f64,
    easing: String,
    disable: bool,
}

impl Default for AnimConfig {
    fn default() -> Self {
        Self {
            enter_duration_ms: 500.0,
            stagger_ms: 24.0,
            easing: "cubic-out".to_string(),
            disable: false,
        }
    }
}

impl AnimConfig {
    fn from_wit(cfg: &Option<WitAnimationConfig>) -> Self {
        match cfg {
            None => Self::default(),
            Some(c) => Self {
                enter_duration_ms: c.enter_duration_ms.map(|v| v as f64).unwrap_or(500.0),
                stagger_ms: c.stagger_ms.unwrap_or(24.0),
                easing: c.easing.clone().unwrap_or_else(|| "cubic-out".to_string()),
                disable: c.disable,
            },
        }
    }
}

fn eval_easing(name: &str, x: f64) -> f64 {
    match name {
        "linear" => x,
        "cubic-in" => x * x * x,
        "cubic-in-out" => {
            if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
        }
        _ => 1.0 - (1.0 - x).powi(3), // 缺省 cubic-out
    }
}

/// 布局 + 渲染主题的缩放快照（fit-to-width 应用后）
struct ScaledLayout {
    layout: Layout,
    theme: RecordTheme,
}

/// 图会话（WIT resource diagram 的实现体）
pub struct DiagramSession {
    ast: Result<DiagramAst, String>,
    theme_record: ThemeRecord,
    anim_cfg: AnimConfig,
    width_constraint: Option<f64>,
    state: WitInteractionState,

    scaled: Option<ScaledLayout>,
    steady: Option<WitRenderResult>,
    enter_done: bool,
    last_result: Option<WitRenderResult>,
}

impl DiagramSession {
    /// 创建会话（constructor）— 解析失败延迟到 render 报告
    /// （WIT constructor 无错误通道，与 deneb 降级策略一致）
    pub fn new(source: String, opts: Option<WitDiagramOptions>) -> Self {
        let opts = opts.unwrap_or_default();
        let theme_record = match opts.theme.as_deref() {
            Some(name) => builtin_theme_record(name).unwrap_or_default(),
            None => ThemeRecord::default(),
        };
        Self {
            ast: parse_to_result(&source),
            theme_record,
            anim_cfg: AnimConfig::from_wit(&opts.animation),
            width_constraint: opts.width.filter(|w| *w > 0.0),
            state: WitInteractionState::default(),
            scaled: None,
            steady: None,
            enter_done: false,
            last_result: None,
        }
    }

    /// 当前生效的主题记录
    pub fn theme(&self) -> WitDiagramTheme {
        record_to_wit_theme(self.theme_record.clone())
    }

    /// 更新源码 — 重解析 + 重布局 + 入场重播；解析失败保留旧图并返回 Err
    pub fn update_source(&mut self, source: String) -> Result<(), String> {
        let parsed = parse_to_result(&source);
        if let Err(e) = &parsed {
            return Err(e.clone());
        }
        self.ast = parsed;
        self.invalidate();
        self.enter_done = false;
        self.state = WitInteractionState::default();
        Ok(())
    }

    /// 宽度约束（fit-to-width，仅收缩；高度为内容后验派生，忽略）
    pub fn resize(&mut self, width: f64, height: f64) {
        let _ = height;
        let constraint = if width > 0.0 { Some(width) } else { None };
        if self.width_constraint == constraint {
            return;
        }
        self.width_constraint = constraint;
        self.invalidate();
    }

    /// 交互状态回注（hover 提亮 / 选中 outline，即时生效）
    pub fn set_state(&mut self, state: WitInteractionState) {
        self.state = state;
    }

    /// 主题记录应用 — 6 色槽经 shape_slot 消费；重布局（字体影响尺寸），不重播入场
    pub fn set_theme(&mut self, theme: WitDiagramTheme) {
        self.theme_record = wit_theme_to_record(theme);
        self.invalidate();
    }

    /// 渲染（t ∈ [0,1]；t=1 精确稳态；disable 时任意 t 渲染稳态）
    pub fn render(&mut self, t: f64) -> Result<WitRenderResult, String> {
        if let Err(e) = &self.ast {
            return Err(e.clone());
        }
        let mut tt = t.clamp(0.0, 1.0);
        if self.anim_cfg.disable {
            tt = 1.0;
        }
        self.ensure_steady()?;
        let steady = match self.steady.clone() {
            Some(s) => s,
            // ensure_steady Ok ⇒ 稳态已建立；此分支构造上不可达，仅消除残余 panic 面
            None => return Err("internal: steady state missing after ensure_steady".to_string()),
        };

        let mut result = if !self.enter_done && tt < 1.0 {
            self.apply_enter(steady, tt)
        } else {
            steady
        };
        if tt >= 1.0 {
            self.enter_done = true;
        }

        self.apply_interaction(&mut result);
        self.apply_dirty_diff(&mut result);
        self.last_result = Some(result.clone());
        Ok(result)
    }

    /// 命中区（节点 AABB + node-id；索引与 interaction-state 一致）
    ///
    /// 解析失败会话返回空表：WIT 协议方法无错误通道，诚实降级为「无命中区」；
    /// 宿主以 `render()` 的 Err 作为解析失败的权威信号。
    pub fn hit_regions(&mut self) -> Vec<WitHitRegion> {
        if self.ast.is_err() {
            return Vec::new();
        }
        self.ensure_scaled();
        let scaled = self.scaled.as_ref().expect("ensure_scaled 建立布局(ast 已 Ok)");
        layout_to_hit_regions(&scaled.layout)
    }

    /// 解析错误访问器（lib_mode/宿主侧区分「空命中区」与「源码无效」）
    pub fn parse_error(&self) -> Option<&str> {
        self.ast.as_ref().err().map(String::as_str)
    }

    // ─── 内部：布局与稳态 ────────────────────────────────────

    fn invalidate(&mut self) {
        self.scaled = None;
        self.steady = None;
    }

    fn ensure_scaled(&mut self) {
        if self.scaled.is_some() {
            return;
        }
        let ast = match &self.ast {
            Ok(ast) => ast,
            Err(_) => return,
        };
        let base_record = self.theme_record.clone();
        let theme = RecordTheme::new(base_record.clone());
        let config = LayoutConfig::default();
        let layout = compute_layout(ast, &theme, &config);
        let scale = match self.width_constraint {
            Some(w) if layout.width > w => w / layout.width,
            _ => 1.0,
        };
        let (layout, render_record) = if scale < 1.0 {
            (scale_layout(&layout, scale), base_record.with_scaled_fonts(scale))
        } else {
            (layout, base_record)
        };
        self.scaled = Some(ScaledLayout {
            layout,
            theme: RecordTheme::new(render_record),
        });
    }

    fn ensure_steady(&mut self) -> Result<(), String> {
        if self.steady.is_some() {
            return Ok(());
        }
        self.ensure_scaled();
        let scaled = match self.scaled.take() {
            Some(s) => s,
            None => {
                return Err(match &self.ast {
                    Err(e) => e.clone(),
                    Ok(_) => "布局不可用".to_string(),
                })
            }
        };
        let ast = match &self.ast {
            Ok(ast) => ast,
            Err(e) => return Err(e.clone()),
        };
        let output = match ast.kind {
            DiagramKind::Sequence => SequenceRenderer::render(&scaled.layout, &scaled.theme),
            _ => FlowchartRenderer::render(&scaled.layout, &scaled.theme),
        }
        .map_err(|e| e.to_string())?;
        let layers: Vec<WitLayer> = output.layers.all().iter().map(|l| layer_to_wit_layer(l.clone())).collect();
        self.steady = Some(WitRenderResult {
            layers,
            width: scaled.layout.width,
            height: scaled.layout.height,
        });
        self.scaled = Some(scaled);
        Ok(())
    }

    // ─── 内部：Tier 1 入场 ───────────────────────────────────

    /// 入场相位：节点 fade+grow / 边与标签 fade，按指令序级联；静态层 [0,0.3] 淡入
    fn apply_enter(&self, mut result: WitRenderResult, t: f64) -> WitRenderResult {
        for layer in &mut result.layers {
            match layer.kind.as_str() {
                "nodes" => {
                    for (i, cmd) in layer.commands.iter_mut().enumerate() {
                        let p = self.item_phase(t, i);
                        if p < 1.0 {
                            scale_cmd_geometry(cmd, grow_factor(p));
                            apply_cmd_alpha(cmd, p);
                        }
                    }
                }
                "edges" | "labels" => {
                    for (i, cmd) in layer.commands.iter_mut().enumerate() {
                        let p = self.item_phase(t, i);
                        if p < 1.0 {
                            apply_cmd_alpha(cmd, p);
                        }
                    }
                }
                "background" | "subgraphs" => {
                    let fade = (t / STATIC_FADE_UNTIL).clamp(0.0, 1.0);
                    if fade < 1.0 {
                        for cmd in &mut layer.commands {
                            apply_cmd_alpha(cmd, fade);
                        }
                    }
                }
                _ => {}
            }
        }
        result
    }

    /// 第 idx 项在相位 t 的入场进度（0..1，已缓动）
    fn item_phase(&self, t: f64, idx: usize) -> f64 {
        let total = self.anim_cfg.enter_duration_ms.max(1.0);
        let delay_ms = (idx as f64 * self.anim_cfg.stagger_ms).min(STAGGER_CAP_MS);
        let delay_frac = (delay_ms / total).min(DELAY_FRAC_CAP);
        let x = ((t - delay_frac) / (1.0 - DELAY_FRAC_CAP)).clamp(0.0, 1.0);
        eval_easing(&self.anim_cfg.easing, x)
    }

    // ─── 内部：交互态 ────────────────────────────────────────

    /// hover 提亮 / 选中 outline（Nodes 层指令序 = 命中区序：flowchart 1:1，
    /// sequence 参与者在前、激活框在后 — 区域索引天然只覆盖参与者）
    fn apply_interaction(&self, result: &mut WitRenderResult) {
        if self.state.hovered.is_none() && self.state.selected.is_empty() {
            return;
        }
        let Some(layer) = result.layers.iter_mut().find(|l| l.kind == "nodes") else {
            return;
        };
        let focus = self.theme_record.foreground.clone();
        if let Some(h) = self.state.hovered {
            if let Some(cmd) = layer.commands.get_mut(h as usize) {
                lighten_cmd_fill(cmd, HOVER_LIGHTEN);
            }
        }
        for &s in &self.state.selected {
            if let Some(cmd) = layer.commands.get_mut(s as usize) {
                cmd.stroke = Some(WitPaint::Solid(focus.clone()));
                cmd.stroke_width = Some(SELECTED_STROKE_WIDTH);
            }
        }
    }

    /// dirty = 与上次返回结果相比是否变化（宿主增量翻译依据）
    fn apply_dirty_diff(&self, result: &mut WitRenderResult) {
        let Some(last) = &self.last_result else {
            return;
        };
        for layer in &mut result.layers {
            let prev = last.layers.iter().find(|l| l.kind == layer.kind);
            layer.dirty = match prev {
                Some(p) => p.commands != layer.commands,
                None => true,
            };
        }
    }
}

fn parse_to_result(source: &str) -> Result<DiagramAst, String> {
    mermaid_canvas_core::parse_mermaid(source).map_err(|e| e.to_string())
}

/// grow 的几何因子：保持最小可见尺寸，避免 p→0 时零尺寸指令
fn grow_factor(p: f64) -> f64 {
    0.05 + 0.95 * p
}

// ─── 布局缩放（fit-to-width）────────────────────────────────

fn scale_layout(layout: &Layout, s: f64) -> Layout {
    let mut out = Layout {
        width: layout.width * s,
        height: layout.height * s,
        nodes: Default::default(),
        edges: Vec::with_capacity(layout.edges.len()),
        subgraphs: Vec::with_capacity(layout.subgraphs.len()),
    };
    for (id, nl) in &layout.nodes {
        let mut n = nl.clone();
        n.x *= s;
        n.y *= s;
        n.width *= s;
        n.height *= s;
        n.label.x *= s;
        n.label.y *= s;
        n.label.width *= s;
        n.label.height *= s;
        n.label.font_size *= s;
        n.bounds.x *= s;
        n.bounds.y *= s;
        n.bounds.width *= s;
        n.bounds.height *= s;
        out.nodes.insert(id.clone(), n);
    }
    for el in &layout.edges {
        let mut e = el.clone();
        e.points = e.points.iter().map(|&(x, y)| (x * s, y * s)).collect();
        if let Some(label) = &mut e.label {
            label.x *= s;
            label.y *= s;
            label.width *= s;
            label.height *= s;
            label.font_size *= s;
        }
        if let Some((x, y)) = &mut e.label_anchor {
            *x *= s;
            *y *= s;
        }
        out.edges.push(e);
    }
    for sg in &layout.subgraphs {
        let mut g = sg.clone();
        g.x *= s;
        g.y *= s;
        g.width *= s;
        g.height *= s;
        g.label.x *= s;
        g.label.y *= s;
        g.label.width *= s;
        g.label.height *= s;
        g.label.font_size *= s;
        out.subgraphs.push(g);
    }
    out
}

// ─── WIT 指令后处理（入场 / 交互）───────────────────────────

/// 解析 "#rgb" / "#rrggbb" → (r, g, b)
fn parse_hex(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

fn with_alpha(color: &str, alpha: f64) -> String {
    match parse_hex(color) {
        Some((r, g, b)) => format!("rgba({},{},{},{:.3})", r, g, b, alpha),
        None => color.to_string(),
    }
}

fn lighten(color: &str, amount: f64) -> String {
    match parse_hex(color) {
        Some((r, g, b)) => {
            let f = |c: u8| -> u8 { (c as f64 + (255.0 - c as f64) * amount).round() as u8 };
            format!("#{:02x}{:02x}{:02x}", f(r), f(g), f(b))
        }
        None => color.to_string(),
    }
}

fn paint_with_alpha(paint: &WitPaint, alpha: f64) -> WitPaint {
    match paint {
        WitPaint::Solid(c) => WitPaint::Solid(with_alpha(c, alpha)),
        other => other.clone(),
    }
}

fn apply_cmd_alpha(cmd: &mut WitDrawCmd, alpha: f64) {
    if let Some(fill) = cmd.fill.take() {
        cmd.fill = Some(paint_with_alpha(&fill, alpha));
    }
    if let Some(stroke) = cmd.stroke.take() {
        cmd.stroke = Some(paint_with_alpha(&stroke, alpha));
    }
}

fn lighten_cmd_fill(cmd: &mut WitDrawCmd, amount: f64) {
    if let Some(WitPaint::Solid(c)) = &cmd.fill {
        cmd.fill = Some(WitPaint::Solid(lighten(c, amount)));
    }
}

/// 围绕指令自身包围盒中心缩放几何（rect/circle/path；text 不缩放）
fn scale_cmd_geometry(cmd: &mut WitDrawCmd, s: f64) {
    match cmd.cmd_type.as_str() {
        "rect" if cmd.params.len() >= 4 => {
            let (x, y, w, h) = (cmd.params[0], cmd.params[1], cmd.params[2], cmd.params[3]);
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let (nw, nh) = (w * s, h * s);
            cmd.params[0] = cx - nw / 2.0;
            cmd.params[1] = cy - nh / 2.0;
            cmd.params[2] = nw;
            cmd.params[3] = nh;
            if let Some(r) = cmd.corner_radius {
                cmd.corner_radius = Some(r * s);
            }
        }
        "circle" if cmd.params.len() >= 3 => {
            cmd.params[2] *= s;
        }
        "path" => {
            scale_path_params(&mut cmd.params, s);
        }
        _ => {}
    }
}

/// 路径参数缩放：0=MoveTo 1=LineTo 2=Bezier 3=Quad 4=Arc 5=Close。
/// 坐标与半径（长度量）缩放；弧角与 ccw 标志保持不变。
fn scale_path_params(params: &mut [f64], s: f64) {
    // 先以原始坐标求包围盒中心
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut visit = |x: f64, y: f64| {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    };
    let mut i = 0;
    while i < params.len() {
        match params[i] as u32 {
            0 | 1 if i + 2 < params.len() => {
                visit(params[i + 1], params[i + 2]);
                i += 3;
            }
            2 if i + 6 < params.len() => {
                for k in [1, 3, 5] {
                    visit(params[i + k], params[i + k + 1]);
                }
                i += 7;
            }
            3 if i + 4 < params.len() => {
                visit(params[i + 1], params[i + 2]);
                visit(params[i + 3], params[i + 4]);
                i += 5;
            }
            4 if i + 6 < params.len() => {
                visit(params[i + 1], params[i + 2]);
                i += 7;
            }
            5 => { i += 1; }
            _ => { i += 1; }
        }
    }
    if !min_x.is_finite() {
        return;
    }
    let (cx, cy) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let scale_pt = |x: f64| cx + (x - cx) * s;
    let scale_py = |y: f64| cy + (y - cy) * s;
    let mut i = 0;
    while i < params.len() {
        match params[i] as u32 {
            0 | 1 if i + 2 < params.len() => {
                params[i + 1] = scale_pt(params[i + 1]);
                params[i + 2] = scale_py(params[i + 2]);
                i += 3;
            }
            2 if i + 6 < params.len() => {
                for k in [1, 3, 5] {
                    params[i + k] = scale_pt(params[i + k]);
                    params[i + k + 1] = scale_py(params[i + k + 1]);
                }
                i += 7;
            }
            3 if i + 4 < params.len() => {
                for k in [1, 3] {
                    params[i + k] = scale_pt(params[i + k]);
                    params[i + k + 1] = scale_py(params[i + k + 1]);
                }
                i += 5;
            }
            4 if i + 6 < params.len() => {
                params[i + 1] = scale_pt(params[i + 1]);
                params[i + 2] = scale_py(params[i + 2]);
                params[i + 3] *= s;
                i += 7;
            }
            5 => { i += 1; }
            _ => { i += 1; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOWCHART: &str = "flowchart TD\n    A[Start] -->|go| B{Choice?}\n    B -->|yes| C[(DB)]";

    fn session(source: &str) -> DiagramSession {
        DiagramSession::new(source.to_string(), None)
    }

    fn nodes_layer(r: &WitRenderResult) -> &WitLayer {
        r.layers.iter().find(|l| l.kind == "nodes").unwrap()
    }

    // ─── 会话生命周期 ────────────────────────────────────────

    #[test]
    fn test_construct_render_steady() {
        let mut s = session(FLOWCHART);
        let r = s.render(1.0).unwrap();
        assert!(r.width > 0.0 && r.height > 0.0);
        assert!(!nodes_layer(&r).commands.is_empty());
    }

    #[test]
    fn test_parse_error_deferred_to_render() {
        let mut s = session("this is not mermaid $$$");
        // constructor 无错误通道：render 报告
        let err = s.render(1.0).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_hit_regions_on_parse_error_returns_empty_not_panic() {
        // 协议回归(P1)：hit-regions 无错误通道，解析失败会话须降级为空表而非 panic/trap
        let mut s = session("this is not mermaid $$$");
        assert!(s.hit_regions().is_empty());
        assert!(s.parse_error().is_some());
    }

    #[test]
    fn test_lib_mode_hit_regions_returns_err_on_bad_source() {
        let err = crate::lib_mode::hit_regions("garbage $$$", None).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_update_source_replays_enter_and_updates_geometry() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        s.update_source("flowchart TD\n    A --> B".to_string()).unwrap();
        let early = s.render(0.1).unwrap();
        let steady = s.render(1.0).unwrap();
        // 重播入场：早期节点几何小于稳态
        let early_area: f64 = nodes_layer(&early).commands.iter().map(area_of).sum();
        let steady_area: f64 = nodes_layer(&steady).commands.iter().map(area_of).sum();
        assert!(early_area < steady_area, "early={} steady={}", early_area, steady_area);
        // 源码变化生效
        assert_eq!(s.hit_regions().len(), 2);
    }

    #[test]
    fn test_update_source_invalid_keeps_old_diagram() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        let before = s.hit_regions().len();
        assert!(s.update_source("garbage $$$".to_string()).is_err());
        assert_eq!(s.hit_regions().len(), before, "解析失败保留旧图");
        s.render(1.0).unwrap();
    }

    #[test]
    fn test_steady_repeats_are_stable_and_clean() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        let r1 = s.render(1.0).unwrap();
        let r2 = s.render(1.0).unwrap();
        assert_eq!(r1, r2);
        assert!(r1.layers.iter().all(|l| !l.dirty), "稳态重复请求 dirty 全 false");
    }

    // ─── 七种图表类型过 v2 会话 ──────────────────────────────

    #[test]
    fn test_all_seven_diagram_types_through_session() {
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
            let mut s = session(src);
            let r = s.render(1.0)
                .unwrap_or_else(|e| panic!("{}: render failed: {}", name, e));
            assert!(r.width > 0.0 && r.height > 0.0, "{}: positive size", name);
            assert!(
                r.layers.iter().any(|l| !l.commands.is_empty()),
                "{}: at least one non-empty layer",
                name,
            );
            assert!(!s.hit_regions().is_empty(), "{}: node hit regions", name);
        }
    }

    // ─── 命中区 ──────────────────────────────────────────────

    #[test]
    fn test_hit_regions_carry_node_ids_and_match_treatment_order() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        let regions = s.hit_regions();
        assert_eq!(regions.len(), 3);
        assert_eq!(regions[0].node_id.as_deref(), Some("A"));
        assert_eq!(regions[1].node_id.as_deref(), Some("B"));
        assert_eq!(regions[2].node_id.as_deref(), Some("C"));
        for r in &regions {
            assert!(r.bounds_w > 0.0 && r.bounds_h > 0.0);
        }
    }

    #[test]
    fn test_hit_regions_available_before_render() {
        let mut s = session(FLOWCHART);
        let regions = s.hit_regions();
        assert_eq!(regions.len(), 3);
    }

    #[test]
    fn test_sequence_hit_regions_exclude_internal_activation_nodes() {
        let src = "sequenceDiagram\n    participant A\n    participant B\n    activate A\n    A->>B: m\n    deactivate A";
        let mut s = session(src);
        s.render(1.0).unwrap();
        let regions = s.hit_regions();
        assert_eq!(regions.len(), 2, "仅参与者，__act_ 内部节点排除");
        assert!(regions.iter().all(|r| !r.node_id.as_deref().unwrap_or("").starts_with("__act_")));
    }

    // ─── Tier 1 入场 ─────────────────────────────────────────

    /// dirty = 与上次返回的帧相比是否变化；跨会话/跨帧比较需忽略 dirty
    fn canonical(r: &WitRenderResult) -> WitRenderResult {
        let mut c = r.clone();
        for l in &mut c.layers {
            l.dirty = false;
        }
        c
    }

    fn solid_fill(cmd: &WitDrawCmd) -> String {
        match &cmd.fill {
            Some(WitPaint::Solid(c)) => c.clone(),
            other => panic!("expected solid fill, got {:?}", other),
        }
    }

    fn area_of(cmd: &WitDrawCmd) -> f64 {
        match cmd.cmd_type.as_str() {
            "rect" => cmd.params[2] * cmd.params[3],
            "circle" => std::f64::consts::PI * cmd.params[2] * cmd.params[2],
            _ => 0.0,
        }
    }

    #[test]
    fn test_enter_grows_nodes_monotonically() {
        let mut s = session(FLOWCHART);
        let a = area_sum(&s.render(0.1).unwrap());
        let b = area_sum(&s.render(0.5).unwrap());
        let c = area_sum(&s.render(1.0).unwrap());
        assert!(a < b && b <= c, "a={} b={} c={}", a, b, c);
        assert!(a > 0.0, "部分节点已开始生长");
    }

    fn area_sum(r: &WitRenderResult) -> f64 {
        nodes_layer(r).commands.iter().map(area_of).sum()
    }

    #[test]
    fn test_enter_fades_edges_and_labels() {
        let mut s = session(FLOWCHART);
        let early = s.render(0.05).unwrap();
        let edges = early.layers.iter().find(|l| l.kind == "edges").unwrap();
        assert!(
            edges.commands.iter().all(|c| is_translucent(c)),
            "入场早期边为半透明",
        );
        let labels = early.layers.iter().find(|l| l.kind == "labels").unwrap();
        assert!(labels.commands.iter().all(|c| is_translucent(c)));
    }

    fn is_translucent(cmd: &WitDrawCmd) -> bool {
        let solid = |p: &Option<WitPaint>| match p {
            Some(WitPaint::Solid(c)) => c.starts_with("rgba"),
            _ => false,
        };
        solid(&cmd.fill) || solid(&cmd.stroke)
    }

    #[test]
    fn test_t1_is_exact_steady_state() {
        let mut s = session(FLOWCHART);
        let via_t1 = s.render(1.0).unwrap();
        // 与禁用动画（任意 t 稳态）逐字段一致
        let opts = WitDiagramOptions {
            animation: Some(WitAnimationConfig { disable: true, ..Default::default() }),
            ..Default::default()
        };
        let mut s2 = DiagramSession::new(FLOWCHART.to_string(), Some(opts));
        let disabled = s2.render(0.0).unwrap();
        assert_eq!(via_t1, disabled);
    }

    #[test]
    fn test_disable_animation_renders_steady_at_any_t() {
        let opts = WitDiagramOptions {
            animation: Some(WitAnimationConfig { disable: true, ..Default::default() }),
            ..Default::default()
        };
        let mut s = DiagramSession::new(FLOWCHART.to_string(), Some(opts));
        let r0 = s.render(0.0).unwrap();
        let r1 = s.render(1.0).unwrap();
        assert_eq!(canonical(&r0), canonical(&r1));
        assert!(!nodes_layer(&r0).commands.is_empty());
        // D9：native 不附着 Tier 2 — 帧内零 anim-desc
        assert!(!r0.layers.iter().any(|l| l.commands.iter().any(|c| c.anim.is_some())));
    }

    #[test]
    fn test_no_anim_desc_ever_attached() {
        let mut s = session(FLOWCHART);
        for t in [0.0, 0.3, 0.7, 1.0] {
            let r = s.render(t).unwrap();
            assert!(
                !r.layers.iter().any(|l| l.commands.iter().any(|c| c.anim.is_some())),
                "t={} 无 anim-desc",
                t,
            );
        }
    }

    #[test]
    fn test_enter_done_stops_stagger_processing() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        // 入场完成后 set_state 触发重渲染：节点几何保持稳态（不被 grow 影响）
        s.set_state(WitInteractionState { hovered: None, selected: vec![0] });
        let r = s.render(0.3).unwrap();
        let steady_area: f64 = {
            let mut s2 = session(FLOWCHART);
            area_sum(&s2.render(1.0).unwrap())
        };
        let now = area_sum(&r);
        assert!((now - steady_area).abs() < 1e-6, "now={} steady={}", now, steady_area);
    }

    // ─── 交互态 ──────────────────────────────────────────────

    #[test]
    fn test_hover_lightens_node_fill() {
        let mut s = session(FLOWCHART);
        let base = s.render(1.0).unwrap();
        let base_hovered = solid_fill(&nodes_layer(&base).commands[0]);
        let base_other = solid_fill(&nodes_layer(&base).commands[1]);
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        let r = s.render(1.0).unwrap();
        let hovered = solid_fill(&nodes_layer(&r).commands[0]);
        assert_ne!(hovered, base_hovered);
        assert!(hovered.starts_with('#'), "提亮保持 hex 形式: {}", hovered);
        // 未 hover 节点不变
        assert_eq!(solid_fill(&nodes_layer(&r).commands[1]), base_other);
    }

    #[test]
    fn test_selected_node_gets_outline() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: None, selected: vec![1] });
        let r = s.render(1.0).unwrap();
        let layer = nodes_layer(&r);
        assert_eq!(
            layer.commands[1].stroke,
            Some(WitPaint::Solid("#333333".to_string())),
            "选中 outline = foreground",
        );
        assert_eq!(layer.commands[1].stroke_width, Some(SELECTED_STROKE_WIDTH));
        // 非选中节点无 outline 强化
        assert!(layer.commands[0].stroke.is_some()); // 原生节点自带描边
        assert_eq!(layer.commands[0].stroke_width, None);
    }

    // ─── 主题 ────────────────────────────────────────────────

    #[test]
    fn test_constructor_theme_name_selects_record() {
        let opts = WitDiagramOptions {
            theme: Some("dark".to_string()),
            ..Default::default()
        };
        let mut s = DiagramSession::new(FLOWCHART.to_string(), Some(opts));
        let r = s.render(1.0).unwrap();
        let bg = r.layers.iter().find(|l| l.kind == "background").unwrap();
        assert!(bg.commands.iter().any(|c| c.fill == Some(WitPaint::Solid("#1e1e2e".to_string()))));
    }

    #[test]
    fn test_unknown_theme_name_falls_back_to_default() {
        let opts = WitDiagramOptions {
            theme: Some("nope".to_string()),
            ..Default::default()
        };
        let mut s = DiagramSession::new(FLOWCHART.to_string(), Some(opts));
        let r = s.render(1.0).unwrap();
        let bg = r.layers.iter().find(|l| l.kind == "background").unwrap();
        assert!(bg.commands.iter().any(|c| c.fill == Some(WitPaint::Solid("#ffffff".to_string()))));
    }

    #[test]
    fn test_set_theme_applies_record_runtime() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        let theme = WitDiagramTheme {
            background: "#000102".to_string(),
            foreground: "#eefeff".to_string(),
            edge_color: "#445566".to_string(),
            edge_label_background: "#000102".to_string(),
            node_colors: vec!["#112233".to_string(); 6],
            node_stroke: "#99aabb".to_string(),
            title_color: "#ffffff".to_string(),
            font_family: "Mono".to_string(),
            base_font_size: 14.0,
            title_font_size: 18.0,
            margin: WitMargin { top: 20.0, right: 20.0, bottom: 20.0, left: 20.0 },
        };
        s.set_theme(theme);
        let r = s.render(1.0).unwrap();
        let bg = r.layers.iter().find(|l| l.kind == "background").unwrap();
        assert!(bg.commands.iter().any(|c| c.fill == Some(WitPaint::Solid("#000102".to_string()))));
        let nodes = nodes_layer(&r);
        assert!(nodes.commands.iter().all(|c| c.fill == Some(WitPaint::Solid("#112233".to_string()))));
        // theme() 回读
        assert_eq!(s.theme().background, "#000102");
    }

    #[test]
    fn test_theme_change_does_not_replay_enter() {
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        let mut theme = record_to_wit_theme(ThemeRecord::default());
        theme.background = "#abcdef".to_string();
        s.set_theme(theme);
        let r = s.render(0.2).unwrap();
        // 不重播：节点保持稳态几何
        let steady_area = {
            let mut s2 = session(FLOWCHART);
            area_sum(&s2.render(1.0).unwrap())
        };
        assert!((area_sum(&r) - steady_area).abs() < 1e-6);
    }

    // ─── resize（fit-to-width）───────────────────────────────

    #[test]
    fn test_resize_shrinks_to_width_constraint() {
        let mut s = session(FLOWCHART);
        let natural = s.render(1.0).unwrap();
        assert!(natural.width > 100.0);
        let target = natural.width / 2.0;
        s.resize(target, 0.0);
        let after = s.render(1.0).unwrap();
        assert!(
            (after.width - target).abs() < 1.0,
            "after={} target={}",
            after.width,
            target,
        );
        assert!(after.height < natural.height, "高度随比例收缩");
        // 命中区同步收缩
        let regions = s.hit_regions();
        assert!(regions.iter().all(|r| r.bounds_x + r.bounds_w <= after.width + 1.0));
    }

    #[test]
    fn test_resize_wider_than_natural_keeps_natural_size() {
        let mut s = session(FLOWCHART);
        let natural = s.render(1.0).unwrap();
        s.resize(natural.width * 4.0, 0.0);
        let after = s.render(1.0).unwrap();
        assert_eq!(after.width, natural.width, "fit-to-width 仅收缩不放大");
    }

    #[test]
    fn test_resize_same_width_is_noop() {
        let mut s = session(FLOWCHART);
        let r1 = s.render(1.0).unwrap();
        s.resize(1_000_000.0, 0.0);
        let r2 = s.render(1.0).unwrap();
        assert_eq!(canonical(&r1), canonical(&r2));
    }

    #[test]
    fn test_constructor_width_option_applies() {
        let natural = {
            let mut s = session(FLOWCHART);
            s.render(1.0).unwrap().width
        };
        let opts = WitDiagramOptions {
            width: Some(natural / 3.0),
            ..Default::default()
        };
        let mut s = DiagramSession::new(FLOWCHART.to_string(), Some(opts));
        let r = s.render(1.0).unwrap();
        assert!((r.width - natural / 3.0).abs() < 1.0);
    }

    // ─── 无损投影（corner-radius / font / paint）─────────────

    #[test]
    fn test_roundrect_corner_radius_survives_abi() {
        // B{Choice?} → Diamond(path)；A[Start] → Rectangle；加入 stadium 节点验证圆角
        let src = "flowchart TD\n    A([Stadium node]) --> B[Plain]";
        let mut s = session(src);
        let r = s.render(1.0).unwrap();
        let rect = nodes_layer(&r).commands.iter().find(|c| c.cmd_type == "rect").unwrap();
        assert_eq!(rect.corner_radius, Some(8.0), "stadium 圆角无损过 ABI");
    }

    #[test]
    fn test_text_commands_carry_font_and_anchor() {
        let mut s = session(FLOWCHART);
        let r = s.render(1.0).unwrap();
        let labels = r.layers.iter().find(|l| l.kind == "labels").unwrap();
        let text = labels.commands.iter().find(|c| c.cmd_type == "text").unwrap();
        let font = text.font.as_ref().expect("text carries font-desc");
        assert_eq!(font.family.as_deref(), Some("sans-serif"));
        assert_eq!(font.weight, None);
        // 节点标签 anchor=Middle(1) baseline=Middle(1)
        assert_eq!(text.params[3], 1.0);
        assert_eq!(text.params[4], 1.0);
    }

    // ─── 辅助函数 ────────────────────────────────────────────

    #[test]
    fn test_hex_parsing_helpers() {
        assert_eq!(parse_hex("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_hex("#000"), Some((0, 0, 0)));
        assert_eq!(parse_hex("rgba(1,2,3,0.5)"), None);
        assert_eq!(with_alpha("#ff0000", 0.5), "rgba(255,0,0,0.500)");
        assert_eq!(lighten("#000000", 0.5), "#808080");
        assert_eq!(lighten("not-a-color", 0.5), "not-a-color");
    }

    #[test]
    fn test_eval_easing_variants() {
        assert_eq!(eval_easing("linear", 0.25), 0.25);
        assert!((eval_easing("cubic-out", 0.5) - 0.875).abs() < 1e-9);
        assert!((eval_easing("cubic-in", 0.5) - 0.125).abs() < 1e-9);
        assert!((eval_easing("cubic-in-out", 0.5) - 0.5).abs() < 1e-9);
        assert!((eval_easing("unknown", 0.5) - 0.875).abs() < 1e-9, "缺省 cubic-out");
    }
}
