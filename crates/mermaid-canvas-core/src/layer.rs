//! 渲染层系统
//!
//! 提供分层渲染能力，支持脏标记和增量更新。
//! Mermaid 图表特化的图层：Background, Subgraphs, Edges, Nodes, Labels, Title, Annotations。

use crate::instruction::RenderOutput;

/// 层类型 — Mermaid 图表特化
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerKind {
    /// 背景层 (z-index: 0)
    Background,
    /// 子图层 (z-index: 1)
    Subgraphs,
    /// 边/连线层 (z-index: 2)
    Edges,
    /// 节点层 (z-index: 3)
    Nodes,
    /// 标签层 (z-index: 4)
    Labels,
    /// 标题层 (z-index: 5)
    Title,
    /// 标注层 (z-index: 6)
    Annotations,
}

impl LayerKind {
    /// 获取默认的 z-index
    pub fn default_z_index(&self) -> u32 {
        match self {
            LayerKind::Background => 0,
            LayerKind::Subgraphs => 1,
            LayerKind::Edges => 2,
            LayerKind::Nodes => 3,
            LayerKind::Labels => 4,
            LayerKind::Title => 5,
            LayerKind::Annotations => 6,
        }
    }

    /// 获取所有标准层
    pub fn all_standard_kinds() -> Vec<Self> {
        vec![
            Self::Background,
            Self::Subgraphs,
            Self::Edges,
            Self::Nodes,
            Self::Labels,
            Self::Title,
            Self::Annotations,
        ]
    }
}

/// 渲染层
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// 层类型
    pub kind: LayerKind,
    /// 是否脏（需要重新渲染）
    pub dirty: bool,
    /// 渲染指令
    pub commands: RenderOutput,
    /// z-index
    pub z_index: u32,
}

impl Layer {
    /// 创建新层
    pub fn new(kind: LayerKind) -> Self {
        Self {
            kind,
            dirty: true,
            commands: RenderOutput::new(),
            z_index: kind.default_z_index(),
        }
    }

    /// 标记为脏
    pub fn mark_dirty(&mut self) { self.dirty = true; }
    /// 标记为干净
    pub fn mark_clean(&mut self) { self.dirty = false; }

    /// 更新渲染指令
    pub fn update_commands(&mut self, commands: RenderOutput) {
        self.commands = commands;
        self.dirty = true;
    }

    /// 清空渲染指令
    pub fn clear(&mut self) {
        self.commands.clear();
        self.dirty = true;
    }
}

impl From<LayerKind> for Layer {
    fn from(kind: LayerKind) -> Self { Self::new(kind) }
}

/// 渲染层集合
#[derive(Debug, Clone, PartialEq)]
pub struct RenderLayers {
    layers: Vec<Layer>,
}

impl RenderLayers {
    /// 创建新的渲染层集合（包含所有标准层）
    pub fn new() -> Self {
        Self {
            layers: LayerKind::all_standard_kinds().into_iter().map(Layer::from).collect(),
        }
    }

    /// 创建空的渲染层集合
    pub fn empty() -> Self { Self { layers: Vec::new() } }

    /// 获取所有层
    pub fn all(&self) -> &[Layer] { &self.layers }

    /// 获取脏层迭代器
    pub fn dirty_layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.iter().filter(|l| l.dirty)
    }

    /// 标记指定层为脏
    pub fn mark_dirty(&mut self, kind: LayerKind) {
        if let Some(l) = self.get_layer_mut(kind) { l.mark_dirty(); }
    }

    /// 标记所有层为脏
    pub fn mark_all_dirty(&mut self) {
        for l in &mut self.layers { l.mark_dirty(); }
    }

    /// 标记所有层为干净
    pub fn mark_all_clean(&mut self) {
        for l in &mut self.layers { l.mark_clean(); }
    }

    /// 获取指定类型的层
    pub fn get_layer(&self, kind: LayerKind) -> Option<&Layer> {
        self.layers.iter().find(|l| l.kind == kind)
    }

    /// 获取可变的指定类型层
    pub fn get_layer_mut(&mut self, kind: LayerKind) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.kind == kind)
    }

    /// 更新指定层的渲染指令
    pub fn update_layer(&mut self, kind: LayerKind, commands: RenderOutput) {
        if let Some(l) = self.get_layer_mut(kind) { l.update_commands(commands); }
    }

    /// 判断是否有脏层
    pub fn has_dirty_layers(&self) -> bool { self.layers.iter().any(|l| l.dirty) }

    /// 清空所有层
    pub fn clear_all(&mut self) { for l in &mut self.layers { l.clear(); } }
}

impl Default for RenderLayers {
    fn default() -> Self { Self::new() }
}
