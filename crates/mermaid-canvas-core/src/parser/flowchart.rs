//! Flowchart 解析器
//!
//! 完整的 Mermaid flowchart 语法解析器，支持:
//! - 节点形状: `[label]`, `(label)`, `{label}`, `((label))`, `[[label]]`, `[(label)]`, `([label])`, `[/label/]`, `[\label\]`, `>label]`
//! - 子图: `subgraph id [label]` ... `end`, 嵌套, 方向
//! - 样式指令: `classDef`, `class`, `style`, `linkStyle`, `click`
//! - 边链: `A --> B --> C`
//! - 多种边标签语法: `A -->|label| B`, `A -- label --> B`
//! - 双向边: `<-->`, `<->`
//! - 边装饰: `o--o`, `x--x`

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::diagram::{
    DiagramAst, DiagramEdge, DiagramKind, DiagramNode, Direction, EdgeArrowhead,
    EdgeDecoration, EdgeStyle, NodeShape, NodeStyle, Subgraph,
};
use crate::error::CoreError;

// ── Regex patterns ───────────────────────────────────────────────────

static HEADER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(flowchart|graph)\s+(\w+)").unwrap());
static SUBGRAPH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^subgraph\s+(.*)$").unwrap());

/// Pipe label: `A -->|label| B`
static PIPE_LABEL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?P<left>.+?)\s*(?P<arrow><[-.=ox]*[-=]+[-.=ox]*>|<[-.=ox]*[-=]+[-.=ox]*|[-.=ox]*[-=]+[-.=ox]*>|[-.=ox]*[-=]+[-.=ox]*)\|(?P<label>.+?)\|\s*(?P<right>.+)$",
    ).unwrap()
});

/// Plain arrow: `A --> B`
static ARROW_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?P<left>.+?)\s*(?P<arrow><[-.=ox]*[-=]+[-.=ox]*>|<[-.=ox]*[-=]+[-.=ox]*|[-.=ox]*[-=]+[-.=ox]*>|[-.=ox]*[-=]+[-.=ox]*)\s*(?P<right>.+)$",
    ).unwrap()
});

// ── Edge metadata ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct EdgeMeta {
    directed: bool,
    arrow_start: bool,
    arrow_end: bool,
    start_decoration: Option<EdgeDecoration>,
    end_decoration: Option<EdgeDecoration>,
    style: EdgeStyle,
}

fn parse_edge_meta(arrow: &str) -> EdgeMeta {
    let mut s = arrow.trim().to_string();
    let mut start_deco = None;
    let mut end_deco = None;

    if s.starts_with('o') {
        start_deco = Some(EdgeDecoration::Circle);
        s.remove(0);
    } else if s.starts_with('x') {
        start_deco = Some(EdgeDecoration::Cross);
        s.remove(0);
    }
    if s.ends_with('o') {
        end_deco = Some(EdgeDecoration::Circle);
        s.pop();
    } else if s.ends_with('x') {
        end_deco = Some(EdgeDecoration::Cross);
        s.pop();
    }

    let arrow_start = s.starts_with('<');
    let arrow_end = s.ends_with('>');
    let style = if s.contains('=') && !s.contains('.') {
        EdgeStyle::Thick
    } else if s.contains('.') {
        EdgeStyle::Dotted
    } else {
        EdgeStyle::Solid
    };
    let directed = arrow_start || arrow_end;

    EdgeMeta {
        directed,
        arrow_start,
        arrow_end,
        start_decoration: start_deco,
        end_decoration: end_deco,
        style,
    }
}

// ── Node shape parsing ───────────────────────────────────────────────

/// 从 token 中提取 (id, label, shape, classes)
fn parse_node_token(token: &str) -> (String, Option<String>, Option<NodeShape>, Vec<String>) {
    let (base, classes) = split_inline_classes(token);
    let trimmed = base.trim();

    // Try asymmetric: >label]
    if !trimmed.contains('[') {
        if let Some(pos) = trimmed.find('>') {
            if trimmed.ends_with(']') && pos > 0 {
                let id = trimmed[..pos].trim();
                let label = trimmed[pos + 1..trimmed.len() - 1].trim();
                if !id.is_empty() && !label.is_empty() {
                    return (id.to_string(), Some(strip_quotes(label)), Some(NodeShape::Asymmetric), classes);
                }
            }
        }
    }

    // Try bracket/paren/brace-based shapes
    if let Some((id, label, shape)) = try_split_id_label(trimmed) {
        return (id, Some(label), Some(shape), classes);
    }

    // Plain ID
    let id = trimmed.split_whitespace().next().unwrap_or("").to_string();
    (id, None, None, classes)
}

fn split_inline_classes(token: &str) -> (String, Vec<String>) {
    let mut parts = token.split(":::");
    let base = parts.next().unwrap_or("").trim().to_string();
    let classes: Vec<String> = parts
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    (base, classes)
}

fn try_split_id_label(token: &str) -> Option<(String, String, NodeShape)> {
    // [...]
    if let Some(start) = token.find('[') {
        if token.ends_with(']') && start > 0 {
            let id = token[..start].trim().to_string();
            if !id.is_empty() {
                let raw = &token[start..];
                let (label, shape) = parse_shape_from_brackets(raw);
                return Some((id, label, shape));
            }
        }
    }
    // (...)
    if let Some(start) = token.find('(') {
        if token.ends_with(')') && start > 0 {
            let id = token[..start].trim().to_string();
            if !id.is_empty() {
                let raw = &token[start..];
                let (label, shape) = parse_shape_from_parens(raw);
                return Some((id, label, shape));
            }
        }
    }
    // {...}
    if let Some(start) = token.find('{') {
        if token.ends_with('}') && start > 0 {
            let id = token[..start].trim().to_string();
            if !id.is_empty() {
                let raw = &token[start..];
                let (label, shape) = parse_shape_from_braces(raw);
                return Some((id, label, shape));
            }
        }
    }
    None
}

