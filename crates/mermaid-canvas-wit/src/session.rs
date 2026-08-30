//! 有状态图会话 — WIT v2 `resource diagram` 的实现核心
//!
//! 会话持有源码/主题/交互态/宽度约束，宿主经 resource 方法驱动：
//! - `update-source` → 重解析 + 重布局 + 入场重播（t 从 0 重播）；解析失败返回 Err 且保留旧图
//! - `resize` → fit-to-width：宽度约束仅收缩（内容自适应尺寸是后验的；高度由内容派生，忽略）
//! - `set-state` → 关联聚焦（hover 1-hop：相关保持 / 其余 dim 过渡）+ 相连边
//!   脉冲光点（Tier 2 路径跟随）+ hover 提亮 / 选中 outline
//! - `set-theme` → 记录应用（6 色槽经 shape_slot），重布局（字体影响尺寸），不重播入场
//! - `render(t)` → Tier 1 语义相位：t=1 为精确稳态；t∈[0,1) 入场 stagger
//!   （节点 fade+grow、边/标签 fade，按指令序级联）；disable 时任意 t 渲染稳态
//! - `hit-regions()` → 节点 AABB + node-id + 声明式 hover 效果（宿主侧命中，零 wasm 调用）
//!
//! Tier 2（v2）：`anims` 多轨道协议通道；稳态帧仅在 SignalFlow preset 下附着
//! 边呼吸动效（opacity 0.75↔1.0 ping-pong），hover 聚焦时附着 dim 过渡
//! （Once opacity）与相连边脉冲（Loop translate 路径跟随 + opacity 端点
//! 淡入淡出）——宿主本地采样零 wasm 调用，全部完成即静止（activity 判据）。

use crate::convert::{
    layer_to_wit_layer, layout_to_hit_regions, record_to_wit_theme, wit_theme_to_record,
};
use crate::wit_types::*;
use mermaid_canvas_component::theme::ThemeRecord;
use mermaid_canvas_component::{
    builtin_theme_record, compute_layout, FlowchartRenderer, Layout, LayoutConfig, RecordTheme,
    SequenceRenderer, StylePreset, Theme,
};
use mermaid_canvas_core::{DiagramAst, DiagramKind};

/// 级联相位偏移总量上限（毫秒）— 与 deneb 入场编排语义对齐
const STAGGER_CAP_MS: f64 = 400.0;
/// 级联延迟占入场时长的最大比例（item 窗口恒为 0.6，保证 t=1 全部完成）
const DELAY_FRAC_CAP: f64 = 0.4;
/// 静态层（背景/子图）纯淡入相位窗上界
const STATIC_FADE_UNTIL: f64 = 0.3;
/// SignalFlow 呼吸动效参数（Tier 2 附着 — opacity 0.75↔1.0 ping-pong）
const BREATH_DURATION_MS: u32 = 2400;
/// 选中荧光分层（描边宽度增量 / alpha — 元素本色向外扩散,R8）
const SELECTED_GLOW_LAYERS: [(f64, f64); 4] =
    [(1.8, 0.32), (3.6, 0.18), (6.0, 0.09), (9.0, 0.045)];
/// 选中荧光呼吸周期（PingPong opacity 1.0↔0.55;disable 时静态）
const SELECTED_GLOW_BREATH_MS: u32 = 1800;
/// hover 提亮强度（向白色收敛比例）
const HOVER_LIGHTEN: f64 = 0.18;
/// hover 语义色收敛比例（hover-color 注入时）
const HOVER_TINT: f64 = 0.5;

// ─── 关联聚焦（hover 1-hop 高亮 + 相连边脉冲 — canvas@2.0.0 Tier 2）───

/// 非相关元素淡化 alpha（保留色相；对齐 deneb 柱图 dim 0.32 语义 / archify 关联聚焦 ~0.2 观感）
const FOCUS_DIM_ALPHA: f64 = 0.25;
/// dim 进/出过渡时长（Tier 2 Once 采样；archify UI 态 140-200ms 档）
const FOCUS_FADE_MS: u32 = 150;
/// 相连边脉冲周期（archify relationship-pulse token-life 1.2s 同档）
const PULSE_DURATION_MS: u32 = 1100;
/// 路径跟随关键帧采样数（折线弧长均分 → 分段线性跟随，恒速过弯）
const PULSE_SAMPLES: usize = 12;
/// 脉冲 pill 核心（长 / 粗 / alpha — 比连线稍粗的流动线段;R9 收窄:荧光感
/// 由分层光晕承担,芯体保持锐利）
const PULSE_PILL: (f64, f64, f64) = (19.0, 4.0, 0.95);
/// pill 荧光中层（更长 / 更宽 / 低 alpha — 同轨道随行的辉光托底）
const PULSE_PILL_GLOW: (f64, f64, f64) = (27.0, 10.0, 0.22);
/// pill 光晕外层（最宽最淡 — bloom 余晖,边色系）
const PULSE_PILL_HALO: (f64, f64, f64) = (34.0, 15.0, 0.10);
/// 每边脉冲指令总数（halo + glow + core 三层;测试契约锁定）
#[cfg_attr(not(test), allow(dead_code))]
const PULSE_CMDS_PER_EDGE: usize = 3;
/// 相连边之间的错峰间隔（避免全部边同步启脉）
const PULSE_EDGE_STAGGER_MS: u32 = 80;

/// dim 目标层判别（键 = 层 + 指令位置；布局生命周期内跨 render 稳定）
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum DimLayer {
    Nodes,
    Edges,
    Labels,
    Subgraphs,
}

impl DimLayer {
    fn kind(self) -> &'static str {
        match self {
            DimLayer::Nodes => "nodes",
            DimLayer::Edges => "edges",
            DimLayer::Labels => "labels",
            DimLayer::Subgraphs => "subgraphs",
        }
    }
}

type DimKey = (DimLayer, usize);

/// 关联聚焦索引 — ensure_steady 伴随稳态构建（invalidate 一并失效）
#[derive(Clone, Debug, Default)]
struct FocusMap {
    /// hit 索引 → 节点 id（BTreeMap 过滤序，与 layout_to_hit_regions 口径一致）
    node_ids: Vec<String>,
    /// 边聚焦信息（布局边序）
    edges: Vec<FocusEdge>,
    /// labels 层控制块标签起始位（时序图 loop/alt 文本；流程图为尾部即空区间）
    control_label_start: usize,
    /// 时序图激活框属主（nodes 层尾部无 id 指令按位对应）
    activation_owners: Vec<String>,
}

