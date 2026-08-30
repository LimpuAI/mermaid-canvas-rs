//! Sequence 解析器
//!
//! 完整的 Mermaid sequence diagram 语法解析器，支持:
//! - 参与者声明: `participant Name`, `participant Name as Alias`, `actor Name`
//! - 11 种箭头类型: `->`, `-->`, `->>`, `-->>`, `-x`, `--x`, `-)`, `--)`, `<-`, `<--`, `<<->>`
//! - 激活/失活: `activate Name`, `deactivate Name`, `+Name`/`-Name` 记号
//! - 笔记: `Note left of/right of/over Name: text`
//! - 控制块: `loop`, `alt/else`, `opt`, `par/and`, `critical/option`, `break`
//! - 背景矩形: `rect rgb(r,g,b)` / `rect rgba(r,g,b,a)`
//! - 自动编号: `autonumber`
//! - 注释: `%%`
//! - 创建/销毁: `create participant Name`, `destroy Name`

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::diagram::{
    ControlBlockKind, DiagramAst, DiagramEdge, DiagramKind, DiagramNode, EdgeArrowhead,
    EdgeStyle, NodeShape, NodeStyle, NotePosition, SequenceActivation, SequenceControlBlock,
    SequenceMeta, SequenceNote, SequenceRect,
};
use crate::error::CoreError;

// ── Regex patterns ───────────────────────────────────────────────────

static MESSAGE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?P<from>[^\s:+-]+?)\s*(?P<activate_from>[+-]?)(?P<arrow>(?:<<-->>|<<->>|<--|<-|-->>|->>|-->)|(?:-[x)]|-->|->|--x|-x))(?P<activate_to>[+-]?)(?:\s+(?P<to>[^\s:]+?))?\s*(?::\s*(?P<label>.+))?$"
    ).unwrap()
});

static PARTICIPANT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:participant|actor)\s+(.+)$").unwrap()
});

static PARTICIPANT_ALIAS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<name>\S+)\s+as\s+(?P<alias>.+)$").unwrap()
});

static NOTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^Note\s+(?P<pos>left of|right of|over)\s+(?P<target>[^:]+?)(?:\s*:\s*(?P<text>.+))?$").unwrap()
});

static RECT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^rect\s+(rgba?\s*\([^)]+\))\s*$").unwrap()
});

static CONTROL_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<kind>loop|alt|opt|par|critical|break)\s+(?P<label>.*)$").unwrap()
});

static CONTROL_ELSE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<kind>else|and|option)\s*(?P<label>.*)$").unwrap()
});

static CREATE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^create\s+(?:participant|actor)\s+(.+)$").unwrap()
});

// ── Arrow parsing ────────────────────────────────────────────────────

/// 箭头信息
#[derive(Debug, Clone)]
struct ArrowInfo {
    /// 是否有向
    directed: bool,
    /// 结束箭头类型
    arrow_end: Option<EdgeArrowhead>,
    /// 起始箭头类型 (反向箭头时)
    arrow_start: Option<EdgeArrowhead>,
    /// 边样式
    style: EdgeStyle,
    /// 是否反向 (target → source)
    reversed: bool,
}

fn parse_arrow(arrow: &str) -> Option<ArrowInfo> {
    match arrow.trim() {
        // Solid arrows
        "->>" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::Arrow),
            arrow_start: None, style: EdgeStyle::Solid, reversed: false,
        }),
        "-->>" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::Arrow),
            arrow_start: None, style: EdgeStyle::Dashed, reversed: false,
        }),
        "->" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::OpenTriangle),
            arrow_start: None, style: EdgeStyle::Solid, reversed: false,
        }),
        "-->" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::OpenTriangle),
            arrow_start: None, style: EdgeStyle::Dashed, reversed: false,
        }),
        "-x" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::Cross),
            arrow_start: None, style: EdgeStyle::Solid, reversed: false,
        }),
        "--x" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::Cross),
            arrow_start: None, style: EdgeStyle::Dashed, reversed: false,
        }),
        "-)" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::OpenTriangle),
            arrow_start: None, style: EdgeStyle::Solid, reversed: false,
        }),
        "--)" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::OpenTriangle),
            arrow_start: None, style: EdgeStyle::Dashed, reversed: false,
        }),
        // Reverse arrows
        "<-" => Some(ArrowInfo {
            directed: true, arrow_end: None,
            arrow_start: Some(EdgeArrowhead::OpenTriangle), style: EdgeStyle::Solid, reversed: true,
        }),
        "<--" => Some(ArrowInfo {
            directed: true, arrow_end: None,
            arrow_start: Some(EdgeArrowhead::OpenTriangle), style: EdgeStyle::Dashed, reversed: true,
        }),
        // Bidirectional
        "<<->>" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::Arrow),
            arrow_start: Some(EdgeArrowhead::Arrow), style: EdgeStyle::Solid, reversed: false,
        }),
        "<<-->>" => Some(ArrowInfo {
            directed: true, arrow_end: Some(EdgeArrowhead::Arrow),
            arrow_start: Some(EdgeArrowhead::Arrow), style: EdgeStyle::Dashed, reversed: false,
        }),
        _ => None,
    }
}