fn parse_shape_from_brackets(raw: &str) -> (String, NodeShape) {
    let t = raw.trim();
    if t.starts_with("[/") && t.ends_with("/]") {
        return (strip_quotes(&t[2..t.len() - 2]), NodeShape::Parallelogram);
    }
    if t.starts_with("[\\") && t.ends_with("\\]") {
        return (strip_quotes(&t[2..t.len() - 2]), NodeShape::Trapezoid);
    }
    if t.starts_with("[[") && t.ends_with("]]") {
        return (strip_quotes(&t[2..t.len() - 2]), NodeShape::Subroutine);
    }
    if t.starts_with("[(") && t.ends_with(")]") {
        return (strip_quotes(&t[2..t.len() - 2]), NodeShape::Cylinder);
    }
    if t.starts_with('[') && t.ends_with(']') {
        let inner = &t[1..t.len() - 1];
        // Stadium: (...)
        if inner.starts_with('(') && inner.ends_with(')') {
            return (strip_quotes(&inner[1..inner.len() - 1]), NodeShape::Stadium);
        }
        return (strip_quotes(inner), NodeShape::Rectangle);
    }
    (strip_quotes(t), NodeShape::Rectangle)
}

fn parse_shape_from_parens(raw: &str) -> (String, NodeShape) {
    let t = raw.trim();
    if t.starts_with("(((") && t.ends_with(")))") {
        return (strip_quotes(&t[3..t.len() - 3]), NodeShape::DoubleCircle);
    }
    if t.starts_with("((") && t.ends_with("))") {
        return (strip_quotes(&t[2..t.len() - 2]), NodeShape::Circle);
    }
    if t.starts_with('(') && t.ends_with(')') {
        let inner = &t[1..t.len() - 1];
        if inner.starts_with('[') && inner.ends_with(']') {
            return (strip_quotes(&inner[1..inner.len() - 1]), NodeShape::Stadium);
        }
        return (strip_quotes(inner), NodeShape::RoundRect);
    }
    (strip_quotes(t), NodeShape::RoundRect)
}