/// 单条边的聚焦信息
#[derive(Clone, Debug, Default)]
struct FocusEdge {
    from: String,
    to: String,
    /// 路由折线（脉冲路径跟随采样源）
    points: Vec<(f64, f64)>,
    /// 生命线（时序图内部结构 — 参与 dim 判定，不参与脉冲）
    is_lifeline: bool,
    /// 边层指令位置（辉光/主线/箭头/装饰全组）
    cmd_slots: Vec<usize>,
    /// 边标签在 labels 层的位置
    label_slot: Option<usize>,
}

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
        // quint-out — archify 签名曲线 cubic-bezier(0.22,1,0.36,1) 的近似（T22）
        "quint-out" => 1.0 - (1.0 - x).powi(5),
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
    /// 关联聚焦索引（稳态伴随构建；invalidate 一并失效）
    focus: Option<FocusMap>,
    /// 上一帧 dim 目标集（过渡方向判定：newly 1→dim / staying 恒 dim / restored dim→1）
    last_dim: std::collections::BTreeSet<DimKey>,
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
            focus: None,
            last_dim: Default::default(),
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

    /// 交互状态回注（hover 关联聚焦 + 脉冲 / 提亮 / 选中 outline）
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
    /// 每区域按 preset 档位声明 hover 效果（T23 — 宿主对 draw-cmd.id == index
    /// 的指令采样渲染，零 wasm 调用）。解析失败会话返回空表：WIT 协议方法无
    /// 错误通道，诚实降级为「无命中区」；宿主以 `render()` 的 Err 作为解析失败的权威信号。
    pub fn hit_regions(&mut self) -> Vec<WitHitRegion> {
        if self.ast.is_err() {
            return Vec::new();
        }
        self.ensure_scaled();
        let scaled = self.scaled.as_ref().expect("ensure_scaled 建立布局(ast 已 Ok)");
        let mut regions = layout_to_hit_regions(&scaled.layout);
        let preset = StylePreset::parse(&self.theme_record.style_preset);
        let hover_spec = preset.hover_effect();
        for region in &mut regions {
            region.hover = Some(WitHoverEffect {
                kind: hover_spec.kind.to_string(),
                params: hover_spec.params.clone(),
            });
        }
        regions
    }

    /// 解析错误访问器（lib_mode/宿主侧区分「空命中区」与「源码无效」）
    pub fn parse_error(&self) -> Option<&str> {
        self.ast.as_ref().err().map(String::as_str)
    }

    // ─── 内部：布局与稳态 ────────────────────────────────────

    fn invalidate(&mut self) {
        self.scaled = None;
        self.steady = None;
        self.focus = None;
        // 布局失效后 dim 键不再指向有效指令 — 整体丢弃，退场即全新淡入
        self.last_dim.clear();
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
        let mut layers: Vec<WitLayer> = output.layers.all().iter().map(|l| layer_to_wit_layer(l.clone())).collect();
        // Tier 2 附着（T23）：SignalFlow preset 且未禁用 — 边 path 附着 opacity 呼吸
        if !self.anim_cfg.disable
            && StylePreset::parse(&self.theme_record.style_preset) == StylePreset::SignalFlow
        {
            attach_edge_breathing(&mut layers);
        }
        // 关联聚焦索引（hover 1-hop dim 分组 + 脉冲路径源）
        let focus = build_focus(&mut layers, &scaled.layout, ast.kind);
        self.focus = Some(focus);
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
                    let phases = self.family_phases(t, &layer.commands);
                    for (cmd, p) in layer.commands.iter_mut().zip(phases) {
                        if p < 1.0 {
                            scale_cmd_geometry(cmd, grow_factor(p));
                            apply_cmd_alpha(cmd, p);
                        }
                    }
                }
                "edges" | "labels" => {
                    let phases = self.family_phases(t, &layer.commands);
                    for (cmd, p) in layer.commands.iter_mut().zip(phases) {
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

    /// 命令族相位（R10 家族同步）— 同一 `cmd.id` 的全部指令（节点 =
    /// 主体/bevel/sigil；边 = 主线/箭头）共享一个入场相位，stagger 秩取
    /// 该 id 首个出现位置；无 id 指令回退自身索引。
    fn family_phases(&self, t: f64, commands: &[WitDrawCmd]) -> Vec<f64> {
        let mut first_rank: std::collections::BTreeMap<u32, usize> = Default::default();
        commands
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let rank = match cmd.id {
                    Some(id) => *first_rank.entry(id).or_insert(i),
                    None => i,
                };
                self.item_phase(t, rank)
            })
            .collect()
    }

    /// 第 idx 项在相位 t 的入场进度（0..1，已缓动）
    ///
    /// stagger 档位随 preset 联动（T22）：SignalFlow ×0.8 更细腻、Blueprint ×0 无 stagger
    fn item_phase(&self, t: f64, idx: usize) -> f64 {
        let total = self.anim_cfg.enter_duration_ms.max(1.0);
        let preset = StylePreset::parse(&self.theme_record.style_preset);
        let stagger = self.anim_cfg.stagger_ms * preset.stagger_factor();
        let delay_ms = (idx as f64 * stagger).min(STAGGER_CAP_MS);
        let delay_frac = (delay_ms / total).min(DELAY_FRAC_CAP);
        let x = ((t - delay_frac) / (1.0 - DELAY_FRAC_CAP)).clamp(0.0, 1.0);
        eval_easing(&self.anim_cfg.easing, x)
    }

    // ─── 内部：交互态 ────────────────────────────────────────

    /// hover 提亮 / 选中 outline / 关联聚焦（1-hop dim + 相连边脉冲）
    ///
    /// 命中匹配按 draw-cmd.id == 命中区索引（v2 命令身份接线）。关联聚焦：
    /// hover 节点及其直接相连元素保持全亮，其余经 Tier 2 opacity 过渡至
    /// [`FOCUS_DIM_ALPHA`]（保留色相）；相连边附着路径跟随脉冲光点
    /// （translate 关键帧 = 折线弧长采样，Loop 恒速流动，尾迹错峰成 comet）。
    /// hover 离场：此前 dim 的指令附着恢复轨道（dim→1），宿主 Once 采样
    /// 完成后归位恒等态（配合宿主 activity 判据收敛为非脏）。
    fn apply_interaction(&mut self, result: &mut WitRenderResult) {
        if self.state.hovered.is_none()
            && self.state.selected.is_empty()
            && self.last_dim.is_empty()
        {
            return;
        }
        let Some(focus) = self.focus.clone() else { return };
        let n_nodes = focus.node_ids.len();

        // —— 目标 dim 集 + 相连边脉冲 ——
        let mut dim: std::collections::BTreeSet<DimKey> = Default::default();
        let mut pulses: Vec<WitDrawCmd> = Vec::new();
        let mut hovered_valid = false;
        if let Some(h) = self.state.hovered {
            // 源更新后陈旧索引防御：越界即不进入聚焦（避免全图误 dim）
            if (h as usize) < n_nodes {
                hovered_valid = true;
                let pid = focus.node_ids[h as usize].clone();
                let related_edges: Vec<usize> = focus
                    .edges
                    .iter()
                    .enumerate()
                    .filter(|(_, fe)| fe.from == pid || fe.to == pid)
                    .map(|(i, _)| i)
                    .collect();
                let related_nodes: std::collections::BTreeSet<&str> = {
                    let mut set = std::collections::BTreeSet::new();
                    set.insert(pid.as_str());
                    for &ei in &related_edges {
                        let fe = &focus.edges[ei];
                        set.insert(fe.from.as_str());
                        set.insert(fe.to.as_str());
                    }
                    set
                };
                let related_hit: std::collections::BTreeSet<usize> = focus
                    .node_ids
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| related_nodes.contains(id.as_str()))
                    .map(|(i, _)| i)
                    .collect();

                // 节点层：非相关节点（含时序图尾部激活框按属主判定）
                if let Some(layer) = result.layers.iter().find(|l| l.kind == "nodes") {
                    // 激活框区起点 = 首个无 id rect（R7 卡片族尺寸随 preset 变化,
                    // 不能按 n_nodes 固定步进;激活框族 = 无 id 的 rect 连续段）
                    let act_base = layer
                        .commands
                        .iter()
                        .position(|c| c.id.is_none() && c.cmd_type == "rect");
                    for (pos, cmd) in layer.commands.iter().enumerate() {
                        let unrelated = match cmd.id {
                            Some(i) => !related_hit.contains(&(i as usize)),
                            // 无 id rect = 时序图激活框（按属主判定;其他无 id
                            // 指令 = 脉冲点/底盘等,不参与节点 dim）
                            None if cmd.cmd_type == "rect" => {
                                let slot = act_base.and_then(|b| pos.checked_sub(b));
                                !slot
                                    .and_then(|s| focus.activation_owners.get(s))
                                    .map(|owner| related_nodes.contains(owner.as_str()))
                                    .unwrap_or(false)
                            }
                            None => false,
                        };
                        if unrelated {
                            dim.insert((DimLayer::Nodes, pos));
                        }
                    }
                }
                // 边层：非相关边全组（辉光/主线/箭头/装饰）+ 边标签（含前置底盘）
                for (i, fe) in focus.edges.iter().enumerate() {
                    if related_edges.contains(&i) {
                        continue;
                    }
                    for &p in &fe.cmd_slots {
                        dim.insert((DimLayer::Edges, p));
                    }
                    if let Some(l) = fe.label_slot {
                        dim.insert((DimLayer::Labels, l));
                        // 底盘在文字前一格（渲染器契约:plate + text 相邻对）
                        dim.insert((DimLayer::Labels, l - 1));
                    }
                }
                // 节点标签（labels 层首块 == hit 序）+ 控制块标签（尾部全 dim）
                for i in 0..n_nodes {
                    if !related_hit.contains(&i) {
                        dim.insert((DimLayer::Labels, i));
                    }
                }
                if let Some(layer) = result.layers.iter().find(|l| l.kind == "labels") {
                    for pos in focus.control_label_start..layer.commands.len() {
                        dim.insert((DimLayer::Labels, pos));
                    }
                }
                // 子图框（控制块/子图容器）全 dim
                if let Some(layer) = result.layers.iter().find(|l| l.kind == "subgraphs") {
                    for pos in 0..layer.commands.len() {
                        dim.insert((DimLayer::Subgraphs, pos));
                    }
                }

                // 相连边脉冲（生命线不参与；disable 尊重静态场景）
                if !self.anim_cfg.disable {
                    // 边色 = 脉冲光晕色系;芯体 = 模式感知荧光色(与边色分离,R9-2)
                    let (edge_color, core_color) = match self.scaled.as_ref() {
                        Some(s) => {
                            let dark =
                                mermaid_canvas_component::theme::color_luma(s.theme.background_color()) < 0.5;
                            (
                                s.theme.edge_color().to_string(),
                                pulse_core_color(dark, s.theme.node_text_color()),
                            )
                        }
                        None => (
                            "#8a8f98".to_string(),
                            pulse_core_color(false, "#1f2328"),
                        ),
                    };
                    let mut rank = 0u32;
                    for &ei in &related_edges {
                        let fe = &focus.edges[ei];
                        if fe.is_lifeline || fe.points.len() < 2 {
                            continue;
                        }
                        let base_delay = rank * PULSE_EDGE_STAGGER_MS;
                        rank += 1;
                        pulses.extend(pulse_pill_cmds(fe, base_delay, &edge_color, &core_color));
                    }
                }
            }
        }

        // —— dim 过渡附着（对比上一帧目标集）——
        // clone 而非 take:退场路径保留 prev(幂等重放恢复轨道直到下一次
        // 有效 hover 重算目标集);take 拿空后第二次渲染会早退产出干净帧,
        // 150ms 恢复淡出被顶替成跳变
        let prev = self.last_dim.clone();
        if !self.anim_cfg.disable {
            for &k in &dim {
                let (from, to) = if prev.contains(&k) {
                    (FOCUS_DIM_ALPHA, FOCUS_DIM_ALPHA)
                } else {
                    (1.0, FOCUS_DIM_ALPHA)
                };
                if let Some(cmd) = cmd_at_mut(result, k.0, k.1) {
                    attach_dim_anim(cmd, from, to);
                }
            }
            for &k in prev.iter().filter(|k| !dim.contains(k)) {
                if let Some(cmd) = cmd_at_mut(result, k.0, k.1) {
                    attach_dim_anim(cmd, FOCUS_DIM_ALPHA, 1.0);
                }
            }
        } else {
            // disable：无 Tier 2 — 直接烘焙 alpha（保留 hover 聚焦语义，零动画）
            for &k in &dim {
                if let Some(cmd) = cmd_at_mut(result, k.0, k.1) {
                    apply_cmd_alpha(cmd, FOCUS_DIM_ALPHA);
                }
            }
        }
        // 仅在有效 hover 聚焦计算后更新目标集:退场/无 hover 渲染保留旧集,
        // 恢复轨道幂等重放(transition 期间宿主每帧重渲染,清空会被干净稳态
        // 顶替,150ms 淡出被打断成跳变);静止判定交给宿主 activity 判据
        if hovered_valid {
            self.last_dim = dim;
        }

        // —— hover 提亮 / 选中荧光扩散（节点层，叠加于聚焦之上）——
        let mut selection_glow: Vec<WitDrawCmd> = Vec::new();
        if let Some(layer) = result.layers.iter_mut().find(|l| l.kind == "nodes") {
            if hovered_valid {
                if let Some(h) = self.state.hovered {
                    for cmd in layer.commands.iter_mut() {
                        if cmd.id == Some(h) {
                            match &self.theme_record.hover_color {
                                Some(hc) => tint_cmd_fill_toward(cmd, hc, HOVER_TINT),
                                None => lighten_cmd_fill(cmd, HOVER_LIGHTEN),
                            }
                        }
                    }
                }
            }
            // 选中荧光扩散(R8):去黑粗边 — 以元素本色向外的分层辉光 + 呼吸
            // (仅主体指令:有填充的族成员,排除柔影/bevel 剪影)
            for &s in &self.state.selected {
                for cmd in layer.commands.iter() {
                    if cmd.id == Some(s) && cmd.fill.is_some() && cmd.cmd_type != "text" {
                        selection_glow.extend(selection_glow_cmds(cmd, !self.anim_cfg.disable));
                    }
                }
            }
            // 荧光层先入(节点之上、脉冲之下;携带节点 id 与主体联动)
            layer.commands.append(&mut selection_glow);
            // 脉冲点追加（层尾 = 绘制于全部节点之上；无 id → 不参与宿主 hover 命中）
            layer.commands.append(&mut pulses);
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

/// SignalFlow 呼吸动效附着（T23）— 边层全部 path 指令挂 opacity 0.75↔1.0
///
/// 2400ms ping-pong；disable 或非 SignalFlow 不附着（调用方门控）。
fn attach_edge_breathing(layers: &mut [WitLayer]) {
    for layer in layers.iter_mut() {
        if layer.kind != "edges" {
            continue;
        }
        for cmd in &mut layer.commands {
            if cmd.cmd_type != "path" {
                continue;
            }
            cmd.anims.push(WitAnimDesc {
                property: WitAnimProperty::Opacity,
                keyframes: vec![
                    WitKeyframe { t: 0.0, value: 0.75, easing: "sine-in-out".to_string() },
                    WitKeyframe { t: 1.0, value: 1.0, easing: "sine-in-out".to_string() },
                ],
                duration_ms: BREATH_DURATION_MS,
                delay_ms: 0,
                loop_mode: WitLoopMode::PingPong,
                alt_color: None,
            });
        }
    }
}

// ─── 关联聚焦辅助 ───────────────────────────────────────────

/// 层内指令寻位（dim 过渡 / 烘焙共用）
fn cmd_at_mut(result: &mut WitRenderResult, kind: DimLayer, pos: usize) -> Option<&mut WitDrawCmd> {
    result
        .layers
        .iter_mut()
        .find(|l| l.kind == kind.kind())
        .and_then(|l| l.commands.get_mut(pos))
}

/// 构建关联聚焦索引 — 消费渲染器边组归属标记（`CmdDecor.id` = 布局边索引），
/// 分组边层指令槽位后**剥除 id**：宿主 hover 效果以节点 hit-index 命中，
/// 边指令残留 id 会造成跨命中串扰。
///
/// labels 层契约（两渲染器一致，测试锁定）：[节点标签 == hit 序]
/// → [边标签按布局边序（跳过生命线）] → [控制块标签（时序图）]。
/// nodes 层契约：[节点指令（id = hit 序）] → [激活框（无 id，时序图）]。
fn build_focus(layers: &mut [WitLayer], layout: &Layout, kind: DiagramKind) -> FocusMap {
    let node_ids: Vec<String> = layout
        .nodes
        .iter()
        .filter(|(k, _)| !k.starts_with("__act_"))
        .map(|(_, nl)| nl.id.clone())
        .collect();

    let mut label_pos = node_ids.len();
    let mut edges: Vec<FocusEdge> = layout
        .edges
        .iter()
        .map(|el| {
            let is_lifeline = el.to.ends_with("_lifeline_end");
            // R7 边标签 = 底盘 + 文字两条指令（渲染器契约）— 槽位指向文字指令
            let label_slot = if !is_lifeline && el.label.is_some() {
                let slot = Some(label_pos + 1);
                label_pos += 2;
                slot
            } else {
                None
            };
            FocusEdge {
                from: el.from.clone(),
                to: el.to.clone(),
                points: el.points.clone(),
                is_lifeline,
                cmd_slots: Vec::new(),
                label_slot,
            }
        })
        .collect();

    if let Some(layer) = layers.iter_mut().find(|l| l.kind == "edges") {
        for (pos, cmd) in layer.commands.iter_mut().enumerate() {
            if let Some(ei) = cmd.id.take() {
                if let Some(fe) = edges.get_mut(ei as usize) {
                    fe.cmd_slots.push(pos);
                }
            }
        }
    }

    let activation_owners = if kind == DiagramKind::Sequence {
        layout
            .nodes
            .iter()
            .filter(|(k, _)| k.starts_with("__act_"))
            .filter_map(|(_, nl)| activation_owner(&nl.id))
            .collect()
    } else {
        Vec::new()
    };

    FocusMap {
        node_ids,
        edges,
        control_label_start: label_pos,
        activation_owners,
    }
}

/// 激活框属主解析 — nl.id = "activation_{participant_id}_{start_step}"
/// （participant_id 可含下划线，末段恒为步号 → rsplit_once）
fn activation_owner(node_id: &str) -> Option<String> {
    let rest = node_id.strip_prefix("activation_")?;
    let (pid, _) = rest.rsplit_once('_')?;
    Some(pid.to_string())
}

/// dim 过渡轨道（Once；from == to 即恒值 — 保持已 dim 状态跨 render 稳定）
fn attach_dim_anim(cmd: &mut WitDrawCmd, from: f64, to: f64) {
    cmd.anims.push(WitAnimDesc {
        property: WitAnimProperty::Opacity,
        keyframes: vec![
            WitKeyframe { t: 0.0, value: from as f32, easing: "cubic-out".to_string() },
            WitKeyframe { t: 1.0, value: to as f32, easing: "cubic-out".to_string() },
        ],
        duration_ms: FOCUS_FADE_MS,
        delay_ms: 0,
        loop_mode: WitLoopMode::Once,
        alt_color: None,
    });
}

/// 折线弧长均分采样 — f ∈ [0,1] → 沿线位置（恒速；路径跟随脉冲的几何源）
fn poly_sample(points: &[(f64, f64)], f: f64) -> (f64, f64) {
    match points.len() {
        0 => (0.0, 0.0),
        1 => points[0],
        _ => {
            let seg_len: Vec<f64> = points
                .windows(2)
                .map(|w| {
                    let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                    (dx * dx + dy * dy).sqrt()
                })
                .collect();
            let total: f64 = seg_len.iter().sum();
            if total <= f64::EPSILON {
                return points[0];
            }
            let target = f.clamp(0.0, 1.0) * total;
            let mut acc = 0.0;
            for (i, &l) in seg_len.iter().enumerate() {
                if acc + l >= target || i == seg_len.len() - 1 {
                    let t = if l > f64::EPSILON { (target - acc) / l } else { 0.0 };
                    let (a, b) = (points[i], points[i + 1]);
                    return (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                }
                acc += l;
            }
            points[points.len() - 1]
        }
    }
}

/// 选中荧光扩散指令族（R8）— 元素本体的几何克隆,去填充、以元素
/// 自身颜色(描边优先)微提亮后的分层描边,宽度递增 alpha 递减向外
/// 扩散;携带本体 id 与 hover/dim 联动。breathing = Tier 2 呼吸轨道。
fn selection_glow_cmds(body: &WitDrawCmd, breathing: bool) -> Vec<WitDrawCmd> {
    let paint_head = |p: &Option<WitPaint>| -> Option<String> {
        match p {
            Some(WitPaint::Solid(c)) => Some(c.clone()),
            Some(WitPaint::Gradient(g)) => g.stops.first().map(|s| s.color.clone()),
            None => None,
        }
    };
    // 荧光基色 = 元素自身颜色:描边色优先,回退填充首色
    let base_hex = paint_head(&body.stroke)
        .or_else(|| paint_head(&body.fill))
        .unwrap_or_default();
    if parse_hex(&base_hex).is_none() {
        return Vec::new();
    }
    let glow_color = lighten(&base_hex, 0.25);
    SELECTED_GLOW_LAYERS
        .iter()
        .map(|&(extra, alpha)| {
            let mut g = WitDrawCmd {
                fill: None,
                stroke: Some(WitPaint::Solid(with_alpha(&glow_color, alpha))),
                stroke_width: Some(extra),
                anims: if breathing {
                    vec![WitAnimDesc {
                        property: WitAnimProperty::Opacity,
                        keyframes: vec![
                            WitKeyframe { t: 0.0, value: 1.0, easing: "sine-in-out".to_string() },
                            WitKeyframe { t: 1.0, value: 0.55, easing: "sine-in-out".to_string() },
                        ],
                        duration_ms: SELECTED_GLOW_BREATH_MS,
                        delay_ms: 0,
                        loop_mode: WitLoopMode::PingPong,
                        alt_color: None,
                    }]
                } else {
                    Vec::new()
                },
                ..body.clone()
            };
            g.text_content = None;
            g.font = None;
            g.id = body.id;
            g
        })
        .collect()
}

/// 折线 f 处所在线段的方向角（度;pill 转向采样源）
fn poly_angle_deg(points: &[(f64, f64)], f: f64) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let seg_len: Vec<f64> = points
        .windows(2)
        .map(|w| {
            let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            (dx * dx + dy * dy).sqrt()
        })
        .collect();
    let total: f64 = seg_len.iter().sum();
    if total <= f64::EPSILON {
        return 0.0;
    }
    let target = f.clamp(0.0, 1.0) * total;
    let mut acc = 0.0;
    for (i, &l) in seg_len.iter().enumerate() {
        if acc + l >= target || i == seg_len.len() - 1 {
            let (dx, dy) = (points[i + 1].0 - points[i].0, points[i + 1].1 - points[i].1);
            return dy.atan2(dx).to_degrees();
        }
        acc += l;
    }
    0.0
}

/// 相连边脉冲 pill（R8 — 比连线稍粗的流动线段 + 荧光宽层）：
/// 锚定路径起点居中的圆角矩形，translate 关键帧编码整条折线相对位移
/// （宿主分段线性 = 恒速路径跟随），rotate 关键帧按所在线段方向角转向
/// （绕 pill 中心 = 平移中心，转心恒在路径上）；opacity 轨道环首尾
/// 淡入淡出（Loop 重启无跳变）。glow 层 = 更长更宽低 alpha 同轨道随行。
fn pulse_pill_layer(
    fe: &FocusEdge,
    delay_ms: u32,
    length: f64,
    height: f64,
    base_alpha: f64,
    color: &str,
) -> WitDrawCmd {
    let p0 = fe.points[0];
    let mut kf_x = Vec::with_capacity(PULSE_SAMPLES);
    let mut kf_y = Vec::with_capacity(PULSE_SAMPLES);
    let mut kf_r = Vec::with_capacity(PULSE_SAMPLES);
    for k in 0..PULSE_SAMPLES {
        let f = k as f64 / (PULSE_SAMPLES - 1) as f64;
        let (x, y) = poly_sample(&fe.points, f);
        kf_x.push(WitKeyframe { t: f as f32, value: (x - p0.0) as f32, easing: "linear".to_string() });
        kf_y.push(WitKeyframe { t: f as f32, value: (y - p0.1) as f32, easing: "linear".to_string() });
        kf_r.push(WitKeyframe { t: f as f32, value: poly_angle_deg(&fe.points, f) as f32, easing: "linear".to_string() });
    }
    WitDrawCmd {
        cmd_type: "rect".to_string(),
        // 居中锚定 p0:平移中心 = pill 中心 = rotate 中心,恒在路径上
        params: vec![p0.0 - length / 2.0, p0.1 - height / 2.0, length, height],
        fill: Some(WitPaint::Solid(with_alpha(color, base_alpha))),
        stroke: None,
        stroke_width: None,
        // 全圆角 = pill(半径 = 高度一半)
        corner_radius: Some(height / 2.0),
        corner_radii: None,
        dash: None,
        line_cap: None,
        shadow: None,
        text_content: None,
        font: None,
        group_depth: 0,
        id: None,
        anims: vec![
            WitAnimDesc {
                property: WitAnimProperty::TranslateX,
                keyframes: kf_x,
                duration_ms: PULSE_DURATION_MS,
                delay_ms,
                loop_mode: WitLoopMode::Loop,
                alt_color: None,
            },
            WitAnimDesc {
                property: WitAnimProperty::TranslateY,
                keyframes: kf_y,
                duration_ms: PULSE_DURATION_MS,
                delay_ms,
                loop_mode: WitLoopMode::Loop,
                alt_color: None,
            },
            WitAnimDesc {
                property: WitAnimProperty::Rotate,
                keyframes: kf_r,
                duration_ms: PULSE_DURATION_MS,
                delay_ms,
                loop_mode: WitLoopMode::Loop,
                alt_color: None,
            },
            WitAnimDesc {
                property: WitAnimProperty::Opacity,
                keyframes: vec![
                    WitKeyframe { t: 0.0, value: 0.0, easing: "linear".to_string() },
                    WitKeyframe { t: 0.1, value: 1.0, easing: "linear".to_string() },
                    WitKeyframe { t: 0.85, value: 1.0, easing: "linear".to_string() },
                    WitKeyframe { t: 1.0, value: 0.0, easing: "linear".to_string() },
                ],
                duration_ms: PULSE_DURATION_MS,
                delay_ms,
                loop_mode: WitLoopMode::Loop,
                alt_color: None,
            },
        ],
    }
}

/// 脉冲芯体荧光色 — 模式感知(§1.3 双模式平权),与边色构造性分离:
/// 深底 = 前景再提亮(白热荧光芯);浅底 = 前景再压深(墨芯彗核)。
/// 边色是中调弱化色,芯体取前景极端端 — 任何主题下两者都不可混淆(R9-2)。
fn pulse_core_color(dark_bg: bool, foreground: &str) -> String {
    if dark_bg {
        lighten(foreground, 0.55)
    } else {
        mermaid_canvas_component::theme::darken_color(foreground, 0.35)
    }
}

/// 一条相连边的完整脉冲指令族（R9-2 三层荧光）：
/// halo（边色晕 bloom 余晖）→ glow（边色提亮荧光托底）→ core（模式感知芯体）
fn pulse_pill_cmds(fe: &FocusEdge, delay_ms: u32, edge_color: &str, core_color: &str) -> Vec<WitDrawCmd> {
    let (hl, hh, ha) = PULSE_PILL_HALO;
    let (gl, gh, ga) = PULSE_PILL_GLOW;
    let (cl, ch, ca) = PULSE_PILL;
    vec![
        pulse_pill_layer(fe, delay_ms, hl, hh, ha, &lighten(edge_color, 0.3)),
        pulse_pill_layer(fe, delay_ms, gl, gh, ga, &lighten(edge_color, 0.55)),
        pulse_pill_layer(fe, delay_ms, cl, ch, ca, core_color),
    ]
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
        title: None,
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
    if let Some(mut title) = layout.title.clone() {
        title.x *= s;
        title.y *= s;
        title.width *= s;
        title.height *= s;
        title.font_size *= s;
        out.title = Some(title);
    }
    out
}

// ─── WIT 指令后处理（入场 / 交互）───────────────────────────

// 色彩工具统一委托 component 的 theme 工具(R9 — hex 之外的 CSS 色串
// 同样可解析,消除会话层本地 hex-only 副本)

fn parse_hex(color: &str) -> Option<(u8, u8, u8)> {
    mermaid_canvas_component::theme::parse_hex_color(color)
}

fn with_alpha(color: &str, alpha: f64) -> String {
    mermaid_canvas_component::theme::with_color_alpha(color, alpha)
}

/// 烘焙 alpha 乘法叠加（R10 — 见 theme::mul_color_alpha 注释）
fn mul_alpha(color: &str, factor: f64) -> String {
    mermaid_canvas_component::theme::mul_color_alpha(color, factor)
}

fn lighten(color: &str, amount: f64) -> String {
    mermaid_canvas_component::theme::lighten_color(color, amount)
}

fn paint_with_alpha(paint: &WitPaint, alpha: f64) -> WitPaint {
    // 乘法叠加而非替换（R10）：保留 bevel(0.28) 等装饰的基 alpha，
    // 动画结束不再跳变；基 alpha 1.0 的常规填充行为不变
    match paint {
        WitPaint::Solid(c) => WitPaint::Solid(mul_alpha(c, alpha)),
        // 渐变按 stop 逐个注 alpha（T21 — 渐变填充节点参与入场淡入）
        WitPaint::Gradient(g) => WitPaint::Gradient(WitLinearGradient {
            x0: g.x0,
            y0: g.y0,
            x1: g.x1,
            y1: g.y1,
            stops: g.stops.iter().map(|s| WitGradientStop {
                pos: s.pos,
                color: mul_alpha(&s.color, alpha),
            }).collect(),
        }),
    }
}

fn apply_cmd_alpha(cmd: &mut WitDrawCmd, alpha: f64) {
    if let Some(fill) = cmd.fill.take() {
        cmd.fill = Some(paint_with_alpha(&fill, alpha));
    }
    if let Some(stroke) = cmd.stroke.take() {
        cmd.stroke = Some(paint_with_alpha(&stroke, alpha));
    }
    // 阴影随主体淡入淡出(R10 — 业界惯例:阴影 alpha 与元素 alpha 同轨,
    // 入场时阴影不领先于形状出现,避免「黑框先行」)
    if let Some(sh) = cmd.shadow.as_mut() {
        sh.alpha = (sh.alpha * alpha).clamp(0.0, 1.0);
    }
}

/// 向目标色混合单色（hex 解析失败退化为提亮）
fn mix_hex_toward(c: &str, target: &str, amount: f64) -> String {
    match (parse_hex(c), parse_hex(target)) {
        (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
            let mix = |a: u8, b: u8| -> u8 {
                (a as f64 + (b as f64 - a as f64) * amount).round() as u8
            };
            format!("#{:02x}{:02x}{:02x}", mix(r1, r2), mix(g1, g2), mix(b1, b2))
        }
        _ => lighten(c, amount),
    }
}

fn lighten_cmd_fill(cmd: &mut WitDrawCmd, amount: f64) {
    match &cmd.fill {
        Some(WitPaint::Solid(c)) => {
            cmd.fill = Some(WitPaint::Solid(lighten(c, amount)));
        }
        // R7 渐变填充（classic 微渐变起）— 逐 stop 提亮
        Some(WitPaint::Gradient(g)) => {
            let mut ng = g.clone();
            for stop in &mut ng.stops {
                stop.color = lighten(&stop.color, amount);
            }
            cmd.fill = Some(WitPaint::Gradient(ng));
        }
        _ => {}
    }
}

/// 填充向目标语义色收敛（hover-color 注入路径；渐变逐 stop）
fn tint_cmd_fill_toward(cmd: &mut WitDrawCmd, target: &str, amount: f64) {
    match &cmd.fill {
        Some(WitPaint::Solid(c)) => {
            cmd.fill = Some(WitPaint::Solid(mix_hex_toward(c, target, amount)));
        }
        Some(WitPaint::Gradient(g)) => {
            let mut ng = g.clone();
            for stop in &mut ng.stops {
                stop.color = mix_hex_toward(&stop.color, target, amount);
            }
            cmd.fill = Some(WitPaint::Gradient(ng));
        }
        _ => {}
    }
}

/// 围绕指令自身包围盒中心缩放几何（rect/circle/path；text 不缩放）
fn scale_cmd_geometry(cmd: &mut WitDrawCmd, s: f64) {
    // 渐变坐标随几何同步缩放（T21 — 保持渐变与形状对齐）
    let scale_gradient = |paint: &mut Option<WitPaint>, cx: f64, cy: f64| {
        if let Some(WitPaint::Gradient(g)) = paint {
            g.x0 = cx + (g.x0 - cx) * s;
            g.y0 = cy + (g.y0 - cy) * s;
            g.x1 = cx + (g.x1 - cx) * s;
            g.y1 = cy + (g.y1 - cy) * s;
        }
    };
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
            scale_gradient(&mut cmd.fill, cx, cy);
            scale_cmd_shadow(cmd, s);
        }
        "circle" if cmd.params.len() >= 3 => {
            cmd.params[2] *= s;
               scale_cmd_shadow(cmd, s);
        }
        "path" => {
            scale_path_params(&mut cmd.params, s);
               scale_cmd_shadow(cmd, s);
        }
        _ => {}
    }
}

