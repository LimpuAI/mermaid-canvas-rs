//! ab_glyph 文本渲染
//!
//! 使用 ab_glyph (rusttype 继任者) 进行字形轮廓渲染。
//! 核心区别：所有字形共享同一基线，outline_glyph 自动处理
//! 亚像素定位，draw() 回调直接给出相对于 px_bounds 的像素坐标。

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use mermaid_canvas_core::{TextAnchor, TextBaseline};

/// 字体状态
pub struct FontState {
    font: FontVec,
}

impl FontState {
    /// 从字体字节创建
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let font = FontVec::try_from_vec(bytes.to_vec())
            .map_err(|e| format!("Failed to parse font: {}", e))?;
        Ok(Self { font })
    }

    /// 尝试加载系统字体（macOS / Linux / Windows）
    pub fn load_system_font() -> Result<Self, String> {
        let candidates = [
            "/System/Library/Fonts/SFCompact.ttf",
            "/System/Library/Fonts/SFNS.ttf",
            "/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
        ];

        let mut last_err = "no system font found".to_string();
        for path in &candidates {
            match std::fs::read(path) {
                Ok(bytes) => return Self::from_bytes(&bytes),
                Err(e) => last_err = e.to_string(),
            }
        }

        Err(format!("Failed to load system font: {}", last_err))
    }

    /// 渲染一行文本到 Pixmap
    pub fn draw_text(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        text: &str,
        x: f64,
        y: f64,
        font_size: f64,
        anchor: TextAnchor,
        baseline: TextBaseline,
        color: &str,
    ) {
        let color = match super::color::parse_color(color) {
            Some(c) => c,
            None => tiny_skia::Color::BLACK,
        };

        let scale = PxScale::from(font_size as f32);
        let scaled_font = self.font.as_scaled(scale);

        // 1. 布局：计算每个字形位置，包含 kerning
        let mut glyphs = Vec::new();
        let mut cursor_x: f32 = 0.0;
        let mut last_id = None;

        for ch in text.chars() {
            let glyph_id = scaled_font.font().glyph_id(ch);

            if let Some(prev) = last_id {
                cursor_x += scaled_font.kern(prev, glyph_id);
            }

            let mut glyph = scaled_font.scaled_glyph(ch);
            glyph.position = point(cursor_x, 0.0);
            glyphs.push(glyph);

            cursor_x += scaled_font.h_advance(glyph_id);
            last_id = Some(glyph_id);
        }

        let text_width = cursor_x;

        // 2. anchor 偏移
        let x_offset: f32 = match anchor {
            TextAnchor::Start => 0.0,
            TextAnchor::Middle => -text_width / 2.0,
            TextAnchor::End => -text_width,
        };

        // 3. baseline 偏移
        let ascent = scaled_font.ascent();
        let descent = scaled_font.descent();
        let y_offset: f32 = match baseline {
            TextBaseline::Top => ascent,
            TextBaseline::Middle => (ascent + descent) / 2.0,
            TextBaseline::Bottom => descent,
            TextBaseline::Alphabetic => 0.0,
        };

        let base_x = x as f32 + x_offset;
        let base_y = y as f32 + y_offset;

        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let stride = pixmap.width();
        let pixels = pixmap.pixels_mut();
        let (fr, fg, fb, fa) = (color.red(), color.green(), color.blue(), color.alpha());

        // 4. 渲染：outline → draw 回调
        //    px_bounds 返回绝对像素矩形，draw(x,y) 相对于 bounds.min
        for mut glyph in glyphs {
            glyph.position.x += base_x;
            glyph.position.y += base_y;

            let outlined = match self.font.outline_glyph(glyph) {
                Some(o) => o,
                None => continue,
            };

            let bounds = outlined.px_bounds();
            let min_x = bounds.min.x;
            let min_y = bounds.min.y;

            outlined.draw(|gx, gy, coverage| {
                let dst_x = (min_x + gx as f32).round() as i32;
                let dst_y = (min_y + gy as f32).round() as i32;

                if dst_x < 0 || dst_x >= pw || dst_y < 0 || dst_y >= ph {
                    return;
                }
                if coverage < 0.01 {
                    return;
                }

                let idx = (dst_y as u32 * stride + dst_x as u32) as usize;
                let bg = pixels[idx];

                let alpha = fa * coverage;
                let inv = 1.0 - alpha;

                let r = ((fr * alpha + bg.red() as f32 / 255.0 * inv) * 255.0) as u8;
                let g = ((fg * alpha + bg.green() as f32 / 255.0 * inv) * 255.0) as u8;
                let b = ((fb * alpha + bg.blue() as f32 / 255.0 * inv) * 255.0) as u8;
                let a = ((alpha * 255.0) + bg.alpha() as f32 * inv).min(255.0) as u8;

                pixels[idx] = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_state_load() {
        let font = FontState::load_system_font();
        assert!(font.is_ok(), "Should load a system font");
    }

    #[test]
    fn test_font_state_from_invalid_bytes() {
        let result = FontState::from_bytes(b"not a font");
        assert!(result.is_err());
    }
}