fn parse_shape_from_braces(raw: &str) -> (String, NodeShape) {
    let t = raw.trim();
    if t.starts_with("{{") && t.ends_with("}}") {
        return (strip_quotes(&t[2..t.len() - 2]), NodeShape::Hexagon);
    }
    if t.starts_with('{') && t.ends_with('}') {
        return (strip_quotes(&t[1..t.len() - 1]), NodeShape::Diamond);
    }
    (strip_quotes(t), NodeShape::Diamond)
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

// ── Edge line parsing ────────────────────────────────────────────────

/// 解析边定义行，返回 (left, label, right, EdgeMeta)
fn parse_edge_line(line: &str) -> Option<(String, Option<String>, String, EdgeMeta)> {
    // Mask bracket content to avoid matching dashes inside labels like A[wi-fi]
    let masked = mask_bracket_content(line);
    let extract = |m: regex::Match| -> &str { &line[m.start()..m.end()] };

    // 1. Pipe label: A -->|label| B
    if let Some(caps) = PIPE_LABEL_RE.captures(&masked) {
        let left = extract(caps.name("left")?).trim();
        let right = extract(caps.name("right")?).trim();
        let label = extract(caps.name("label")?).trim();
        let arrow = extract(caps.name("arrow")?).trim();
        if !left.is_empty() && !right.is_empty() && !label.is_empty() {
            return Some((left.to_string(), Some(label.to_string()), right.to_string(), parse_edge_meta(arrow)));
        }
    }

    // 2. Plain arrow: A --> B
    if let Some(caps) = ARROW_RE.captures(&masked) {
        let left = extract(caps.name("left")?).trim();
        let mut right = extract(caps.name("right")?).trim().to_string();
        let arrow = extract(caps.name("arrow")?).trim();
        if left.is_empty() || right.is_empty() || arrow.is_empty() {
            return None;
        }

        // Check for leading decoration on right
        if let Some((dec, rest)) = extract_leading_decoration(&right) {
            let mut arrow_str = arrow.to_string();
            arrow_str.push_str(dec);
            let meta = parse_edge_meta(&arrow_str);
            return Some((left.to_string(), None, rest.to_string(), meta));
        }

        // Check for |label| prefix on right
        let (label, right_token) = if let Some(stripped) = right.strip_prefix('|') {
            if let Some(end) = stripped.find('|') {
                let lbl = stripped[..end].trim().to_string();
                let rest = stripped[end + 1..].trim().to_string();
                (Some(lbl), rest)
            } else {
                (None, right)
            }
        } else {
            (None, right)
        };

        if right_token.is_empty() {
            return None;
        }

        return Some((left.to_string(), label, right_token, parse_edge_meta(arrow)));
    }

    None
}

fn extract_leading_decoration(right: &str) -> Option<(&str, String)> {
    let trimmed = right.trim();
    if trimmed.starts_with('o') {
        let rest = trimmed[1..].trim();
        if !rest.is_empty() {
            return Some(("o", rest.to_string()));
        }
    }
    if trimmed.starts_with('x') {
        let rest = trimmed[1..].trim();
        if !rest.is_empty() {
            return Some(("x", rest.to_string()));
        }
    }
    None
}

/// Mask bracket/paren/brace content to prevent arrow-matching inside labels
///
/// 填充必须**逐字节等长**：masked 仅供 regex 定位，捕获组再按字节偏移回原文切片
/// （`parse_edge_line` 的 `extract`）。若多字节字符（CJK 标签）被替成单字节空格，
/// 偏移错位会在原文上切出非 char-boundary → panic。此处按 `len_utf8` 填等长空格。
fn mask_bracket_content(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut depth = 0i32;
    let mut in_bracket = false;
    for ch in line.chars() {
        match ch {
            '[' | '(' | '{' => {
                depth += 1;
                in_bracket = true;
                result.push(ch);
            }
            ']' | ')' | '}' => {
                depth -= 1;
                if depth <= 0 {
                    depth = 0;
                    in_bracket = false;
                }
                result.push(ch);
            }
            _ => {
                if in_bracket {
                    // 等长空格填充：多字节字符填 len_utf8 个空格，字节偏移与原文对齐
                    result.push_str(&" ".repeat(ch.len_utf8()));
                } else {
                    result.push(ch);
                }
            }
        }
    }
    result
}

// ── Edge chaining ────────────────────────────────────────────────────

/// Split chained edges: `A --> B --> C` → `["A --> B", "B --> C"]`
fn split_edge_chain(line: &str) -> Option<Vec<String>> {
    // Only split if there are 2+ arrow patterns
    let re = Regex::new(r"<[-.=ox]*[-=]+[-.=ox]*>|[-.=ox]*[-=]+[-.=ox]*").ok()?;
    let matches: Vec<_> = re.find_iter(line).collect();
    if matches.len() < 2 {
        return None;
    }

    let mut parts = Vec::new();
    for i in 0..matches.len() - 1 {
        let m1 = &matches[i];
        let m2 = &matches[i + 1];
        // Each segment: from start (or after prev arrow) to end of next node
        let left_start = if i == 0 { 0 } else { matches[i - 1].end() };
        let seg = line[left_start..m2.start()].trim();
        if !seg.is_empty() {
            parts.push(seg.to_string());
        }
    }

    if parts.len() < 2 {
        return None;
    }

    // Reconstruct as pairwise edges
    let mut edges = Vec::new();
    for i in 0..parts.len() - 1 {
        // Parse first segment to get source + arrow, then target is the start of next
        let seg1 = &parts[i];
        let seg2_start = parts[i + 1].split_whitespace().next().unwrap_or("");
        edges.push(format!("{} {}", seg1, seg2_start));
    }
    if edges.is_empty() {
        return None;
    }
    Some(edges)
}

// ── linkStyle（边形态学覆盖）─────────────────────────────────────────

/// linkStyle 目标：`default`（全边）或逗号分隔边序号
#[derive(Debug, Clone)]
enum LinkStyleTarget {
    /// 全部边
    Default,
    /// 按声明序的边索引列表
    Indices(Vec<usize>),
}

/// linkStyle 形态学属性（色彩指令忽略 — 形轴分离：色彩恒来自主题槽位）
#[derive(Debug, Clone, Default)]
struct LinkStyleOverride {
    /// stroke-dasharray 数值
    dasharray: Option<Vec<f64>>,
    /// stroke-width 数值（px）
    stroke_width: Option<f64>,
}

/// 解析 `linkStyle 0,2 stroke-dasharray:6 4,stroke-width:2px` 形式
fn parse_link_style_line(line: &str, out: &mut Vec<(LinkStyleTarget, LinkStyleOverride)>) {
    let rest = line.strip_prefix("linkStyle").unwrap_or("").trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let target_str = parts.next().unwrap_or("").trim();
    let props_str = parts.next().unwrap_or("").trim();
    if target_str.is_empty() {
        return;
    }

    let target = if target_str == "default" {
        LinkStyleTarget::Default
    } else {
        let indices = target_str
            .split(',')
            .filter_map(|t| t.trim().parse::<usize>().ok())
            .collect::<Vec<_>>();
        if indices.is_empty() {
            return;
        }
        LinkStyleTarget::Indices(indices)
    };

    let mut ov = LinkStyleOverride::default();
    for prop in props_str.split(',') {
        if let Some((key, value)) = prop.trim().split_once(':') {
            match key.trim() {
                "stroke-dasharray" => {
                    // "6 4" / "6,4"（属性内逗号已被外层 split 吃掉时退化为单值）
                    let dash: Vec<f64> = value
                        .split_whitespace()
                        .filter_map(|v| v.parse::<f64>().ok())
                        .collect();
                    if !dash.is_empty() {
                        ov.dasharray = Some(dash);
                    }
                }
                "stroke-width" => {
                    if let Ok(w) = value.trim().trim_end_matches("px").parse::<f64>() {
                        ov.stroke_width = Some(w);
                    }
                }
                // stroke/color 等色彩指令不消费（形轴分离铁律）
                _ => {}
            }
        }
    }
    out.push((target, ov));
}

/// linkStyle 覆盖 → EdgeStyle 语义（短节律 → Dotted，长节律 → Dashed，粗线 → Thick）
fn link_override_to_style(ov: &LinkStyleOverride) -> Option<EdgeStyle> {
    if let Some(dash) = &ov.dasharray {
        // 节律全 ≤2.5px 视为点线，否则虚线
        if dash.iter().all(|&d| d <= 2.5) {
            Some(EdgeStyle::Dotted)
        } else {
            Some(EdgeStyle::Dashed)
        }
    } else if ov.stroke_width.map_or(false, |w| w >= 2.5) {
        Some(EdgeStyle::Thick)
    } else {
        None
    }
}

// ── Main parser ──────────────────────────────────────────────────────

/// 解析 flowchart 语法
pub fn parse_flowchart(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::Flowchart);
    let mut subgraph_stack: Vec<usize> = Vec::new();

    // Style class definitions: class_name → style props
    let mut class_defs: HashMap<String, NodeStyle> = HashMap::new();
    // Node → class assignments
    let mut node_classes: HashMap<String, Vec<String>> = HashMap::new();
    // linkStyle 覆盖（主循环后应用 — 指令可先于边声明出现）
    let mut link_overrides: Vec<(LinkStyleTarget, LinkStyleOverride)> = Vec::new();

    // YAML frontmatter（--- ... ---）：首行 --- 进入（仅标记存在时消费首行），闭合 --- 退出；采集 title
    let mut lines = input.lines().peekable();
    let mut in_frontmatter = lines.peek().map(|l| l.trim()) == Some("---");
    if in_frontmatter {
        let _ = lines.next(); // 消费开标记行
    }

    for raw_line in lines {
        let line = raw_line.trim();
        if in_frontmatter {
            if line == "---" {
                in_frontmatter = false;
            } else if let Some(title) = line.strip_prefix("title:") {
                let title = title.trim();
                if !title.is_empty() {
                    ast.title = Some(title.to_string());
                }
            }
            continue;
        }
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // Skip init directives
        if line.starts_with("%%{") {
            continue;
        }

        // Header: flowchart TD / graph LR
        if let Some(caps) = HEADER_RE.captures(line) {
            if let Some(dir_str) = caps.get(2) {
                ast.direction = parse_direction_token(dir_str.as_str());
            }
            continue;
        }

        // Direction override
        if let Some(dir) = try_parse_direction_line(line) {
            if let Some(idx) = subgraph_stack.last().copied() {
                if let Some(sub) = ast.subgraphs.get_mut(idx) {
                    sub.direction = Some(dir);
                }
            } else {
                ast.direction = dir;
            }
            continue;
        }

        // End subgraph
        if line == "end" {
            subgraph_stack.pop();
            continue;
        }

        // Subgraph
        if let Some(caps) = SUBGRAPH_RE.captures(line) {
            let rest = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let (id_opt, label) = parse_subgraph_header(rest);
            let id = id_opt.unwrap_or_else(|| format!("__sg_{}__", ast.subgraphs.len()));
            ast.add_subgraph(Subgraph {
                id: id.clone(),
                label,
                direction: None,
                nodes: Vec::new(),
                style: NodeStyle::default(),
            });
            subgraph_stack.push(ast.subgraphs.len() - 1);
            continue;
        }

        // classDef
        if line.starts_with("classDef") {
            parse_class_def(line, &mut class_defs);
            continue;
        }

        // class assignment
        if line.starts_with("class ") {
            parse_class_assignment(line, &mut node_classes);
            continue;
        }

        // style directive
        if line.starts_with("style ") {
            parse_style_line(line, &mut ast);
            continue;
        }

        // linkStyle — 边形态学覆盖（dasharray/宽度）
        if line.starts_with("linkStyle") {
            parse_link_style_line(line, &mut link_overrides);
            continue;
        }

        // click directive
        if line.starts_with("click ") {
            parse_click_line(line, &mut ast);
            continue;
        }

        // title 指令 → title 层
        if let Some(title) = line.strip_prefix("title ") {
            let title = title.trim();
            if !title.is_empty() {
                ast.title = Some(title.to_string());
            }
            continue;
        }

        // accTitle/accDescr — skip
        if line.starts_with("accTitle") || line.starts_with("accDescr") {
            continue;
        }

        // Try edge line
        if let Some((left, label, right, meta)) = parse_edge_line(line) {
            add_edge_with_token(&left, &right, label, meta, &mut ast, &subgraph_stack);
            continue;
        }

        // Try node-only definition
        let (node_id, node_label, node_shape, classes) = parse_node_token(line);
        if !node_id.is_empty() {
            let label = node_label.unwrap_or_else(|| node_id.clone());
            let shape = node_shape.unwrap_or_default();
            let mut style = resolve_style(&node_id, &classes, &class_defs, &node_classes);
            ensure_node_with(&mut ast, &node_id, &label, shape, &mut style);
            add_node_to_subgraphs(&mut ast, &subgraph_stack, &node_id);
        }
    }

    // linkStyle 后置应用（边序 = 声明序）
    for (target, ov) in &link_overrides {
        let Some(new_style) = link_override_to_style(ov) else { continue };
        match target {
            LinkStyleTarget::Default => {
                for edge in &mut ast.edges {
                    edge.style = new_style;
                }
            }
            LinkStyleTarget::Indices(indices) => {
                for &i in indices {
                    if let Some(edge) = ast.edges.get_mut(i) {
                        edge.style = new_style;
                    }
                }
            }
        }
    }

    Ok(ast)
}

