//! 交互类型定义
//!
//! 提供命中测试和坐标查找相关类型。

/// 包围盒
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// x 坐标
    pub x: f64,
    /// y 坐标
    pub y: f64,
    /// 宽度
    pub width: f64,
    /// 高度
    pub height: f64,
}

impl BoundingBox {
    /// 创建新的包围盒
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// 扩展包围盒
    pub fn expand(&self, padding: f64) -> Self {
        Self {
            x: self.x - padding,
            y: self.y - padding,
            width: self.width + 2.0 * padding,
            height: self.height + 2.0 * padding,
        }
    }

    /// 检查点是否在包围盒内
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

/// 命中区域
#[derive(Debug, Clone, PartialEq)]
pub struct HitRegion {
    /// 区域索引
    pub index: usize,
    /// 系列索引
    pub series: Option<usize>,
    /// 包围盒
    pub bounds: BoundingBox,
}

/// 命中测试结果
#[derive(Debug, Clone, PartialEq)]
pub struct HitResult {
    /// 命中的区域索引
    pub index: usize,
    /// 命中的系列
    pub series: Option<usize>,
}