// ── Activation stack ─────────────────────────────────────────────────

/// 每个参与者的激活栈
struct ActivationTracker {
    /// participant_id → 栈中激活记录的索引列表
    stacks: HashMap<String, Vec<usize>>,
}

impl ActivationTracker {
    fn new() -> Self {
        Self { stacks: HashMap::new() }
    }

    /// 激活一个参与者，返回当前深度
    fn activate(&mut self, participant_id: &str, step: usize, activations: &mut Vec<SequenceActivation>) -> usize {
        let stack = self.stacks.entry(participant_id.to_string()).or_default();
        let depth = stack.len();
        let idx = activations.len();
        activations.push(SequenceActivation {
            participant_id: participant_id.to_string(),
            start_step: step,
            end_step: None,
            depth,
        });
        stack.push(idx);
        depth
    }

    /// 失活一个参与者
    fn deactivate(&mut self, participant_id: &str, step: usize, activations: &mut Vec<SequenceActivation>) {
        if let Some(stack) = self.stacks.get_mut(participant_id) {
            if let Some(idx) = stack.pop() {
                if let Some(act) = activations.get_mut(idx) {
                    act.end_step = Some(step);
                }
            }
        }
    }

    /// 关闭所有未关闭的激活
    fn close_all(&mut self, step: usize, activations: &mut Vec<SequenceActivation>) {
        for (_, stack) in &self.stacks {
            for &idx in stack {
                if let Some(act) = activations.get_mut(idx) {
                    act.end_step = Some(step);
                }
            }
        }
    }
}

// ── Main parser ──────────────────────────────────────────────────────