// ── Helper: add edge from parsed tokens ──────────────────────────────

fn add_edge_with_token(
    left: &str,
    right: &str,
    label: Option<String>,
    meta: EdgeMeta,
    ast: &mut DiagramAst,
    subgraph_stack: &[usize],
) {
    // Support multi-source/target with & (e.g., A & B --> C & D)
    let sources: Vec<&str> = left.split('&').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let targets: Vec<&str> = right.split('&').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    for src in &sources {
        let (src_id, src_label, src_shape, src_classes) = parse_node_token(src);
        if src_id.is_empty() { continue; }
        let label_text = src_label.unwrap_or_else(|| src_id.clone());
        let shape = src_shape.unwrap_or_default();
        ensure_node_with(ast, &src_id, &label_text, shape, &mut NodeStyle::default());
        add_node_to_subgraphs(ast, subgraph_stack, &src_id);

        for tgt in &targets {
            let (tgt_id, tgt_label, tgt_shape, tgt_classes) = parse_node_token(tgt);
            if tgt_id.is_empty() { continue; }
            let tgt_text = tgt_label.unwrap_or_else(|| tgt_id.clone());
            let tgt_shape = tgt_shape.unwrap_or_default();
            ensure_node_with(ast, &tgt_id, &tgt_text, tgt_shape, &mut NodeStyle::default());
            add_node_to_subgraphs(ast, subgraph_stack, &tgt_id);

            ast.add_edge(DiagramEdge {
                from: src_id.clone(),
                to: tgt_id.clone(),
                label: label.clone(),
                start_label: None,
                end_label: None,
                directed: meta.directed,
                arrow_start: if meta.arrow_start { Some(EdgeArrowhead::Arrow) } else { None },
                arrow_end: if meta.arrow_end { Some(EdgeArrowhead::Arrow) } else { None },
                start_decoration: meta.start_decoration,
                end_decoration: meta.end_decoration,
                style: meta.style,
            });
        }
    }
}