/// 阴影参数入场随动（R10）：offset 随几何缩放（跟随形体移动），blur/spread
/// **不缩** — 柔度是观感参数而非几何量，随几何缩到 0 会把柔影退化成
/// 硬边灰块（业界入场惯例：形状缩放淡入，阴影只淡入不锐化）。
/// alpha 亦不动 — 淡入由 apply_cmd_alpha 独立控制，两轨正交。
fn scale_cmd_shadow(cmd: &mut WitDrawCmd, s: f64) {
    if let Some(sh) = cmd.shadow.as_mut() {
        sh.offset_x *= s;
        sh.offset_y *= s;
        sh.width *= s;
        sh.height *= s;
        let soften = s.max(0.85);
        sh.blur *= soften;
        sh.spread *= soften;
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

    /// 填充首色（Solid 色或 Gradient 首 stop — R7 classic 起节点为微渐变）
    fn fill_head(cmd: &WitDrawCmd) -> String {
        match &cmd.fill {
            Some(WitPaint::Solid(c)) => c.clone(),
            Some(WitPaint::Gradient(g)) => g.stops.first().map(|s| s.color.clone())
                .unwrap_or_else(|| panic!("gradient with no stops")),
            other => panic!("expected fill, got {:?}", other),
        }
    }

    /// 节点主体指令（首个带填充的族成员 — 跳过柔影/bevel）
    fn node_body<'a>(cmds: &'a [WitDrawCmd], id: u32) -> &'a WitDrawCmd {
        cmds.iter()
            .find(|c| c.id == Some(id) && c.fill.is_some())
            .unwrap_or_else(|| panic!("node body cmd with id={} not found", id))
    }

    fn area_of(cmd: &WitDrawCmd) -> f64 {
        if cmd.fill.is_none() {
            return 0.0; // 荧光/剪影(stroke-only)不占填充面积
        }
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
        // 缺省 classic preset：帧内零 anim-desc（呼吸动效仅 SignalFlow 附着）
        assert!(!r0.layers.iter().any(|l| l.commands.iter().any(|c| !c.anims.is_empty())));
    }

    #[test]
    fn test_no_anim_desc_attached_on_default_preset() {
        let mut s = session(FLOWCHART);
        for t in [0.0, 0.3, 0.7, 1.0] {
            let r = s.render(t).unwrap();
            assert!(
                !r.layers.iter().any(|l| l.commands.iter().any(|c| !c.anims.is_empty())),
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
        let base_hovered = fill_head(node_body(&nodes_layer(&base).commands, 0));
        let base_other = fill_head(node_body(&nodes_layer(&base).commands, 1));
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        let r = s.render(1.0).unwrap();
        let hovered = fill_head(node_body(&nodes_layer(&r).commands, 0));
        assert_ne!(hovered, base_hovered);
        assert!(hovered.starts_with('#'), "提亮保持 hex 形式: {}", hovered);
        // 未 hover 节点不变
        assert_eq!(fill_head(node_body(&nodes_layer(&r).commands, 1)), base_other);
    }

    #[test]
    fn test_selected_node_gets_glow_diffusion() {
        // R8: 选中 = 元素本色向外分层荧光 + 呼吸(替代黑粗边)
        let mut s = session(FLOWCHART);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: None, selected: vec![1] });
        let r = s.render(1.0).unwrap();
        let layer = nodes_layer(&r);
        // 主体描边不被覆盖(去黑粗边)
        let body = node_body(&layer.commands, 1);
        assert_ne!(body.stroke, Some(WitPaint::Solid("#333333".to_string())), "无前景色粗边覆盖");
        // 荧光族:3 层宽度递增 alpha 递减,携带节点 id,呼吸 PingPong
        let glows: Vec<&WitDrawCmd> = layer
            .commands
            .iter()
            .filter(|c| c.id == Some(1) && c.fill.is_none() && c.anims.len() == 1)
            .collect();
        assert_eq!(glows.len(), SELECTED_GLOW_LAYERS.len(), "3 层扩散荧光");
        for (i, (g, &(extra, alpha))) in glows.iter().zip(SELECTED_GLOW_LAYERS.iter()).enumerate() {
            assert_eq!(g.stroke_width, Some(extra), "层 {i} 宽度增量");
            let stroke_alpha = match &g.stroke {
                Some(WitPaint::Solid(c)) => c.clone(),
                other => panic!("glow stroke solid, got {:?}", other),
            };
            assert!(stroke_alpha.contains(&format!("{:.3}", alpha)), "层 {i} alpha {alpha}");
            let breath = &g.anims[0];
            assert!(matches!(breath.property, WitAnimProperty::Opacity));
            assert!(matches!(breath.loop_mode, WitLoopMode::PingPong), "荧光呼吸");
        }
        // 非选中节点无荧光轨道
        let other_glows = layer
            .commands
            .iter()
            .filter(|c| c.id == Some(0) && c.fill.is_none() && !c.anims.is_empty())
            .count();
        assert_eq!(other_glows, 0, "非选中无荧光");
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
        assert!(bg.commands.iter().any(|c| c.fill == Some(WitPaint::Solid("#1a1b26".to_string()))));
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
            hover_color: None,
            style_preset: None,
            font_family: "Mono".to_string(),
            base_font_size: 14.0,
            title_font_size: 18.0,
            margin: WitMargin { top: 20.0, right: 20.0, bottom: 20.0, left: 20.0 },
        };
        s.set_theme(theme);
        let r = s.render(1.0).unwrap();
        let bg = r.layers.iter().find(|l| l.kind == "background").unwrap();
        assert!(bg.commands.iter().any(|c| c.fill == Some(WitPaint::Solid("#000102".to_string()))));
        // T17：节点 fill = tint（底色混角色色）— 同槽同色、非底色、非全饱和原色
        let nodes = nodes_layer(&r);
        let fills: Vec<&WitPaint> = nodes
            .commands
            .iter()
            .filter_map(|c| c.fill.as_ref())
            .collect();
        // R7 微渐变:比较 stop 色(渐变坐标随节点位置不同,不参与相等性)
        let stop_colors: Vec<String> = fills
            .iter()
            .map(|f| match f {
                WitPaint::Solid(c) => c.clone(),
                WitPaint::Gradient(g) => g.stops.iter().map(|s| s.color.clone()).collect::<Vec<_>>().join(","),
            })
            .collect();
        assert!(
            stop_colors.iter().all(|c| *c == stop_colors[0]),
            "同槽（全 Rectangle）同 tint: {:?}",
            stop_colors
        );
        assert_ne!(*fills[0], WitPaint::Solid("#000102".to_string()), "非纯底色");
        assert_ne!(*fills[0], WitPaint::Solid("#112233".to_string()), "非全饱和原色");
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
        // Stadium = 全圆角（半径 = 高度一半）无损过 ABI
        assert_eq!(rect.corner_radius, Some(rect.params[3] / 2.0), "stadium 圆角无损过 ABI");
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
        // R9:委托 component 工具 — CSS rgb()/rgba() 亦可解析(alpha 丢弃)
        assert_eq!(parse_hex("rgba(1,2,3,0.5)"), Some((1, 2, 3)));
        assert_eq!(parse_hex("not-a-color"), None);
        assert_eq!(with_alpha("#ff0000", 0.5), "rgba(255,0,0,0.500)");
        assert_eq!(lighten("#000000", 0.5), "#808080");
        assert_eq!(lighten("not-a-color", 0.5), "not-a-color");
    }

    // ─── 保真度地基（T11-T16）端到端 ──────────────────────

    #[test]
    fn test_cjk_flowchart_full_pipeline() {
        // T16 回归：CJK 标签边行全管线不 panic，标签无损
        let mut s = session("flowchart TD\n  开始[启动] --> 结束[完成]");
        let r = s.render(1.0).unwrap();
        assert_eq!(s.hit_regions().len(), 2);
        let labels = r.layers.iter().find(|l| l.kind == "labels").unwrap();
        let texts: Vec<&str> = labels.commands.iter().filter_map(|c| c.text_content.as_deref()).collect();
        assert!(texts.contains(&"启动"), "labels: {:?}", texts);
        assert!(texts.contains(&"完成"));
    }

    #[test]
    fn test_title_renders_via_full_pipeline() {
        let mut s = session("flowchart TD\n    title 流程标题\n    A --> B");
        let r = s.render(1.0).unwrap();
        let title = r.layers.iter().find(|l| l.kind == "title").unwrap();
        assert!(title.commands.iter().any(|c| c.text_content.as_deref() == Some("流程标题")));
        // 标题带预留：总高大于无标题版
        let mut s2 = session("flowchart TD\n    A --> B");
        let r2 = s2.render(1.0).unwrap();
        assert!(r.height > r2.height, "title={} no-title={}", r.height, r2.height);
    }

    #[test]
    fn test_subgraph_box_renders_via_full_pipeline() {
        let src = "flowchart TD\n    subgraph sg1 [My Group]\n        A --> B\n    end\n    B --> C";
        let mut s = session(src);
        let r = s.render(1.0).unwrap();
        let sg = r.layers.iter().find(|l| l.kind == "subgraphs").unwrap();
        assert!(sg.commands.iter().any(|c| c.cmd_type == "rect"), "子图框");
        assert!(sg.commands.iter().any(|c| c.text_content.as_deref() == Some("My Group")), "子图标题");
        // 命中区仅节点（子图框不是命中区）
        assert_eq!(s.hit_regions().len(), 3);
    }

    #[test]
    fn test_dotted_edge_dash_survives_full_pipeline() {
        // -.-> 解析为 Dotted → dash [2,3] + round cap
        let mut s = session("flowchart TD\n    A -.-> B");
        let r = s.render(1.0).unwrap();
        let edges = r.layers.iter().find(|l| l.kind == "edges").unwrap();
        let line = edges.commands.iter().find(|c| c.cmd_type == "path" && c.fill.is_none()).unwrap();
        assert_eq!(line.dash, Some(vec![2.0, 3.0]));
        assert_eq!(line.line_cap.as_deref(), Some("round"));
    }

    #[test]
    fn test_thick_edge_width_survives_full_pipeline() {
        let mut s = session("flowchart TD\n    A ==> B");
        let r = s.render(1.0).unwrap();
        let edges = r.layers.iter().find(|l| l.kind == "edges").unwrap();
        let line = edges.commands.iter().find(|c| c.cmd_type == "path").unwrap();
        assert_eq!(line.stroke_width, Some(2.5), "Thick 边 2.5px 过 ABI");
    }

    #[test]
    fn test_arrowhead_renders_via_full_pipeline() {
        let mut s = session("flowchart TD\n    A --> B");
        let r = s.render(1.0).unwrap();
        let edges = r.layers.iter().find(|l| l.kind == "edges").unwrap();
        // 主线 + 实心三角箭头
        assert!(edges.commands.len() >= 2, "主线 + 箭头");
        assert!(
            edges.commands.iter().any(|c| c.cmd_type == "path" && c.fill.is_some()),
            "箭头为独立 fill path 指令",
        );
    }

    #[test]
    fn test_hover_matches_by_id_with_multicmd_node() {
        // DoubleCircle 节点产出 2 条指令（外圆+内圆）— hover 按 cmd.id 命中而非位置序
        let src = "flowchart TD\n    A(((Double))) --> B[Plain]";
        let mut s = session(src);
        let base = s.render(1.0).unwrap();
        let base_outer = fill_head(node_body(&nodes_layer(&base).commands, 0));
        let base_b = fill_head(node_body(&nodes_layer(&base).commands, 1));
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        let r = s.render(1.0).unwrap();
        let cmds = &nodes_layer(&r).commands;
        // 关联聚焦：A-B 相连 → halo+点列追加于节点层尾
        // A(DoubleCircle 卡片族 7:柔影组×2+双圆+bevel) + B(rect 族 4) = 11
        // A(DoubleCircle 3:外圆渐变 + 内圆 + 描边圆) + B(rect 族 2:主体 + bevel)
        assert_eq!(cmds.len(), 5 + PULSE_CMDS_PER_EDGE, "A 卡片族 + B 卡片族 + 脉冲指令");
        assert_ne!(fill_head(node_body(cmds, 0)), base_outer, "hover 命中外圆提亮");
        assert_eq!(fill_head(node_body(cmds, 1)), base_b, "相连邻居 B 不 dim（保持原色）");
        // 脉冲 pill：无 id + translate/rotate 路径跟随轨道(R8)
        for dot in &cmds[5..] {
            assert_eq!(dot.id, None, "脉冲 pill 无命中 id");
            assert_eq!(dot.cmd_type, "rect");
            assert!(dot.corner_radius.is_some(), "全圆角 pill");
            assert!(dot.anims.iter().any(|a| matches!(a.property, WitAnimProperty::TranslateX)));
            assert!(dot.anims.iter().any(|a| matches!(a.property, WitAnimProperty::Rotate)));
        }
    }

    // ─── 关联聚焦（hover 1-hop dim + 相连边脉冲）──────────────

    /// 相连边脉冲点提取（nodes 层尾部的 circle + translate 轨道指令）
    /// 脉冲 pill 指令（rect + translate Loop 轨道;区别于选中荧光的 opacity 轨道）
    fn pulse_dots(r: &WitRenderResult) -> Vec<&WitDrawCmd> {
        nodes_layer(r)
            .commands
            .iter()
            .filter(|c| {
                c.cmd_type == "rect"
                    && c.anims.iter().any(|a| {
                        matches!(a.property, WitAnimProperty::TranslateX)
                            && matches!(a.loop_mode, WitLoopMode::Loop)
                    })
            })
            .collect()
    }

    /// 指令是否携带 dim 轨道（Once opacity 过渡）
    fn dim_anim(cmd: &WitDrawCmd) -> Option<&WitAnimDesc> {
        cmd.anims.iter().find(|a| {
            matches!(a.property, WitAnimProperty::Opacity)
                && matches!(a.loop_mode, WitLoopMode::Once)
        })
    }

    #[test]
    fn test_hover_focus_dims_unrelated_only() {
        // 岛屿 D 与 hover 节点 A 无边相连 → dim；A/B/相连边保持
        let src = "flowchart TD\n    A --> B\n    C --> D";
        let mut s = session(src);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        let r = s.render(1.0).unwrap();
        let nodes = nodes_layer(&r);
        // hit 序 = BTreeMap id 序：A,B,C,D
        let by_id = |i: u32| {
            nodes.commands.iter().find(|c| c.id == Some(i)).unwrap()
        };
        assert!(dim_anim(by_id(0)).is_none(), "hover 节点不 dim");
        assert!(dim_anim(by_id(1)).is_none(), "相连邻居 B 不 dim");
        let d = dim_anim(by_id(2)).expect("无关 C dim");
        let (from, to) = (d.keyframes[0].value, d.keyframes[1].value);
        assert_eq!((from, to), (1.0, FOCUS_DIM_ALPHA as f32), "首次进入:全亮 → dim");
        let d4 = dim_anim(by_id(3)).expect("无关 D dim");
        assert_eq!(d4.keyframes[1].value, FOCUS_DIM_ALPHA as f32);
        // 边：C→D（布局边 1）dim；A→B 不 dim
        let edges = r.layers.iter().find(|l| l.kind == "edges").unwrap();
        let unrelated_line = edges
            .commands
            .iter()
            .find(|c| c.stroke_width.is_some() && dim_anim(c).is_some())
            .expect("无关边主线 dim");
        assert_eq!(dim_anim(unrelated_line).unwrap().keyframes[1].value, FOCUS_DIM_ALPHA as f32);
    }

    #[test]
    fn test_hover_focus_staying_dim_is_constant() {
        // 同一 hover 二次 render：已 dim 指令附着恒值轨道（无重播闪跳）
        let src = "flowchart TD\n    A --> B\n    C --> D";
        let mut s = session(src);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        s.render(1.0).unwrap();
        let r = s.render(1.0).unwrap();
        let nodes = nodes_layer(&r);
        let c_cmd = nodes.commands.iter().find(|c| c.id == Some(2)).unwrap();
        let d = dim_anim(c_cmd).expect("C 保持 dim");
        assert_eq!(
            (d.keyframes[0].value, d.keyframes[1].value),
            (FOCUS_DIM_ALPHA as f32, FOCUS_DIM_ALPHA as f32),
            "持续 dim = 恒值轨道"
        );
    }

    #[test]
    fn test_hover_exit_restores_and_settles_clean() {
        let src = "flowchart TD\n    A --> B\n    C --> D";
        let mut s = session(src);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: None, selected: vec![] });
        let r = s.render(1.0).unwrap();
        let nodes = nodes_layer(&r);
        let c_cmd = nodes.commands.iter().find(|c| c.id == Some(2)).unwrap();
        let d = dim_anim(c_cmd).expect("退场:恢复轨道");
        assert_eq!(
            (d.keyframes[0].value, d.keyframes[1].value),
            (FOCUS_DIM_ALPHA as f32, 1.0),
            "dim → 全亮（宿主 Once 完成后恒等收敛）"
        );
        assert!(pulse_dots(&r).is_empty(), "退场无脉冲");
        // 后续无 hover 渲染:恢复轨道幂等重放(基态稳态克隆 + 同轨道,
        // 不叠加);静止判定在宿主 activity 判据(Once 完成 → 非脏)
        let r2 = s.render(1.0).unwrap();
        assert_eq!(canonical(&r), canonical(&r2), "退场帧幂等(无轨道堆积/跳变)");
        let c_cmd2 = nodes_layer(&r2).commands.iter().find(|c| c.id == Some(2)).unwrap();
        assert!(
            dim_anim(c_cmd2).is_some(),
            "恢复轨道持续在档(视觉已收敛至恒等,宿主 activity 判静止)"
        );
    }

    #[test]
    fn test_pulse_follows_path_with_stagger() {
        let src = "flowchart TD\n    A --> B";
        let mut s = session(src);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        let r = s.render(1.0).unwrap();
        let dots = pulse_dots(&r);
        assert_eq!(dots.len(), PULSE_CMDS_PER_EDGE, "1 条相连边 × (halo + glow + core)");
        // 三层:halo(先,最宽最淡) → glow → core(后,最窄);同 delay 同轨道
        let (halo, glow, core) = (dots[0], dots[1], dots[2]);
        let (hl, hh, _) = PULSE_PILL_HALO;
        let (gl, gh, _) = PULSE_PILL_GLOW;
        let (cl, ch, _) = PULSE_PILL;
        assert!((halo.params[2] - hl).abs() < 1e-6 && (halo.params[3] - hh).abs() < 1e-6, "halo 几何");
        assert!((glow.params[2] - gl).abs() < 1e-6 && (glow.params[3] - gh).abs() < 1e-6, "荧光层几何");
        assert!((core.params[2] - cl).abs() < 1e-6 && (core.params[3] - ch).abs() < 1e-6, "核心 pill 几何");
        assert_eq!(core.corner_radius, Some(ch / 2.0), "全圆角 = pill");
        // R9-2:芯体与边色构造性分离(模式感知荧光色,非边色派生)
        let core_hex = match &core.fill {
            Some(WitPaint::Solid(c)) => c.clone(),
            other => panic!("core fill = solid, got {:?}", other),
        };
        assert!(core_hex.starts_with("rgba("), "芯体带 alpha");
        assert_eq!(core.anims[0].delay_ms, glow.anims[0].delay_ms, "同轨道随行");

        let tx = core.anims.iter().find(|a| matches!(a.property, WitAnimProperty::TranslateX)).unwrap();
        assert!(matches!(tx.loop_mode, WitLoopMode::Loop));
        assert_eq!(tx.duration_ms, PULSE_DURATION_MS);
        // 起点位移 0；TD 布局边垂直 → x 恒 0、y 位移跨越边长
        assert!((tx.keyframes[0].value).abs() < 1e-6, "锚定起点");
        assert!(tx.keyframes.iter().all(|k| k.value.abs() < 1e-6), "垂直边 x 恒 0");
        let ty = core.anims.iter().find(|a| matches!(a.property, WitAnimProperty::TranslateY)).unwrap();
        assert!(ty.keyframes[ty.keyframes.len() - 1].value.abs() > 50.0, "y 位移跨越边长");
        // R8 rotate 轨道:垂直边方向角恒 90°(atan2 空间,y-down)
        let rot = core.anims.iter().find(|a| matches!(a.property, WitAnimProperty::Rotate)).unwrap();
        assert!(
            rot.keyframes.iter().all(|k| (k.value - 90.0).abs() < 1e-3),
            "垂直段 pill 竖向跟随"
        );
        let op = core.anims.iter().find(|a| matches!(a.property, WitAnimProperty::Opacity)).unwrap();
        assert_eq!(op.keyframes.first().unwrap().value, 0.0, "环首淡入");
        assert_eq!(op.keyframes.last().unwrap().value, 0.0, "环尾淡出");
    }

    #[test]
    fn test_sequence_focus_keeps_lifeline_and_message_counterparts() {
        let src = "sequenceDiagram\n    participant A\n    participant B\n    participant C\n    A->>B: m1\n    B->>C: m2";
        let mut s = session(src);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(1), selected: vec![] }); // hover B
        let r = s.render(1.0).unwrap();
        let nodes = nodes_layer(&r);
        let by_id = |i: u32| nodes.commands.iter().find(|c| c.id == Some(i)).unwrap();
        assert!(dim_anim(by_id(0)).is_none(), "A = m1 对端,相关");
        assert!(dim_anim(by_id(1)).is_none(), "hover B 自身");
        assert!(dim_anim(by_id(2)).is_none(), "C = m2 对端,相关");
        // 两条消息都涉及 B → 全部边保持 + 各 3 点脉冲;生命线不产脉冲
        let dots = pulse_dots(&r);
        assert_eq!(dots.len(), PULSE_CMDS_PER_EDGE * 2, "两条相关消息 × (halo + 点列)");
    }

    #[test]
    fn test_sequence_focus_dims_unrelated_lifeline() {
        let src = "sequenceDiagram\n    participant A\n    participant B\n    participant C\n    A->>B: m1";
        let mut s = session(src);
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] }); // hover A
        let r = s.render(1.0).unwrap();
        // C 的生命线（布局边,from=C）应 dim；A/B 生命线不 dim
        let edges = r.layers.iter().find(|l| l.kind == "edges").unwrap();
        let dim_paths = edges
            .commands
            .iter()
            .filter(|c| c.cmd_type == "path" && dim_anim(c).is_some())
            .count();
        assert!(dim_paths >= 1, "无关生命线 dim");
        let dots = pulse_dots(&r);
        assert_eq!(dots.len(), PULSE_CMDS_PER_EDGE, "仅 m1 一条消息产脉冲（生命线无脉冲）");
    }

    #[test]
    fn test_sequence_activation_owner_dim_treatment() {
        // A 无消息：与任何活跃交互无关。hover A → C 的激活框 dim；
        // hover B（m 的对端）→ C 相关 → 激活框保持
        let src = "sequenceDiagram\n    participant A\n    participant B\n    participant C\n    activate C\n    C->>B: m\n    deactivate C";
        let mut s = session(src);
        s.render(1.0).unwrap();
        let act_dim = |r: &WitRenderResult| {
            nodes_layer(r)
                .commands
                .iter()
                .find(|c| c.id.is_none() && c.cmd_type == "rect")
                .and_then(|c| dim_anim(c))
                .is_some()
        };
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] }); // hover A（孤岛）
        let r = s.render(1.0).unwrap();
        assert!(act_dim(&r), "属主 C 与 hover A 无关 → 激活框 dim");
        s.set_state(WitInteractionState { hovered: Some(1), selected: vec![] }); // hover B（m 对端）
        let r2 = s.render(1.0).unwrap();
        let act2 = nodes_layer(&r2)
            .commands
            .iter()
            .find(|c| c.id.is_none() && c.cmd_type == "rect")
            .unwrap();
        // 相关后不再有指向 dim 的轨道(允许恢复轨道:目标恒 1.0)
        let dimming = dim_anim(act2).map(|a| a.keyframes.last().unwrap().value);
        assert_ne!(
            dimming,
            Some(FOCUS_DIM_ALPHA as f32),
            "属主 C 相关 → 激活框恢复(而非持续 dim)"
        );
        // 退场恢复:轨道幂等重放(静止判定在宿主 activity 判据)
        s.set_state(WitInteractionState { hovered: None, selected: vec![] });
        let _ = s.render(1.0).unwrap();
        let clean = s.render(1.0).unwrap();
        assert!(
            clean.layers.iter().all(|l| !l.commands.iter().any(|c| c.anims.len() > 1)),
            "退场后无轨道堆积(每指令至多 1 条恢复轨道)"
        );
    }

    #[test]
    fn test_disable_bakes_dim_without_tier2() {
        let src = "flowchart TD\n    A --> B\n    C --> D";
        let mut s = DiagramSession::new(
            src.to_string(),
            Some(WitDiagramOptions {
                animation: Some(WitAnimationConfig { disable: true, ..Default::default() }),
                ..Default::default()
            }),
        );
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(0), selected: vec![] });
        let r = s.render(1.0).unwrap();
        assert!(pulse_dots(&r).is_empty(), "disable 无脉冲");
        let nodes = nodes_layer(&r);
        let c_cmd = node_body(&nodes.commands, 2);
        assert!(
            fill_head(c_cmd).starts_with("rgba("),
            "disable: dim 直接烘焙进颜色 alpha"
        );
        assert!(c_cmd.anims.is_empty(), "disable: 零 Tier 2 附着");
        let a_cmd = node_body(&nodes.commands, 0);
        assert!(fill_head(a_cmd).starts_with('#'), "hover 节点保持 hex 提亮路径");
    }

    #[test]
    fn test_stale_hover_index_is_defensive() {
        // 源更新后陈旧 hover 索引防御：越界不进入聚焦（不全图误 dim）
        let mut s = session("flowchart TD\n    A --> B");
        s.render(1.0).unwrap();
        s.set_state(WitInteractionState { hovered: Some(99), selected: vec![] });
        let r = s.render(1.0).unwrap();
        assert!(pulse_dots(&r).is_empty());
        assert!(
            r.layers.iter().all(|l| l.commands.iter().all(|c| dim_anim(c).is_none())),
            "越界索引不产生任何 dim 轨道"
        );
    }

