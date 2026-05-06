//! 布局配置

/// 布局配置
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// 节点水平间距
    pub node_spacing: f64,
    /// 层级间距
    pub rank_spacing: f64,
    /// 节点水平内边距
    pub node_padding_x: f64,
    /// 节点垂直内边距
    pub node_padding_y: f64,
    /// 标签行高
    pub label_line_height: f64,
    /// 最大标签宽度（字符数）
    pub max_label_width_chars: usize,
    /// 目标宽高比
    pub preferred_aspect_ratio: Option<f64>,
    /// 路由网格单元大小
    pub routing_grid_cell: f64,
    /// 路由转向惩罚
    pub routing_turn_penalty: f64,
    /// 排名迭代次数
    pub ranking_passes: usize,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            node_spacing: 60.0,
            rank_spacing: 80.0,
            node_padding_x: 20.0,
            node_padding_y: 12.0,
            label_line_height: 1.4,
            max_label_width_chars: 40,
            preferred_aspect_ratio: None,
            routing_grid_cell: 16.0,
            routing_turn_penalty: 0.6,
            ranking_passes: 4,
        }
    }
}