/// 解析序列图语法
pub fn parse_sequence(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::Sequence);
    let mut meta = SequenceMeta::default();

    let mut activation_tracker = ActivationTracker::new();
    let mut step_counter: usize = 0;

    // Control block stack: (kind, label, start_step, groups)
    let mut control_stack: Vec<(ControlBlockKind, String, usize, Vec<(String, usize)>)> = Vec::new();
    // Rect stack: (color, start_step)
    let mut rect_stack: Vec<(String, usize)> = Vec::new();

    let mut saw_header = false;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Comments
        if line.starts_with("%%") {
            continue;
        }

        // Header
        if line.starts_with("sequenceDiagram") {
            saw_header = true;
            continue;
        }

        if !saw_header {
            continue;
        }

        // Autonumber
        if line == "autonumber" {
            meta.autonumber = true;
            continue;
        }

        // Title → title 层（mermaid sequence 的 title: 指令）
        if let Some(title) = line.strip_prefix("title:") {
            let title = title.trim();
            if !title.is_empty() {
                ast.title = Some(title.to_string());
            }
            continue;
        }

        // End block (for control blocks or rects)
        if line == "end" {
            // Check rect stack first
            if let Some((color, start_step)) = rect_stack.pop() {
                meta.rects.push(SequenceRect {
                    color,
                    start_step,
                    end_step: step_counter,
                });
                continue;
            }
            // Then control block stack
            if let Some((kind, label, start_step, groups)) = control_stack.pop() {
                meta.control_blocks.push(SequenceControlBlock {
                    kind,
                    label,
                    start_step,
                    end_step: step_counter,
                    groups,
                });
                continue;
            }
            continue;
        }

        // Rect
        if let Some(caps) = RECT_RE.captures(line) {
            let color = caps.get(1).unwrap().as_str().to_string();
            rect_stack.push((color, step_counter));
            continue;
        }

        // Control block start
        if let Some(caps) = CONTROL_BLOCK_RE.captures(line) {
            let kind_str = caps.name("kind").unwrap().as_str();
            let label = caps.name("label").map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            let kind = match kind_str {
                "loop" => ControlBlockKind::Loop,
                "alt" => ControlBlockKind::Alt,
                "opt" => ControlBlockKind::Opt,
                "par" => ControlBlockKind::Par,
                "critical" => ControlBlockKind::Critical,
                "break" => ControlBlockKind::Break,
                _ => continue,
            };
            control_stack.push((kind, label, step_counter, Vec::new()));
            continue;
        }

        // Control block else/and/option
        if let Some(caps) = CONTROL_ELSE_RE.captures(line) {
            let kind_str = caps.name("kind").unwrap().as_str();
            let label = caps.name("label").map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            if let Some((_kind, _label, _start, ref mut groups)) = control_stack.last_mut() {
                groups.push((label, step_counter));
            }
            continue;
        }

        // Create participant/actor
        if let Some(caps) = CREATE_RE.captures(line) {
            let name = caps.get(1).unwrap().as_str().trim().to_string();
            ensure_participant(&mut ast, &mut meta, &name, false);
            continue;
        }

        // Destroy
        if let Some(name) = line.strip_prefix("destroy ") {
            let name = name.trim().to_string();
            let resolved = resolve_participant(&meta, &name);
            // No step increment for destroy - it's just a marker
            let _ = resolved;
            continue;
        }

        // Activate
        if let Some(name) = line.strip_prefix("activate ") {
            let name = name.trim().to_string();
            let resolved = resolve_participant(&meta, &name);
            activation_tracker.activate(&resolved, step_counter, &mut meta.activations);
            continue;
        }

        // Deactivate
        if let Some(name) = line.strip_prefix("deactivate ") {
            let name = name.trim().to_string();
            let resolved = resolve_participant(&meta, &name);
            activation_tracker.deactivate(&resolved, step_counter, &mut meta.activations);
            continue;
        }

        // Note
        if let Some(caps) = NOTE_RE.captures(line) {
            let pos_str = caps.name("pos").unwrap().as_str();
            let target_str = caps.name("target").unwrap().as_str().trim();
            let text = caps.name("text").map(|m| m.as_str().trim().to_string()).unwrap_or_default();

            let position = match pos_str {
                "left of" => {
                    let resolved = resolve_participant(&meta, target_str);
                    NotePosition::LeftOf(resolved)
                }
                "right of" => {
                    let resolved = resolve_participant(&meta, target_str);
                    NotePosition::RightOf(resolved)
                }
                "over" => {
                    let names: Vec<String> = target_str
                        .split(',')
                        .map(|s| resolve_participant(&meta, s.trim()))
                        .collect();
                    NotePosition::Over(names)
                }
                _ => continue,
            };

            meta.notes.push(SequenceNote {
                text,
                position,
                step: step_counter,
            });
            continue;
        }

        // Participant/actor declaration
        if line.starts_with("participant ") || line.starts_with("actor ") {
            let rest = PARTICENT_RE.captures(line)
                .or_else(|| {
                    let prefix = if line.starts_with("actor ") { "actor " } else { "participant " };
                    Regex::new(&format!(r"^{}\s+(.+)$", regex::escape(prefix)))
                        .ok()
                        .and_then(|re| re.captures(line))
                });
            let is_actor = line.starts_with("actor ");
            let rest_str = line.trim_start_matches("participant ").trim_start_matches("actor ").trim();
            if let Some(caps) = PARTICIPANT_ALIAS_RE.captures(rest_str) {
                let name = caps.name("name").unwrap().as_str().trim().to_string();
                let alias = caps.name("alias").unwrap().as_str().trim().to_string();
                meta.aliases.insert(alias.clone(), name.clone());
                ensure_participant(&mut ast, &mut meta, &name, is_actor);
                // Track alias in participant_order too (for lookup by alias)
            } else {
                ensure_participant(&mut ast, &mut meta, rest_str, is_actor);
            }
            continue;
        }

        // Message line: try to parse as From Arrow To : Label
        if let Some(parsed) = parse_message_line(line) {
            let from_resolved = resolve_participant(&meta, &parsed.from);
            let to_resolved = resolve_participant(&meta, &parsed.to);

            // Handle +/- activation notation
            if parsed.activate_from {
                activation_tracker.activate(&from_resolved, step_counter, &mut meta.activations);
            }
            if parsed.activate_to {
                activation_tracker.activate(&to_resolved, step_counter, &mut meta.activations);
            }

            // Ensure both participants exist
            ensure_participant(&mut ast, &mut meta, &from_resolved, false);
            ensure_participant(&mut ast, &mut meta, &to_resolved, false);

            // Determine arrow direction
            let (actual_from, actual_to, arrow_start, arrow_end) = if parsed.arrow.reversed {
                (&to_resolved, &from_resolved, parsed.arrow.arrow_start, parsed.arrow.arrow_end)
            } else {
                (&from_resolved, &to_resolved, parsed.arrow.arrow_start, parsed.arrow.arrow_end)
            };

            // Build label with optional autonumber prefix
            let label = if meta.autonumber {
                meta.message_counter += 1;
                let auto_label = format!("{}. {}", meta.message_counter, parsed.label.as_deref().unwrap_or(""));
                Some(auto_label)
            } else {
                parsed.label
            };

            ast.add_edge(DiagramEdge {
                from: actual_from.clone(),
                to: actual_to.clone(),
                label,
                start_label: None,
                end_label: None,
                directed: parsed.arrow.directed,
                arrow_start,
                arrow_end,
                start_decoration: None,
                end_decoration: None,
                style: parsed.arrow.style,
            });

            // Handle - deactivation notation (deactivates after the message)
            if parsed.deactivate_from {
                activation_tracker.deactivate(&from_resolved, step_counter, &mut meta.activations);
            }
            if parsed.deactivate_to {
                activation_tracker.deactivate(&to_resolved, step_counter, &mut meta.activations);
            }

            step_counter += 1;
            continue;
        }
    }

    // Close any remaining activations
    activation_tracker.close_all(step_counter, &mut meta.activations);

    // Close unclosed control blocks
    while let Some((kind, label, start_step, groups)) = control_stack.pop() {
        meta.control_blocks.push(SequenceControlBlock {
            kind,
            label,
            start_step,
            end_step: step_counter,
            groups,
        });
    }

    // Close unclosed rects
    while let Some((color, start_step)) = rect_stack.pop() {
        meta.rects.push(SequenceRect {
            color,
            start_step,
            end_step: step_counter,
        });
    }

    meta.total_steps = step_counter;
    ast.sequence_meta = Some(meta);

    Ok(ast)
}

