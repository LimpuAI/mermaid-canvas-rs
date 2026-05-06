//! CSS 颜色字符串解析为 tiny-skia Color
//!
//! 支持格式：#fff, #ffffff, #ffffffff, rgb(r,g,b), rgba(r,g,b,a)

/// 将 CSS 颜色字符串解析为 tiny_skia::Color
///
/// tiny-skia Color 内部格式: 0xAABBGGRR (u32)
pub fn parse_color(s: &str) -> Option<tiny_skia::Color> {
    let s = s.trim();

    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }

    if let Some(rest) = s.strip_prefix("rgba(") {
        let rest = rest.strip_suffix(')')?;
        return parse_rgba(rest);
    }

    if let Some(rest) = s.strip_prefix("rgb(") {
        let rest = rest.strip_suffix(')')?;
        return parse_rgb(rest);
    }

    None
}

fn parse_hex(hex: &str) -> Option<tiny_skia::Color> {
    match hex.len() {
        3 => {
            let r = hex_char(hex.as_bytes().get(0)?)?;
            let g = hex_char(hex.as_bytes().get(1)?)?;
            let b = hex_char(hex.as_bytes().get(2)?)?;
            let r = (r << 4) | r;
            let g = (g << 4) | g;
            let b = (b << 4) | b;
            Some(tiny_skia::Color::from_rgba8(r, g, b, 255))
        }
        6 => {
            let r = hex_byte(&hex[0..2])?;
            let g = hex_byte(&hex[2..4])?;
            let b = hex_byte(&hex[4..6])?;
            Some(tiny_skia::Color::from_rgba8(r, g, b, 255))
        }
        8 => {
            let r = hex_byte(&hex[0..2])?;
            let g = hex_byte(&hex[2..4])?;
            let b = hex_byte(&hex[4..6])?;
            let a = hex_byte(&hex[6..8])?;
            Some(tiny_skia::Color::from_rgba8(r, g, b, a))
        }
        _ => None,
    }
}

fn hex_char(b: &u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn hex_byte(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    let hi = hex_char(bytes.get(0)?)?;
    let lo = hex_char(bytes.get(1)?)?;
    Some((hi << 4) | lo)
}

fn parse_rgba(s: &str) -> Option<tiny_skia::Color> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    let a: f32 = parts[3].trim().parse().ok()?;
    let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
    Some(tiny_skia::Color::from_rgba8(r, g, b, a))
}

fn parse_rgb(s: &str) -> Option<tiny_skia::Color> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    Some(tiny_skia::Color::from_rgba8(r, g, b, 255))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_3digit() {
        let c = parse_color("#fff").unwrap();
        assert!((c.red() - 1.0).abs() < 0.01);
        assert!((c.green() - 1.0).abs() < 0.01);
        assert!((c.blue() - 1.0).abs() < 0.01);
        assert!((c.alpha() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hex_6digit() {
        let c = parse_color("#ff0000").unwrap();
        assert!((c.red() - 1.0).abs() < 0.01);
        assert!((c.green() - 0.0).abs() < 0.01);
        assert!((c.blue() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_hex_8digit() {
        let c = parse_color("#ff000080").unwrap();
        assert!((c.alpha() - 128.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn test_rgb() {
        let c = parse_color("rgb(255, 128, 0)").unwrap();
        assert!((c.red() - 1.0).abs() < 0.01);
        assert!((c.green() - 128.0 / 255.0).abs() < 0.01);
        assert!((c.blue() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_rgba() {
        let c = parse_color("rgba(78, 121, 167, 0.30)").unwrap();
        assert!((c.red() - 78.0 / 255.0).abs() < 0.01);
        assert!((c.green() - 121.0 / 255.0).abs() < 0.01);
        assert!((c.blue() - 167.0 / 255.0).abs() < 0.01);
        assert!((c.alpha() - 76.0 / 255.0).abs() < 0.05);
    }

    #[test]
    fn test_invalid() {
        assert!(parse_color("not a color").is_none());
        assert!(parse_color("#gg").is_none());
    }
}