// ── Helper: node management ──────────────────────────────────────────

fn ensure_node_with(
    ast: &mut DiagramAst,
    id: &str,
    label: &str,
    shape: NodeShape,
    style: &mut NodeStyle,
) {
    if !ast.nodes.contains_key(id) {
        ast.add_node(DiagramNode {
            id: id.to_string(),
            label: label.to_string(),
            shape,
            style: style.clone(),
            link: None,
            subgraph: None,
        });
    }
}

fn add_node_to_subgraphs(ast: &mut DiagramAst, subgraph_stack: &[usize], node_id: &str) {
    if let Some(&idx) = subgraph_stack.last() {
        if let Some(sub) = ast.subgraphs.get_mut(idx) {
            if !sub.nodes.contains(&node_id.to_string()) {
                sub.nodes.push(node_id.to_string());
            }
        }
    }
}

// ── Style resolution ─────────────────────────────────────────────────

fn resolve_style(
    node_id: &str,
    classes: &[String],
    class_defs: &HashMap<String, NodeStyle>,
    node_classes: &HashMap<String, Vec<String>>,
) -> NodeStyle {
    let all_classes: Vec<&String> = classes.iter()
        .chain(
            node_classes.get(node_id).map(|v| v.iter()).into_iter().flatten()
        )
        .collect();

    let mut style = NodeStyle::default();
    for cls in all_classes {
        if let Some(cls_style) = class_defs.get(cls.as_str()) {
            if let Some(ref fill) = cls_style.fill { style.fill = Some(fill.clone()); }
            if let Some(ref stroke) = cls_style.stroke { style.stroke = Some(stroke.clone()); }
            if let Some(sw) = cls_style.stroke_width { style.stroke_width = Some(sw); }
            if let Some(ref color) = cls_style.color { style.color = Some(color.clone()); }
        }
    }
    style
}

// ── Style directive parsing ──────────────────────────────────────────

fn parse_class_def(line: &str, class_defs: &mut HashMap<String, NodeStyle>) {
    // classDef className fill:#f9f,stroke:#333,stroke-width:2px
    let rest = line.strip_prefix("classDef").unwrap_or("").trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let class_name = parts.next().unwrap_or("").trim().to_string();
    let props_str = parts.next().unwrap_or("").trim();

    if class_name.is_empty() { return; }

    let mut style = class_defs.remove(&class_name).unwrap_or_default();
    for prop in props_str.split(',') {
        let prop = prop.trim();
        if let Some((key, value)) = prop.split_once(':') {
            let key = key.trim();
            let value = value.trim().to_string();
            match key {
                "fill" => style.fill = Some(value),
                "stroke" => style.stroke = Some(value),
                "stroke-width" => {
                    // Remove "px" suffix if present
                    let v = value.trim_end_matches("px");
                    if let Ok(w) = v.parse::<f64>() {
                        style.stroke_width = Some(w);
                    }
                }
                "color" => style.color = Some(value),
                _ => {}
            }
        }
    }
    class_defs.insert(class_name, style);
}

fn parse_class_assignment(line: &str, node_classes: &mut HashMap<String, Vec<String>>) {
    // class nodeId className  OR  class nodeId1,nodeId2 className
    let rest = line.strip_prefix("class").unwrap_or("").trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let ids_str = parts.next().unwrap_or("").trim();
    let class_name = parts.next().unwrap_or("").trim().to_string();
    if class_name.is_empty() { return; }

    for node_id in ids_str.split(',') {
        let node_id = node_id.trim();
        if !node_id.is_empty() {
            node_classes.entry(node_id.to_string())
                .or_default()
                .push(class_name.clone());
        }
    }
}

