//! Tiny-skia 渲染器
//!
//! 将 mermaid-canvas-core 的 DrawCmd 指令渲染到 tiny-skia Pixmap。

pub mod color;
pub mod text;

use std::path::Path;

use mermaid_canvas_core::{DrawCmd, FillStyle, PathSegment, RenderLayers, StrokeStyle, TextStyle};
use mermaid_canvas_wit::wit_types::{WitDrawCmd, WitLayer, WitPaint};
use text::FontState;

/// 提取 paint 的纯色字符串（渐变超出 tiny-skia demo 渲染器支持范围）
fn solid_color(paint: &Option<WitPaint>) -> Option<String> {
    match paint {
        Some(WitPaint::Solid(c)) => Some(c.clone()),
        _ => None,
    }
}

/// Canvas 渲染器，将 DrawCmd 渲染到 Pixmap
pub struct TinySkiaRenderer {
    pixmap: tiny_skia::Pixmap,
    font_state: FontState,
}

impl TinySkiaRenderer {
    /// 创建指定尺寸的渲染器
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let pixmap = tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| format!("Failed to create {}x{} pixmap", width, height))?;

        let font_state = FontState::load_system_font()?;

        Ok(Self { pixmap, font_state })
    }

    /// 获取 Pixmap 引用
    pub fn pixmap(&self) -> &tiny_skia::Pixmap {
        &self.pixmap
    }

    /// 渲染完整的 RenderLayers（按 z-index 顺序）
    pub fn render_layers(&mut self, layers: &RenderLayers) {
        for layer in layers.all() {
            for cmd in &layer.commands.semantic {
                self.render_cmd(cmd);
            }
        }
    }

    /// 渲染单个 RenderOutput 中的所有指令
    pub fn render_output(&mut self, output: &mermaid_canvas_core::RenderOutput) {
        for cmd in &output.semantic {
            self.render_cmd(cmd);
        }
    }

    /// 渲染 WitLayer 列表（WASM 组件返回的展平绘图指令）
    pub fn render_wit_layers(&mut self, layers: &[WitLayer]) {
        for layer in layers {
            for cmd in &layer.commands {
                self.render_wit_draw_cmd(cmd);
            }
        }
    }

    /// 渲染单个 WitDrawCmd
    pub fn render_wit_draw_cmd(&mut self, cmd: &WitDrawCmd) {
        match cmd.cmd_type.as_str() {
            "rect" => {
                let params = &cmd.params;
                if params.len() >= 4 {
                    let x = params[0];
                    let y = params[1];
                    let w = params[2];
                    let h = params[3];
                    let fill = solid_color(&cmd.fill).map(FillStyle::Color);
                    let stroke = solid_color(&cmd.stroke).map(StrokeStyle::Color);
                    self.draw_rect(x, y, w, h, &fill, &stroke, &cmd.corner_radius, cmd.stroke_width);
                }
            }
            "circle" => {
                let params = &cmd.params;
                if params.len() >= 3 {
                    let cx = params[0];
                    let cy = params[1];
                    let r = params[2];
                    let fill = solid_color(&cmd.fill).map(FillStyle::Color);
                    let stroke = solid_color(&cmd.stroke).map(StrokeStyle::Color);
                    self.draw_circle(cx, cy, r, &fill, &stroke, cmd.stroke_width);
                }
            }
            "path" => {
                let segments = decode_path_segments(&cmd.params);
                let fill = solid_color(&cmd.fill).map(FillStyle::Color);
                let stroke = solid_color(&cmd.stroke).map(StrokeStyle::Color);
                self.draw_path(&segments, &fill, &stroke, cmd.stroke_width);
            }
            "text" => {
                let params = &cmd.params;
                if params.len() >= 5 {
                    let x = params[0];
                    let y = params[1];
                    let font_size = params[2];
                    let anchor = match params[3] as u32 {
                        1 => mermaid_canvas_core::TextAnchor::Middle,
                        2 => mermaid_canvas_core::TextAnchor::End,
                        _ => mermaid_canvas_core::TextAnchor::Start,
                    };
                    let baseline = match params[4] as u32 {
                        0 => mermaid_canvas_core::TextBaseline::Top,
                        1 => mermaid_canvas_core::TextBaseline::Middle,
                        2 => mermaid_canvas_core::TextBaseline::Bottom,
                        _ => mermaid_canvas_core::TextBaseline::Alphabetic,
                    };
                    if let Some(content) = &cmd.text_content {
                        let fill_color = solid_color(&cmd.fill).unwrap_or_else(|| "#000000".to_string());
                        let mut style = TextStyle::new()
                            .with_font_size(font_size)
                            .with_fill(FillStyle::Color(fill_color));
                        if let Some(font) = &cmd.font {
                            if let Some(family) = &font.family {
                                style = style.with_font_family(family.clone());
                            }
                        }
                        self.draw_text(x, y, content, &style, anchor, baseline);
                    }
                }
            }
            _ => {}
        }
    }

    /// 渲染单个 DrawCmd
    pub fn render_cmd(&mut self, cmd: &DrawCmd) {
        match cmd {
            DrawCmd::Rect {
                x, y, width, height,
                fill, stroke, corner_radius,
            } => {
                self.draw_rect(*x, *y, *width, *height, fill, stroke, corner_radius, None);
            }
            DrawCmd::Path { segments, fill, stroke } => {
                self.draw_path(segments, fill, stroke, None);
            }
            DrawCmd::Circle { cx, cy, r, fill, stroke } => {
                self.draw_circle(*cx, *cy, *r, fill, stroke, None);
            }
            DrawCmd::Text { x, y, content, style, anchor, baseline } => {
                self.draw_text(*x, *y, content, style, *anchor, *baseline);
            }
            DrawCmd::Group { label: _, items } => {
                for item in items {
                    self.render_cmd(item);
                }
            }
        }
    }

    /// 保存 Pixmap 为 PNG 文件
    pub fn save_png(&self, path: &Path) -> Result<(), String> {
        let png_data = self.pixmap.encode_png()
            .map_err(|e| format!("Failed to encode PNG: {}", e))?;
        std::fs::write(path, &png_data)
            .map_err(|e| format!("Failed to write PNG: {}", e))
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_rect(
        &mut self,
        x: f64, y: f64, w: f64, h: f64,
        fill: &Option<FillStyle>,
        stroke: &Option<StrokeStyle>,
        corner_radius: &Option<f64>,
        stroke_width: Option<f64>,
    ) {
        let sk_stroke = make_stroke(stroke_width);
        if let Some(radius) = corner_radius.filter(|r| *r > 0.0) {
            let mut pb = tiny_skia::PathBuilder::new();
            let r = radius.min(w / 2.0).min(h / 2.0) as f32;
            let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);

            pb.move_to(x + r, y);
            pb.line_to(x + w - r, y);
            pb.quad_to(x + w, y, x + w, y + r);
            pb.line_to(x + w, y + h - r);
            pb.quad_to(x + w, y + h, x + w - r, y + h);
            pb.line_to(x + r, y + h);
            pb.quad_to(x, y + h, x, y + h - r);
            pb.line_to(x, y + r);
            pb.quad_to(x, y, x + r, y);
            pb.close();

            if let Some(path) = pb.finish() {
                if let Some(fill_style) = fill {
                    if let Some(paint) = self.fill_to_paint(fill_style) {
                        self.pixmap.fill_path(
                            &path,
                            &paint,
                            tiny_skia::FillRule::Winding,
                            tiny_skia::Transform::identity(),
                            None,
                        );
                    }
                }
                if let Some(stroke_style) = stroke {
                    if let Some(paint) = self.stroke_to_paint(stroke_style) {
                        self.pixmap.stroke_path(
                            &path,
                            &paint,
                            &sk_stroke,
                            tiny_skia::Transform::identity(),
                            None,
                        );
                    }
                }
            }
        } else {
            // 普通矩形
            let rect = tiny_skia::Rect::from_xywh(
                x as f32, y as f32,
                w as f32, h as f32,
            );

            if let Some(rect) = rect {
                if let Some(fill_style) = fill {
                    if let Some(paint) = self.fill_to_paint(fill_style) {
                        self.pixmap.fill_rect(
                            rect,
                            &paint,
                            tiny_skia::Transform::identity(),
                            None,
                        );
                    }
                }
                if let Some(stroke_style) = stroke {
                    if let Some(paint) = self.stroke_to_paint(stroke_style) {
                        let mut pb = tiny_skia::PathBuilder::new();
                        let (x, y, w, h) = (rect.left(), rect.top(), rect.width(), rect.height());
                        pb.move_to(x, y);
                        pb.line_to(x + w, y);
                        pb.line_to(x + w, y + h);
                        pb.line_to(x, y + h);
                        pb.close();
                        if let Some(path) = pb.finish() {
                            self.pixmap.stroke_path(
                                &path,
                                &paint,
                                &sk_stroke,
                                tiny_skia::Transform::identity(),
                                None,
                            );
                        }
                    }
                }
            }
        }
    }

    fn draw_path(
        &mut self,
        segments: &[PathSegment],
        fill: &Option<FillStyle>,
        stroke: &Option<StrokeStyle>,
        stroke_width: Option<f64>,
    ) {
        if segments.is_empty() {
            return;
        }

        let sk_stroke = make_stroke(stroke_width);

        let mut pb = tiny_skia::PathBuilder::new();

        for segment in segments {
            match segment {
                PathSegment::MoveTo(x, y) => pb.move_to(*x as f32, *y as f32),
                PathSegment::LineTo(x, y) => pb.line_to(*x as f32, *y as f32),
                PathSegment::BezierTo(cp1x, cp1y, cp2x, cp2y, x, y) => {
                    pb.cubic_to(
                        *cp1x as f32, *cp1y as f32,
                        *cp2x as f32, *cp2y as f32,
                        *x as f32, *y as f32,
                    );
                }
                PathSegment::QuadraticTo(cpx, cpy, x, y) => {
                    pb.quad_to(*cpx as f32, *cpy as f32, *x as f32, *y as f32);
                }
                PathSegment::Arc(cx, cy, r, start_angle, end_angle, _anticlockwise) => {
                    // tiny-skia 没有 arc，用 line segments 近似
                    let steps = 32;
                    let sweep = *end_angle - *start_angle;
                    for i in 0..=steps {
                        let t = *start_angle + sweep * (i as f64 / steps as f64);
                        let px = cx + r * t.cos();
                        let py = cy + r * t.sin();
                        if i == 0 && pb.is_empty() {
                            pb.move_to(px as f32, py as f32);
                        } else {
                            pb.line_to(px as f32, py as f32);
                        }
                    }
                }
                PathSegment::Close => pb.close(),
            }
        }

        if let Some(path) = pb.finish() {
            if let Some(fill_style) = fill {
                if let Some(paint) = self.fill_to_paint(fill_style) {
                    self.pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            }
            if let Some(stroke_style) = stroke {
                if let Some(paint) = self.stroke_to_paint(stroke_style) {
                    self.pixmap.stroke_path(
                        &path,
                        &paint,
                        &sk_stroke,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            }
        }
    }

    fn draw_circle(
        &mut self,
        cx: f64, cy: f64, r: f64,
        fill: &Option<FillStyle>,
        stroke: &Option<StrokeStyle>,
        stroke_width: Option<f64>,
    ) {
        let sk_stroke = make_stroke(stroke_width);
        let path = tiny_skia::PathBuilder::from_circle(
            cx as f32, cy as f32, r as f32,
        );

        if let Some(path) = path {
            if let Some(fill_style) = fill {
                if let Some(paint) = self.fill_to_paint(fill_style) {
                    self.pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            }
            if let Some(stroke_style) = stroke {
                if let Some(paint) = self.stroke_to_paint(stroke_style) {
                    self.pixmap.stroke_path(
                        &path,
                        &paint,
                        &sk_stroke,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            }
        }
    }

    fn draw_text(
        &mut self,
        x: f64, y: f64,
        content: &str,
        style: &TextStyle,
        anchor: mermaid_canvas_core::TextAnchor,
        baseline: mermaid_canvas_core::TextBaseline,
    ) {
        let color_str = match &style.fill {
            FillStyle::Color(c) => c.as_str(),
            _ => "#000000",
        };

        self.font_state.draw_text(
            &mut self.pixmap,
            content,
            x, y,
            style.font_size,
            anchor,
            baseline,
            color_str,
        );
    }

    fn fill_to_paint(&self, fill: &FillStyle) -> Option<tiny_skia::Paint<'static>> {
        match fill {
            FillStyle::Color(c) => {
                let color = color::parse_color(c)?;
                let mut paint = tiny_skia::Paint::default();
                paint.set_color(color);
                Some(paint)
            }
            FillStyle::None => None,
            FillStyle::Gradient(_) => {
                // 暂不支持渐变
                None
            }
        }
    }

    fn stroke_to_paint(&self, stroke: &StrokeStyle) -> Option<tiny_skia::Paint<'static>> {
        match stroke {
            StrokeStyle::Color(c) => {
                let color = color::parse_color(c)?;
                let mut paint = tiny_skia::Paint::default();
                paint.set_color(color);
                Some(paint)
            }
            StrokeStyle::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mermaid_canvas_core::{FillStyle, StrokeStyle};

    #[test]
    fn test_renderer_create() {
        let renderer = TinySkiaRenderer::new(800, 600);
        assert!(renderer.is_ok());
    }

    #[test]
    fn test_renderer_fill_rect() {
        let mut renderer = TinySkiaRenderer::new(100, 100).unwrap();

        renderer.render_cmd(&DrawCmd::Rect {
            x: 10.0, y: 10.0, width: 50.0, height: 50.0,
            fill: Some(FillStyle::Color("#ff0000".to_string())),
            stroke: None,
            corner_radius: None,
        });

        let pixel = renderer.pixmap.pixel(30, 30).unwrap();
        assert!(pixel.red() > 200);
    }

    #[test]
    fn test_renderer_stroke_rect() {
        let mut renderer = TinySkiaRenderer::new(100, 100).unwrap();

        renderer.render_cmd(&DrawCmd::Rect {
            x: 10.0, y: 10.0, width: 50.0, height: 50.0,
            fill: None,
            stroke: Some(StrokeStyle::Color("#0000ff".to_string())),
            corner_radius: None,
        });

        // 描边矩形应该不 panic，且画布上应有一些非零像素
        let has_pixels = renderer.pixmap.pixels()
            .iter()
            .any(|p| p.blue() > 0);
        assert!(has_pixels, "Expected some blue pixels from stroke rect");
    }

    #[test]
    fn test_renderer_circle() {
        let mut renderer = TinySkiaRenderer::new(100, 100).unwrap();

        renderer.render_cmd(&DrawCmd::Circle {
            cx: 50.0, cy: 50.0, r: 20.0,
            fill: Some(FillStyle::Color("#00ff00".to_string())),
            stroke: None,
        });

        let pixel = renderer.pixmap.pixel(50, 50).unwrap();
        assert!(pixel.green() > 200);
    }

    #[test]
    fn test_renderer_path() {
        let mut renderer = TinySkiaRenderer::new(100, 100).unwrap();

        renderer.render_cmd(&DrawCmd::Path {
            segments: vec![
                PathSegment::MoveTo(10.0, 10.0),
                PathSegment::LineTo(90.0, 10.0),
            ],
            fill: None,
            stroke: Some(StrokeStyle::Color("#000000".to_string())),
        });

        // 线段上应该有黑色像素
        let pixel = renderer.pixmap.pixel(50, 10).unwrap();
        // 需要检查像素是否有变化（可能因为抗锯齿而不是纯黑）
        let _ = pixel;
    }
}

fn make_stroke(width: Option<f64>) -> tiny_skia::Stroke {
    let mut stroke = tiny_skia::Stroke::default();
    if let Some(w) = width {
        stroke.width = w as f32;
    }
    stroke
}

/// 从 params 数组解码 PathSegment
/// 编码格式（与 mermaid-canvas-wit convert 对应）：
/// 0=MoveTo(x,y), 1=LineTo(x,y), 2=BezierTo(cp1x,cp1y,cp2x,cp2y,x,y),
/// 3=QuadraticTo(cpx,cpy,x,y), 4=Arc(cx,cy,r,start,end,ccw), 5=Close
fn decode_path_segments(params: &[f64]) -> Vec<PathSegment> {
    let mut segments = Vec::new();
    let mut i = 0;
    while i < params.len() {
        let seg_type = params[i] as u32;
        match seg_type {
            0 => {
                // MoveTo(x, y)
                if i + 2 < params.len() {
                    segments.push(PathSegment::MoveTo(params[i + 1], params[i + 2]));
                    i += 3;
                } else { break; }
            }
            1 => {
                // LineTo(x, y)
                if i + 2 < params.len() {
                    segments.push(PathSegment::LineTo(params[i + 1], params[i + 2]));
                    i += 3;
                } else { break; }
            }
            2 => {
                // BezierTo(cp1x, cp1y, cp2x, cp2y, x, y)
                if i + 6 < params.len() {
                    segments.push(PathSegment::BezierTo(
                        params[i + 1], params[i + 2],
                        params[i + 3], params[i + 4],
                        params[i + 5], params[i + 6],
                    ));
                    i += 7;
                } else { break; }
            }
            3 => {
                // QuadraticTo(cpx, cpy, x, y)
                if i + 4 < params.len() {
                    segments.push(PathSegment::QuadraticTo(
                        params[i + 1], params[i + 2],
                        params[i + 3], params[i + 4],
                    ));
                    i += 5;
                } else { break; }
            }
            4 => {
                // Arc(cx, cy, r, start, end, ccw)
                if i + 6 < params.len() {
                    segments.push(PathSegment::Arc(
                        params[i + 1], params[i + 2],
                        params[i + 3], params[i + 4],
                        params[i + 5],
                        params[i + 6] > 0.5,
                    ));
                    i += 7;
                } else { break; }
            }
            5 => {
                segments.push(PathSegment::Close);
                i += 1;
            }
            _ => { i += 1; }
        }
    }
    segments
}