#[test]
fn dbg_activation_owner() {
    let src = "sequenceDiagram\n    participant A\n    participant B\n    participant C\n    activate C\n    C->>B: m\n    deactivate C";
    let mut s = session(src);
    let r = s.render(1.0).unwrap();
    let nodes = r.layers.iter().find(|l| l.kind == "nodes").unwrap();
    for (i, c) in nodes.commands.iter().enumerate() {
        eprintln!("pos={} type={} id={:?} fill={}", i, c.cmd_type, c.id, c.fill.is_some());
    }
    let focus = s.focus.clone().unwrap();
    eprintln!("node_ids={:?} activation_owners={:?}", focus.node_ids, focus.activation_owners);
    for (i, e) in focus.edges.iter().enumerate() {
        eprintln!("edge {} from={} to={} lifeline={} slots={:?} label={:?}", i, e.from, e.to, e.is_lifeline, e.cmd_slots, e.label_slot);
    }
}

    #[test]
    fn test_poly_sample_constant_speed() {
        // L 形折线：(0,0)→(100,0)→(100,100)，f=0.25 → (50,0)；f=0.75 → (100,50)
        let pts = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let (x, y) = poly_sample(&pts, 0.25);
        assert!((x - 50.0).abs() < 1e-9 && y.abs() < 1e-9);
        let (x, y) = poly_sample(&pts, 0.75);
        assert!((x - 100.0).abs() < 1e-9 && (y - 50.0).abs() < 1e-9);
        assert_eq!(poly_sample(&pts, 0.0), (0.0, 0.0));
        assert_eq!(poly_sample(&pts, 1.0), (100.0, 100.0));
    }

    #[test]
    fn test_activation_owner_parse() {
        assert_eq!(activation_owner("activation_API_2").as_deref(), Some("API"));
        // participant id 含下划线：末段恒为步号
        assert_eq!(activation_owner("activation_web_app_3").as_deref(), Some("web_app"));
        assert_eq!(activation_owner("plainnode"), None);
    }

    #[test]
    fn test_eval_easing_variants() {
        assert_eq!(eval_easing("linear", 0.25), 0.25);
        assert!((eval_easing("cubic-out", 0.5) - 0.875).abs() < 1e-9);
        assert!((eval_easing("cubic-in", 0.5) - 0.125).abs() < 1e-9);
        assert!((eval_easing("cubic-in-out", 0.5) - 0.5).abs() < 1e-9);
        // quint-out（T22）：1-(1-0.5)^5 = 0.96875 — archify 签名曲线近似
        assert!((eval_easing("quint-out", 0.5) - 0.96875).abs() < 1e-9);
        assert!((eval_easing("quint-out", 0.0) - 0.0).abs() < 1e-9);
        assert!((eval_easing("quint-out", 1.0) - 1.0).abs() < 1e-9);
        assert!((eval_easing("unknown", 0.5) - 0.875).abs() < 1e-9, "缺省 cubic-out");
    }

    // ─── archify 美学（T17/T20-T23）────────────────────────

    fn session_with_preset(source: &str, preset: &str) -> DiagramSession {
        let mut theme = record_to_wit_theme(mermaid_canvas_component::ThemeRecord::default());
        theme.style_preset = Some(preset.to_string());
        let mut s = DiagramSession::new(source.to_string(), None);
        s.set_theme(theme);
        s
    }

    #[test]
    fn test_signal_flow_breathing_anim_attached_to_edges() {
        let mut s = session_with_preset(FLOWCHART, "signal-flow");
        let r = s.render(1.0).unwrap();
        let edges = r.layers.iter().find(|l| l.kind == "edges").unwrap();
        let paths: Vec<&WitDrawCmd> = edges.commands.iter().filter(|c| c.cmd_type == "path").collect();
        assert!(!paths.is_empty());
        for cmd in &paths {
            assert_eq!(cmd.anims.len(), 1, "SignalFlow 边 path 附着呼吸动效");
            let anim = &cmd.anims[0];
            assert!(matches!(anim.property, WitAnimProperty::Opacity));
            assert_eq!(anim.duration_ms, 2400);
            assert!(matches!(anim.loop_mode, WitLoopMode::PingPong));
            assert_eq!(anim.keyframes[0].value, 0.75);
            assert_eq!(anim.keyframes[anim.keyframes.len() - 1].value, 1.0);
        }
        // 非 edges 层不附着
        let nodes = nodes_layer(&r);
        assert!(nodes.commands.iter().all(|c| c.anims.is_empty()));
    }

    #[test]
    fn test_signal_flow_glow_layers_and_gradient_fill() {
        let mut s = session_with_preset(FLOWCHART, "signal-flow");
        let r = s.render(1.0).unwrap();
        // 辉光（R7 四层扩散）：主线前的半透明描边族（rgba + 递增宽度）
        let edges = r.layers.iter().find(|l| l.kind == "edges").unwrap();
        let stroked: Vec<&WitDrawCmd> = edges.commands.iter().filter(|c| c.stroke.is_some()).collect();
        assert!(stroked.len() >= 5, "4 辉光层 + 主线");
        let widths: Vec<Option<f64>> = stroked.iter().map(|c| c.stroke_width).collect();
        // R9 收紧档:6.5/4.8/3.2/2.0(preset.glow_layers 契约)
        assert!(
            widths.contains(&Some(6.5)) && widths.contains(&Some(4.8))
                && widths.contains(&Some(3.2)) && widths.contains(&Some(2.0))
        );
        // 渐变填充：节点主体 fill 为 Gradient（垂直;跳过无填充的柔影/bevel）
        let rect = nodes_layer(&r)
            .commands
            .iter()
            .find(|c| c.cmd_type == "rect" && c.fill.is_some())
            .unwrap();
        match &rect.fill {
            Some(WitPaint::Gradient(g)) => {
                assert!((g.y1 - g.y0).abs() > 0.0, "垂直渐变");
                assert_eq!(g.stops.len(), 2);
            }
            other => panic!("SignalFlow 节点应渐变填充, got {:?}", other),
        }
    }

    // ─── R10 软阴影 + 入场家族同步 ───────────────────────────────

    /// paint 字符串的 alpha 通道（rgba(...)；渐变取首个 stop）；hex 返回 1.0
    fn paint_alpha_of(paint: &Option<WitPaint>) -> f64 {
        let c = match paint {
            Some(WitPaint::Solid(c)) => c.clone(),
            Some(WitPaint::Gradient(g)) => match g.stops.first() {
                Some(s) => s.color.clone(),
                None => return 1.0,
            },
            None => return 1.0,
        };
        let lo = c.rfind(',').map(|i| i + 1).unwrap_or(0);
        c[lo..].trim_end_matches(')').trim().parse().unwrap_or(1.0)
    }

    /// 指令的有效 alpha：填充优先，无填充取描边（bevel 为 stroke-only）
    fn effective_alpha_of(cmd: &WitDrawCmd) -> f64 {
        if cmd.fill.is_some() {
            paint_alpha_of(&cmd.fill)
        } else {
            paint_alpha_of(&cmd.stroke)
        }
    }

    #[test]
    fn test_soft_shadow_rides_body_command() {
        // SignalFlow 携带柔影 → shadow 挂在主体指令上(不再有独立阴影指令)
        let mut s = session_with_preset(FLOWCHART, "signal-flow");
        let steady = s.render(1.0).unwrap();
        let nodes = nodes_layer(&steady);
        let bodies: Vec<&WitDrawCmd> = nodes
            .commands
            .iter()
            .filter(|c| c.cmd_type != "text" && c.fill.is_some())
            .collect();
        assert_eq!(bodies.len(), 3, "3 节点主体(rect/diamond 路径/cylinder 路径)");
        let with_shadow: Vec<&WitShadowDesc> =
            bodies.iter().filter_map(|c| c.shadow.as_ref()).collect();
        assert_eq!(with_shadow.len(), 3, "3 节点主体均携带 shadow 字段");
        for sh in &with_shadow {
            assert!((sh.alpha - 0.20).abs() < 1e-9, "SignalFlow 档位 alpha 0.20");
            assert!(sh.blur >= 6.0, "高模糊 = 柔影而非硬边");
            assert!(sh.offset_y > 0.0, "向下投影");
        }
    }

    #[test]
    fn test_entrance_shadow_alpha_follows_body() {
        // 入场中:阴影 alpha = 稳态 alpha × 主体相位(阴影不领先于形状出现)
        let mut s = session_with_preset(FLOWCHART, "signal-flow");
        let early = s.render(0.35).unwrap();
        let nodes = nodes_layer(&early);
        let mut checked = 0;
        for cmd in nodes.commands.iter() {
            if let Some(sh) = &cmd.shadow {
                let body_alpha = paint_alpha_of(&cmd.fill);
                let expected = 0.20 * body_alpha;
                assert!(
                    (sh.alpha - expected).abs() < 0.02,
                    "阴影 alpha({}) 应随主体相位({}) 缩放",
                    sh.alpha,
                    body_alpha,
                );
                assert!(
                    (0.85 * 10.0..10.0).contains(&sh.blur),
                    "入场中阴影柔度保持(下限 0.85 档),不随几何锐化: {}",
                    sh.blur
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 3, "3 节点主体入场中均被检查");
    }

    #[test]
    fn test_entrance_family_phase_sync_within_node() {
        // classic 带 stagger:同 id 家族(主体 + bevel)必须同相位 —— bevel
        // alpha(基 0.28) / 主体 alpha(基 1.0) 恒等于 0.28;相位分裂时比值偏离
        let src = "flowchart TD\n    A[One] --> B[Two]\n    B --> C[Three]";
        let mut s = session_with_preset(src, "classic");
        let early = s.render(0.30).unwrap();
        let nodes = nodes_layer(&early);
        let mut families: std::collections::BTreeMap<u32, Vec<f64>> = Default::default();
        for cmd in nodes.commands.iter() {
            if let Some(id) = cmd.id {
                families.entry(id).or_default().push(effective_alpha_of(cmd));
            }
        }
        // body(基 1.0)与 bevel(基 0.28)的比值锁定 = 同相位
        for (id, alphas) in families {
            assert!(alphas.len() >= 2, "节点 {} 至少主体+bevel 两个带填充指令", id);
            let ratio = alphas[1] / alphas[0];
            assert!(
                (ratio - 0.28).abs() < 0.02,
                "节点 {} 家族相位分裂:比值 {} ≠ 0.28",
                id,
                ratio
            );
        }
    }

    #[test]
    fn test_disable_animation_suppresses_breathing() {
        let opts = WitDiagramOptions {
            animation: Some(WitAnimationConfig { disable: true, ..Default::default() }),
            ..Default::default()
        };
        let mut base = DiagramSession::new(FLOWCHART.to_string(), Some(opts));
        let mut theme = record_to_wit_theme(mermaid_canvas_component::ThemeRecord::default());
        theme.style_preset = Some("signal-flow".to_string());
        base.set_theme(theme);
        let r = base.render(1.0).unwrap();
        assert!(
            !r.layers.iter().any(|l| l.commands.iter().any(|c| !c.anims.is_empty())),
            "disable 时 SignalFlow 也不附着",
        );
    }

    #[test]
    fn test_blueprint_preset_grid_and_no_stagger() {
        // 全矩形 fixture（无 sigil 指令干扰面积断言；分行声明 — 链式行需 ≥3 箭头才拆分）
        let src = "flowchart TD\n    A[One] --> B[Two]\n    B[Two] --> C[Three]";

        fn rect_areas(r: &WitRenderResult) -> Vec<f64> {
            nodes_layer(r).commands.iter().filter(|c| c.cmd_type == "rect").map(area_of).collect()
        }

        // Blueprint 无 stagger：入场早期所有节点相位一致（各自面积相对稳态的比例相同）
        let mut s = session_with_preset(src, "blueprint");
        let early = s.render(0.5).unwrap();
        let mut s2 = session_with_preset(src, "blueprint");
        let steady = s2.render(1.0).unwrap();
        let early_areas = rect_areas(&early);
        let steady_areas = rect_areas(&steady);
        assert_eq!(early_areas.len(), 3);
        let ratios: Vec<f64> = early_areas.iter().zip(&steady_areas).map(|(e, st)| e / st).collect();
        assert!(
            ratios.iter().all(|r| (r - ratios[0]).abs() < 1e-6),
            "Blueprint 全节点同步入场（比例一致）: {:?}",
            ratios,
        );

        // 背景层第二指令 = 网格 path（多段竖横线）
        let r = s.render(1.0).unwrap();
        let bg = r.layers.iter().find(|l| l.kind == "background").unwrap();
        assert!(bg.commands.len() >= 2, "背景 + 网格");
        assert_eq!(bg.commands[1].cmd_type, "path", "网格为 path");
    }

    #[test]
    fn test_signal_flow_stagger_is_finer_than_classic() {
        // 构造 ≥3 节点流（分行声明），验证 stagger 因子的相位数学关系
        let src = "flowchart TD\n    A --> B\n    B --> C";
        let mut classic = DiagramSession::new(src.to_string(), None);
        let mut flow = session_with_preset(src, "signal-flow");
        let _ = classic.render(0.5).unwrap();
        let _ = flow.render(0.5).unwrap();
        // 主要验证 stagger 因子生效：quint-out 未指定时两档 easing 相同，
        // 差异来自 delay — 首节点 idx=0 delay 恒 0，改验证 item_phase 数学关系
        let s = session(src);
        let t = 0.5;
        // classic: idx 2 delay = 2*24/500 = 0.096 → x = (0.5-0.096)/0.6 = 0.673
        let classic_phase = s.item_phase(t, 2);
        let mut flow_s = session_with_preset(src, "signal-flow");
        let _ = flow_s.render(1.0);
        let flow_phase = flow_s.item_phase(t, 2);
        assert!(flow_phase > classic_phase, "SignalFlow stagger 更细腻（相位提前）: {} > {}", flow_phase, classic_phase);
    }

    #[test]
    fn test_hover_declaration_per_preset() {
        // Classic → brighten 0.12(R7 增强)
        let mut s = session(FLOWCHART);
        let regions = s.hit_regions();
        assert!(!regions.is_empty());
        assert!(regions.iter().all(|r| r.hover.as_ref().unwrap().kind == "brighten"));
        assert_eq!(regions[0].hover.as_ref().unwrap().params, vec![0.12]);

        // SignalFlow → glow 0.6(R7 荧光)
        let mut s = session_with_preset(FLOWCHART, "signal-flow");
        let regions = s.hit_regions();
        assert!(regions.iter().all(|r| r.hover.as_ref().unwrap().kind == "glow"));
        assert_eq!(regions[0].hover.as_ref().unwrap().params, vec![0.6]);

        // Blueprint → outline 1.5
        let mut s = session_with_preset(FLOWCHART, "blueprint");
        let regions = s.hit_regions();
        assert!(regions.iter().all(|r| r.hover.as_ref().unwrap().kind == "outline"));
        assert_eq!(regions[0].hover.as_ref().unwrap().params, vec![1.5]);
    }

    #[test]
    fn test_editorial_preset_larger_radius_and_font() {
        let mut s = session_with_preset("flowchart TD\n    A(Round) --> B[Plain]", "editorial");
        let r = s.render(1.0).unwrap();
        let round = nodes_layer(&r).commands.iter().find(|c| c.cmd_type == "rect").unwrap();
        assert_eq!(round.corner_radius, Some(10.0), "Editorial 圆角 10px");
        // 字号 +1（base 14 → 15）
        let labels = r.layers.iter().find(|l| l.kind == "labels").unwrap();
        let text = labels.commands.iter().find(|c| c.cmd_type == "text").unwrap();
        assert_eq!(text.params[2], 15.0, "Editorial 节点字号 +1");
    }
}