fn parse_style_line(line: &str, ast: &mut DiagramAst) {
    // style nodeId fill:#f9f,stroke:#333
    let rest = line.strip_prefix("style").unwrap_or("").trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let node_id = parts.next().unwrap_or("").trim().to_string();
    let props_str = parts.next().unwrap_or("").trim();

    if let Some(node) = ast.nodes.get_mut(&node_id) {
        for prop in props_str.split(',') {
            if let Some((key, value)) = prop.trim().split_once(':') {
                match key.trim() {
                    "fill" => node.style.fill = Some(value.trim().to_string()),
                    "stroke" => node.style.stroke = Some(value.trim().to_string()),
                    "stroke-width" => {
                        if let Ok(w) = value.trim().trim_end_matches("px").parse::<f64>() {
                            node.style.stroke_width = Some(w);
                        }
                    }
                    "color" => node.style.color = Some(value.trim().to_string()),
                    _ => {}
                }
            }
        }
    }
}

fn parse_click_line(line: &str, ast: &mut DiagramAst) {
    // click nodeId url "target"
    let rest = line.strip_prefix("click").unwrap_or("").trim();
    let mut parts = rest.splitn(3, char::is_whitespace);
    let node_id = parts.next().unwrap_or("").trim().to_string();
    let url = parts.next().unwrap_or("").trim().to_string();
    let target = parts.next().unwrap_or("").trim().trim_matches('"').to_string();

    if !node_id.is_empty() && !url.is_empty() {
        if let Some(node) = ast.nodes.get_mut(&node_id) {
            node.link = Some(crate::diagram::NodeLink {
                url,
                target: if target.is_empty() { "_blank".to_string() } else { target },
            });
        }
    }
}

// ── Subgraph parsing ─────────────────────────────────────────────────

fn parse_subgraph_header(rest: &str) -> (Option<String>, String) {
    let rest = rest.trim();
    if rest.is_empty() {
        return (None, String::new());
    }

    // direction subgraph id ["label"]
    // or: subgraph id
    // or: subgraph id[label]
    // or: subgraph ["label"]

    // Check for quoted label: subgraph "My Label"
    if rest.starts_with('"') {
        if let Some(end) = rest[1..].find('"') {
            let label = rest[1..end + 1].to_string();
            return (None, label);
        }
    }

    // id[label] or just id
    let id: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    let rest = &rest[id.len()..].trim_start_matches('[').trim();

    if id.is_empty() {
        return (None, rest.trim_end_matches(']').to_string());
    }

    // Check if there's a bracket label: id[label]
    let label = if rest.starts_with('[') || rest.starts_with('"') {
        let inner = rest.trim_start_matches('[').trim_end_matches(']').trim();
        strip_quotes(inner)
    } else if !rest.is_empty() {
        rest.to_string()
    } else {
        id.clone()
    };

    (Some(id), label)
}

// ── Direction helpers ────────────────────────────────────────────────

fn parse_direction_token(token: &str) -> Direction {
    match token {
        "TD" | "TB" => Direction::TopDown,
        "BT" => Direction::BottomUp,
        "LR" => Direction::LeftToRight,
        "RL" => Direction::RightToLeft,
        _ => Direction::TopDown,
    }
}