// ── Parsed message line ──────────────────────────────────────────────

struct ParsedMessage {
    from: String,
    to: String,
    arrow: ArrowInfo,
    label: Option<String>,
    activate_from: bool,
    activate_to: bool,
    deactivate_from: bool,
    deactivate_to: bool,
}

fn parse_message_line(line: &str) -> Option<ParsedMessage> {
    // Strategy: find arrow pattern in the line, split into from/to parts
    // Arrow patterns to try (ordered by length to match longest first):
    let arrow_patterns = [
        "<<-->>", "<<->>", "-->>", "->>", "-->", "->",
        "--x", "-x", "--)", "-)", "<--", "<-",
    ];

    let line = line.trim();

    // Find the arrow in the line
    for &pattern in &arrow_patterns {
        if let Some(pos) = line.find(pattern) {
            let from_part = line[..pos].trim();
            let rest = &line[pos + pattern.len()..];

            // Check for +/- activation suffixes adjacent to arrow
            // From+ or From- at the end of from_part
            let (actual_from, activate_from, deactivate_from) = parse_activation_suffix(from_part);

            let arrow_info = parse_arrow(pattern)?;

            // Rest starts with optional +/-, then to, then optional : label
            let rest = rest.trim_start();
            let (activate_to, deactivate_to, rest) = parse_activation_prefix(rest);

            // Split rest into to_part and label
            let (to_part, label) = if let Some(colon_pos) = rest.find(':') {
                let to = rest[..colon_pos].trim();
                let lbl = rest[colon_pos + 1..].trim().to_string();
                (to.to_string(), if lbl.is_empty() { None } else { Some(lbl) })
            } else {
                (rest.to_string(), None)
            };

            let (actual_to, activate_to2, deactivate_to2) = parse_activation_suffix(&to_part);

            return Some(ParsedMessage {
                from: actual_from,
                to: actual_to,
                arrow: arrow_info,
                label,
                activate_from: activate_from,
                activate_to: activate_to || activate_to2,
                deactivate_from: deactivate_from,
                deactivate_to: deactivate_to || deactivate_to2,
            });
        }
    }

    None
}