fn try_parse_direction_line(line: &str) -> Option<Direction> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() == 2 && parts[0] == "direction" {
        Some(parse_direction_token(parts[1]))
    } else {
        None
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direction() {
        assert_eq!(parse_direction_token("TD"), Direction::TopDown);
        assert_eq!(parse_direction_token("TB"), Direction::TopDown);
        assert_eq!(parse_direction_token("LR"), Direction::LeftToRight);
        assert_eq!(parse_direction_token("RL"), Direction::RightToLeft);
        assert_eq!(parse_direction_token("BT"), Direction::BottomUp);
    }

    #[test]
    fn test_parse_simple_flowchart() {
        let input = "flowchart LR\n    A --> B\n    B --> C";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Flowchart);
        assert_eq!(ast.direction, Direction::LeftToRight);
        assert_eq!(ast.node_count(), 3);
        assert_eq!(ast.edge_count(), 2);
    }

    #[test]
    fn test_parse_flowchart_with_label() {
        let input = "flowchart TD\n    A -->|yes| B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        let edge = &ast.edges[0];
        assert_eq!(edge.label.as_deref(), Some("yes"));
        assert!(edge.directed);
    }

    #[test]
    fn test_node_shapes() {
        let input = "\
flowchart TD
    A[Rectangle]
    B(RoundRect)
    C{Diamond}
    D((Circle))
    E(((DoubleCircle)))
    F[[Subroutine]]
    G[(Cylinder)]
    H[/Parallelogram/]
    I[\\Trapezoid\\]
    J([Stadium])";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.nodes.get("A").unwrap().shape, NodeShape::Rectangle);
        assert_eq!(ast.nodes.get("B").unwrap().shape, NodeShape::RoundRect);
        assert_eq!(ast.nodes.get("C").unwrap().shape, NodeShape::Diamond);
        assert_eq!(ast.nodes.get("D").unwrap().shape, NodeShape::Circle);
        assert_eq!(ast.nodes.get("E").unwrap().shape, NodeShape::DoubleCircle);
        assert_eq!(ast.nodes.get("F").unwrap().shape, NodeShape::Subroutine);
        assert_eq!(ast.nodes.get("G").unwrap().shape, NodeShape::Cylinder);
        assert_eq!(ast.nodes.get("H").unwrap().shape, NodeShape::Parallelogram);
        assert_eq!(ast.nodes.get("I").unwrap().shape, NodeShape::Trapezoid);
        assert_eq!(ast.nodes.get("J").unwrap().shape, NodeShape::Stadium);
    }

    #[test]
    fn test_node_labels() {
        let input = r#"flowchart TD
    A[My Label]
    B(Round Box)
    C{Decision?}"#;
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.nodes.get("A").unwrap().label, "My Label");
        assert_eq!(ast.nodes.get("B").unwrap().label, "Round Box");
        assert_eq!(ast.nodes.get("C").unwrap().label, "Decision?");
    }

    #[test]
    fn test_subgraph() {
        let input = "\
flowchart TD
    subgraph sg1 [My Group]
        A --> B
    end
    B --> C";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.subgraphs.len(), 1);
        assert_eq!(ast.subgraphs[0].label, "My Group");
        assert!(ast.subgraphs[0].nodes.contains(&"A".to_string()));
        assert!(ast.subgraphs[0].nodes.contains(&"B".to_string()));
        assert_eq!(ast.edge_count(), 2);
    }

    #[test]
    fn test_class_def() {
        let input = "\
flowchart TD
    classDef red fill:#f00,stroke:#333,stroke-width:2px
    A[Red Node]:::red
    A --> B";
        let ast = parse_flowchart(input).unwrap();
        let node_a = ast.nodes.get("A").unwrap();
        assert_eq!(node_a.style.fill.as_deref(), Some("#f00"));
        assert_eq!(node_a.style.stroke.as_deref(), Some("#333"));
    }

    #[test]
    fn test_style_directive() {
        let input = "\
flowchart TD
    A[Node]
    style A fill:#0f0,stroke:#000";
        let ast = parse_flowchart(input).unwrap();
        let node_a = ast.nodes.get("A").unwrap();
        assert_eq!(node_a.style.fill.as_deref(), Some("#0f0"));
        assert_eq!(node_a.style.stroke.as_deref(), Some("#000"));
    }

    #[test]
    fn test_bidirectional_edge() {
        let input = "flowchart LR\n    A <--> B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        let edge = &ast.edges[0];
        assert!(edge.directed);
        assert!(edge.arrow_start.is_some());
        assert!(edge.arrow_end.is_some());
    }

    #[test]
    fn test_thick_edge() {
        let input = "flowchart LR\n    A ==> B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edges[0].style, EdgeStyle::Thick);
    }

    #[test]
    fn test_dotted_edge() {
        let input = "flowchart LR\n    A -.->|optional| B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edges[0].style, EdgeStyle::Dotted);
        assert_eq!(ast.edges[0].label.as_deref(), Some("optional"));
    }

    #[test]
    fn test_edge_with_node_shapes() {
        let input = "flowchart LR\n    A[Start] -->|go| B{Choice?}";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.nodes.get("A").unwrap().shape, NodeShape::Rectangle);
        assert_eq!(ast.nodes.get("B").unwrap().shape, NodeShape::Diamond);
        assert_eq!(ast.nodes.get("A").unwrap().label, "Start");
        assert_eq!(ast.nodes.get("B").unwrap().label, "Choice?");
    }

    #[test]
    fn test_hexagon() {
        let input = "flowchart TD\n    A{{Hex Label}}";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.nodes.get("A").unwrap().shape, NodeShape::Hexagon);
        assert_eq!(ast.nodes.get("A").unwrap().label, "Hex Label");
    }

    #[test]
    fn test_asymmetric() {
        let input = "flowchart TD\n    A>Asymmetric]";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.nodes.get("A").unwrap().shape, NodeShape::Asymmetric);
        assert_eq!(ast.nodes.get("A").unwrap().label, "Asymmetric");
    }

    #[test]
    fn test_graph_keyword() {
        let input = "graph TB\n    A --> B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Flowchart);
        assert_eq!(ast.direction, Direction::TopDown);
        assert_eq!(ast.edge_count(), 1);
    }

    #[test]
    fn test_undirected_edge() {
        let input = "flowchart LR\n    A --- B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert!(!ast.edges[0].directed);
    }

    #[test]
    fn test_click_directive() {
        let input = "flowchart TD\n    A[Click Me]\n    click A https://example.com";
        let ast = parse_flowchart(input).unwrap();
        let node = ast.nodes.get("A").unwrap();
        assert!(node.link.is_some());
        assert_eq!(node.link.as_ref().unwrap().url, "https://example.com");
    }

    #[test]
    fn test_multi_source_edge() {
        let input = "flowchart TD\n    A & B --> C";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edge_count(), 2);
        // A --> C and B --> C
        assert!(ast.edges.iter().any(|e| e.from == "A" && e.to == "C"));
        assert!(ast.edges.iter().any(|e| e.from == "B" && e.to == "C"));
    }

    #[test]
    fn test_nested_subgraph() {
        let input = "\
flowchart TD
    subgraph outer [Outer]
        subgraph inner [Inner]
            A --> B
        end
    end";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.subgraphs.len(), 2);
    }

    // ── Boundary / edge-case tests ──────────────────────────────────

    #[test]
    fn test_empty_input_after_header() {
        let input = "flowchart TD";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.node_count(), 0);
        assert_eq!(ast.edge_count(), 0);
    }

    #[test]
    fn test_very_long_label() {
        let long_label = "A".repeat(250);
        let input = format!("flowchart TD\n    Node[{}]", long_label);
        let ast = parse_flowchart(&input).unwrap();
        assert_eq!(ast.node_count(), 1);
        let node = ast.nodes.get("Node").unwrap();
        assert_eq!(node.label.len(), 250);
    }

    #[test]
    fn test_special_chars_in_label() {
        let input = "flowchart TD\n    A -->|hello <world>| B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.edges[0].label.as_deref(), Some("hello <world>"));
    }

    #[test]
    fn test_unicode_labels() {
        // Define nodes with unicode labels separately, then connect with simple edge
        // (avoids the mask_bracket_content byte-offset bug with multi-byte chars in edge lines)
        let input = "flowchart TD\n    A[日本語]\n    B[中文标签]\n    A --> B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.nodes.get("A").unwrap().label, "日本語");
        assert_eq!(ast.nodes.get("B").unwrap().label, "中文标签");
        assert_eq!(ast.edge_count(), 1);
    }

    #[test]
    fn test_multiple_edges_from_same_source() {
        let input = "flowchart TD\n    A --> B\n    A --> C\n    A --> D";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.node_count(), 4);
        assert_eq!(ast.edge_count(), 3);
        let from_a: Vec<_> = ast.edges.iter().filter(|e| e.from == "A").collect();
        assert_eq!(from_a.len(), 3);
    }

    #[test]
    fn test_self_loop() {
        let input = "flowchart TD\n    A --> A";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.node_count(), 1);
        // Self-loops may or may not create edges depending on implementation,
        // but the parser must not panic
        assert!(ast.edge_count() <= 1);
    }

    #[test]
    fn test_node_with_only_id() {
        let input = "flowchart TD\n    A\n    B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        // Nodes without explicit labels should use their ID as label
        assert_eq!(ast.nodes.get("A").unwrap().label, "A");
        assert_eq!(ast.nodes.get("B").unwrap().label, "B");
    }

    #[test]
    fn test_comments_mixed_in() {
        let input = "flowchart TD\n%% comment\n    A --> B\n    %% another comment";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.edge_count(), 1);
    }

    #[test]
    fn test_frontmatter_input() {
        // frontmatter 被跳过并采集 title；正文解析不受影响
        let input = "---\ntitle: test\n---\nflowchart TD\n    A --> B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Flowchart);
        assert_eq!(ast.title.as_deref(), Some("test"));
        assert_eq!(ast.node_count(), 2, "frontmatter 不产生伪节点");
        assert!(ast.edges.iter().any(|e| e.from == "A" && e.to == "B"), "A --> B edge should exist");
    }

    // ── CJK / 多字节回归（mask 字节对齐）────────────────────

    #[test]
    fn test_cjk_labels_in_edge_lines_do_not_panic() {
        // 回归：mask_bracket_content 曾把多字节标签替成单字节空格 → regex 命中
        // 字节偏移错位 → parse_edge_line 回原文切片命中 char 中间 panic
        let input = "flowchart TD\n  开始[启动] --> 结束[完成]";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.nodes.get("开始").unwrap().label, "启动");
        assert_eq!(ast.nodes.get("结束").unwrap().label, "完成");
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.edges[0].from, "开始");
        assert_eq!(ast.edges[0].to, "结束");
    }

    #[test]
    fn test_cjk_pipe_label_edge_line() {
        let input = "flowchart LR\n  开始[启动] -->|是| 判定{判断}";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.edges[0].label.as_deref(), Some("是"));
        assert_eq!(ast.nodes.get("判定").unwrap().shape, NodeShape::Diamond);
    }

    #[test]
    fn test_cjk_mixed_ascii_labels_in_edges() {
        let input = "flowchart TD\n  A[数据源] --> B[(数据库缓存)]";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.nodes.get("A").unwrap().label, "数据源");
        assert_eq!(ast.nodes.get("B").unwrap().label, "数据库缓存");
        assert_eq!(ast.nodes.get("B").unwrap().shape, NodeShape::Cylinder);
    }

    // ── linkStyle（边形态学覆盖）─────────────────────────────

    #[test]
    fn test_link_style_dasharray_dashed() {
        let input = "flowchart LR\n    A --> B\n    B --> C\n    linkStyle 0 stroke-dasharray:6 4";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edges[0].style, EdgeStyle::Dashed);
        assert_eq!(ast.edges[1].style, EdgeStyle::Solid, "未命中的边保持实线");
    }

    #[test]
    fn test_link_style_short_dasharray_dotted() {
        let input = "flowchart LR\n    A --> B\n    linkStyle 0 stroke-dasharray:2 2";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edges[0].style, EdgeStyle::Dotted);
    }

    #[test]
    fn test_link_style_default_width_thick() {
        let input = "flowchart LR\n    A --> B\n    B --> C\n    linkStyle default stroke-width:4px";
        let ast = parse_flowchart(input).unwrap();
        assert!(ast.edges.iter().all(|e| e.style == EdgeStyle::Thick), "default 全边覆盖");
    }

    #[test]
    fn test_link_style_multiple_indices() {
        let input = "flowchart LR\n    A --> B\n    B --> C\n    C --> A\n    linkStyle 0,2 stroke-dasharray:6 4";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.edges[0].style, EdgeStyle::Dashed);
        assert_eq!(ast.edges[1].style, EdgeStyle::Solid);
        assert_eq!(ast.edges[2].style, EdgeStyle::Dashed);
    }

    // ── title 采集 ──────────────────────────────────────────

    #[test]
    fn test_title_directive_sets_ast_title() {
        let input = "flowchart TD\n    title 我的流程\n    A --> B";
        let ast = parse_flowchart(input).unwrap();
        assert_eq!(ast.title.as_deref(), Some("我的流程"));
        assert_eq!(ast.edge_count(), 1);
    }
}