/// Parse trailing +/- from a participant name
fn parse_activation_suffix(s: &str) -> (String, bool, bool) {
    let s = s.trim();
    if s.ends_with('+') {
        (s[..s.len()-1].trim().to_string(), true, false)
    } else if s.ends_with('-') {
        (s[..s.len()-1].trim().to_string(), false, true)
    } else {
        (s.to_string(), false, false)
    }
}

/// Parse leading +/- from the rest after arrow
fn parse_activation_prefix(s: &str) -> (bool, bool, &str) {
    if s.starts_with('+') {
        (true, false, &s[1..])
    } else if s.starts_with('-') {
        (false, true, &s[1..])
    } else {
        (false, false, s)
    }
}

// ── Helper: resolve participant name/alias ────────────────────────────

fn resolve_participant(meta: &SequenceMeta, name: &str) -> String {
    let name = name.trim();
    if let Some(id) = meta.aliases.get(name) {
        id.clone()
    } else {
        name.to_string()
    }
}

// ── Helper: ensure participant exists in AST ──────────────────────────

fn ensure_participant(ast: &mut DiagramAst, meta: &mut SequenceMeta, name: &str, is_actor: bool) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    if !ast.nodes.contains_key(name) {
        let shape = if is_actor { NodeShape::Circle } else { NodeShape::Rectangle };
        ast.add_node(DiagramNode {
            id: name.to_string(),
            label: name.to_string(),
            shape,
            style: NodeStyle::default(),
            link: None,
            subgraph: None,
        });
        if !meta.participant_order.contains(&name.to_string()) {
            meta.participant_order.push(name.to_string());
        }
    }
    meta.is_actor.insert(name.to_string(), is_actor);
}

// ── Static for PARTICENT alternative ──────────────────────────────────

static PARTICENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:participant|actor)\s+(.+)$").unwrap()
});

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_directive_sets_ast_title() {
        let input = "sequenceDiagram\n    title: 时序交互\n    participant A\n    A->>B: Hello";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.title.as_deref(), Some("时序交互"));
        assert_eq!(ast.edge_count(), 1);
    }

    #[test]
    fn test_basic_participant_declaration() {
        let input = "sequenceDiagram\n    participant Alice\n    participant Bob\n    Alice->>Bob: Hello";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Sequence);
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.edge_count(), 1);
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.participant_order.len(), 2);
        assert_eq!(meta.participant_order[0], "Alice");
        assert_eq!(meta.participant_order[1], "Bob");
    }

    #[test]
    fn test_participant_with_alias() {
        let input = "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: Hi";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert!(ast.nodes.contains_key("A"));
        assert!(ast.nodes.contains_key("B"));
        // Aliases should be resolved
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.aliases.get("Alice"), Some(&"A".to_string()));
        assert_eq!(meta.aliases.get("Bob"), Some(&"B".to_string()));
    }

    #[test]
    fn test_actor_declaration() {
        let input = "sequenceDiagram\n    actor User\n    actor System\n    User->>System: Request";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.nodes.get("User").unwrap().shape, NodeShape::Circle);
        assert_eq!(ast.nodes.get("System").unwrap().shape, NodeShape::Circle);
    }

    #[test]
    fn test_all_arrow_types() {
        let input = "sequenceDiagram\n\
            A->B: solid open\n\
            A-->B: dashed open\n\
            A->>B: solid arrow\n\
            A-->>B: dashed arrow\n\
            A-xB: solid cross\n\
            A--xB: dashed cross\n\
            A-)B: solid open2\n\
            A--)B: dashed open2\n\
            B<-A: reverse solid\n\
            B<--A: reverse dashed";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.edge_count(), 10);

        // Check arrow styles
        let edges = &ast.edges;
        // ->> solid arrow
        assert_eq!(edges[2].arrow_end, Some(EdgeArrowhead::Arrow));
        assert_eq!(edges[2].style, EdgeStyle::Solid);
        // -->> dashed arrow
        assert_eq!(edges[3].arrow_end, Some(EdgeArrowhead::Arrow));
        assert_eq!(edges[3].style, EdgeStyle::Dashed);
        // -x solid cross
        assert_eq!(edges[4].arrow_end, Some(EdgeArrowhead::Cross));
        // --x dashed cross
        assert_eq!(edges[5].arrow_end, Some(EdgeArrowhead::Cross));
        assert_eq!(edges[5].style, EdgeStyle::Dashed);
    }

    #[test]
    fn test_bidirectional_arrows() {
        let input = "sequenceDiagram\n    A<<->>B: bidir solid\n    A<<-->>B: bidir dashed";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.edge_count(), 2);
        assert_eq!(ast.edges[0].arrow_start, Some(EdgeArrowhead::Arrow));
        assert_eq!(ast.edges[0].arrow_end, Some(EdgeArrowhead::Arrow));
        assert_eq!(ast.edges[1].style, EdgeStyle::Dashed);
    }

    #[test]
    fn test_activation_deactivation() {
        let input = "sequenceDiagram\n\
            participant Server\n\
            Client->>Server: Request\n\
            activate Server\n\
            Server-->>Client: Response\n\
            deactivate Server";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.activations.len(), 1);
        assert_eq!(meta.activations[0].participant_id, "Server");
        assert_eq!(meta.activations[0].start_step, 1); // after first message
        assert_eq!(meta.activations[0].end_step, Some(2)); // after second message
    }

    #[test]
    fn test_plus_minus_notation() {
        let input = "sequenceDiagram\n\
            Client->>+Server: Request\n\
            Server->>+Database: Query\n\
            Database-->>-Server: Data\n\
            Server-->>-Client: Response";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        // Should have activation records
        assert!(!meta.activations.is_empty());
        // Server activated at step 0
        assert!(meta.activations.iter().any(|a| a.participant_id == "Server" && a.start_step == 0));
        // Database activated at step 1
        assert!(meta.activations.iter().any(|a| a.participant_id == "Database" && a.start_step == 1));
    }

    #[test]
    fn test_notes() {
        let input = "sequenceDiagram\n\
            participant Alice\n\
            participant Bob\n\
            Note left of Alice: Alice note\n\
            Note right of Bob: Bob note\n\
            Note over Alice,Bob: Shared note";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.notes.len(), 3);
        assert!(matches!(&meta.notes[0].position, NotePosition::LeftOf(id) if id == "Alice"));
        assert!(matches!(&meta.notes[1].position, NotePosition::RightOf(id) if id == "Bob"));
        assert!(matches!(&meta.notes[2].position, NotePosition::Over(names) if names.len() == 2));
    }

    #[test]
    fn test_loop_block() {
        let input = "sequenceDiagram\n\
            participant A\n\
            participant B\n\
            loop Every 10 seconds\n\
                A->>B: Ping\n\
                B-->>A: Pong\n\
            end";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.control_blocks.len(), 1);
        assert_eq!(meta.control_blocks[0].kind, ControlBlockKind::Loop);
        assert_eq!(meta.control_blocks[0].label, "Every 10 seconds");
        assert_eq!(meta.control_blocks[0].start_step, 0);
        assert_eq!(meta.control_blocks[0].end_step, 2);
    }

    #[test]
    fn test_alt_else_block() {
        let input = "sequenceDiagram\n\
            participant A\n\
            participant B\n\
            alt Condition 1\n\
                A->>B: Action 1\n\
            else Condition 2\n\
                A->>B: Action 2\n\
            end";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.control_blocks.len(), 1);
        assert_eq!(meta.control_blocks[0].kind, ControlBlockKind::Alt);
        assert_eq!(meta.control_blocks[0].groups.len(), 1); // one "else"
        assert_eq!(meta.control_blocks[0].groups[0].0, "Condition 2");
    }

    #[test]
    fn test_par_and_block() {
        let input = "sequenceDiagram\n\
            participant A\n\
            participant B\n\
            par Parallel tasks\n\
                A->>B: Task 1\n\
            and\n\
                B->>A: Task 2\n\
            end";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.control_blocks.len(), 1);
        assert_eq!(meta.control_blocks[0].kind, ControlBlockKind::Par);
        assert_eq!(meta.control_blocks[0].groups.len(), 1);
    }

    #[test]
    fn test_rect_background() {
        let input = "sequenceDiagram\n\
            participant A\n\
            participant B\n\
            rect rgb(200, 150, 100)\n\
                A->>B: Message\n\
            end";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.rects.len(), 1);
        assert!(meta.rects[0].color.contains("rgb"));
    }

    #[test]
    fn test_autonumber() {
        let input = "sequenceDiagram\n\
            autonumber\n\
            A->>B: First\n\
            B->>A: Second";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert!(meta.autonumber);
        assert_eq!(ast.edges[0].label.as_deref(), Some("1. First"));
        assert_eq!(ast.edges[1].label.as_deref(), Some("2. Second"));
    }

    #[test]
    fn test_comments() {
        let input = "sequenceDiagram\n\
            %% This is a comment\n\
            participant A\n\
            %% Another comment\n\
            A->>B: Hello";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.edge_count(), 1);
    }

    #[test]
    fn test_create_destroy() {
        let input = "sequenceDiagram\n\
            participant A\n\
            create participant B\n\
            A->>B: Initialize\n\
            B-->>A: Done\n\
            destroy B";
        let ast = parse_sequence(input).unwrap();
        assert!(ast.nodes.contains_key("B"));
    }

    #[test]
    fn test_self_referencing_message() {
        let input = "sequenceDiagram\n\
            participant A\n\
            A->>A: Self call";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.edges[0].from, "A");
        assert_eq!(ast.edges[0].to, "A");
    }

    #[test]
    fn test_implicit_participant_creation() {
        let input = "sequenceDiagram\n\
            Alice->>Bob: Hello\n\
            Bob-->>Alice: Hi";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.edge_count(), 2);
        // Participants should be auto-created in order of appearance
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.participant_order[0], "Alice");
        assert_eq!(meta.participant_order[1], "Bob");
    }

    #[test]
    fn test_empty_sequence() {
        let input = "sequenceDiagram";
        let ast = parse_sequence(input).unwrap();
        assert_eq!(ast.node_count(), 0);
        assert_eq!(ast.edge_count(), 0);
    }

    #[test]
    fn test_total_steps() {
        let input = "sequenceDiagram\n\
            A->>B: Msg1\n\
            B->>C: Msg2\n\
            C-->>A: Msg3";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.total_steps, 3);
    }

    #[test]
    fn test_opt_block() {
        let input = "sequenceDiagram\n\
            opt Optional condition\n\
                A->>B: Maybe\n\
            end";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.control_blocks.len(), 1);
        assert_eq!(meta.control_blocks[0].kind, ControlBlockKind::Opt);
    }

    #[test]
    fn test_critical_block() {
        let input = "sequenceDiagram\n\
            critical Critical section\n\
                A->>B: Important\n\
            option Fallback\n\
                A->>B: Alternative\n\
            end";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.control_blocks.len(), 1);
        assert_eq!(meta.control_blocks[0].kind, ControlBlockKind::Critical);
        assert_eq!(meta.control_blocks[0].groups.len(), 1);
    }

    #[test]
    fn test_break_block() {
        let input = "sequenceDiagram\n\
            break Error condition\n\
                A->>B: Error msg\n\
            end";
        let ast = parse_sequence(input).unwrap();
        let meta = ast.sequence_meta.as_ref().unwrap();
        assert_eq!(meta.control_blocks.len(), 1);
        assert_eq!(meta.control_blocks[0].kind, ControlBlockKind::Break);
    }
}
