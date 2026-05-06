//! State Diagram 解析器
//!
//! 解析 Mermaid stateDiagram-v2 / stateDiagram 语法为 DiagramAst。

use std::collections::{HashMap, VecDeque};

use crate::diagram::{
    DiagramAst, DiagramEdge, DiagramKind, DiagramNode, Direction, EdgeStyle, NodeShape, NodeStyle,
    Subgraph,
};
use crate::error::CoreError;

/// 解析 state diagram 语法
pub fn parse_state(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::State);

    let mut labels: HashMap<String, String> = HashMap::new();
    let mut descriptions: HashMap<String, Vec<String>> = HashMap::new();
    let mut start_states: HashMap<String, String> = HashMap::new();
    let mut end_states: HashMap<String, String> = HashMap::new();
    let mut subgraph_stack: Vec<usize> = Vec::new();

    let mut pending: VecDeque<String> = input.lines().map(String::from).collect();

    while let Some(raw_line) = pending.pop_front() {
        for raw_statement in split_statements(&raw_line) {
            let line = raw_statement.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            let (line, state_shape, label_override) = parse_state_stereotype(line);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let lower = line.to_ascii_lowercase();
            if lower.starts_with("statediagram") {
                continue;
            }

            // direction LR / direction TB etc.
            if let Some(dir) = parse_direction_line(line) {
                ast.direction = dir;
                continue;
            }

            // style / classDef — skip for now
            if lower.starts_with("style ") || lower.starts_with("classdef ") || lower.starts_with("class ") {
                continue;
            }

            // closing brace — pop composite state
            if line == "}" {
                subgraph_stack.pop();
                continue;
            }

            // region separator inside composite state
            if line == "--" {
                continue;
            }

            // composite state: state "name" { ... }
            if let Some((id, label, tail)) = parse_state_container_header(line) {
                let subgraph_idx = ast.subgraphs.len();
                if let Some(ref id) = id {
                    labels.insert(id.clone(), label.clone());
                }
                ast.add_subgraph(Subgraph {
                    id: id.clone().unwrap_or_else(|| format!("__composite_{}__", subgraph_idx)),
                    label: label.clone(),
                    direction: None,
                    nodes: Vec::new(),
                    style: NodeStyle::default(),
                });
                subgraph_stack.push(subgraph_idx);

                // handle inline body after { on same line
                if !tail.is_empty() {
                    if let Some(close_idx) = tail.find('}') {
                        let body = tail[..close_idx].trim();
                        let after = tail[close_idx + 1..].trim();
                        if !after.is_empty() {
                            pending.push_front(after.to_string());
                        }
                        pending.push_front("}".to_string());
                        if !body.is_empty() {
                            pending.push_front(body.to_string());
                        }
                    } else {
                        pending.push_front(tail);
                    }
                }
                continue;
            }

            // state "Label" as id
            if let Some((id, label)) = parse_state_alias_line(line) {
                let label = label_override.clone().unwrap_or(label);
                labels.insert(id.clone(), label);
                let display = state_display_label(&id, &labels, &descriptions);
                let shape = state_shape.unwrap_or(NodeShape::RoundRect);
                ensure_node(&mut ast, &id, display, shape);
                add_node_to_subgraph(&mut ast, &subgraph_stack, &id);
                continue;
            }

            // transition: A --> B : label
            if let Some((left, meta, right, label)) = parse_state_transition(line) {
                let scope = subgraph_stack
                    .last()
                    .map(|&idx| ast.subgraphs[idx].id.clone())
                    .unwrap_or_else(|| "root".to_string());

                let (left_id, left_shape, left_label) =
                    normalize_state_token(&left, true, &mut start_states, &mut end_states, &scope);
                let (right_id, right_shape, right_label) =
                    normalize_state_token(&right, false, &mut start_states, &mut end_states, &scope);

                let left_display = left_label
                    .or_else(|| state_display_label_option(&left_id, &labels, &descriptions));
                let right_display = right_label
                    .or_else(|| state_display_label_option(&right_id, &labels, &descriptions));

                // Don't override shape if node already exists with a specific shape
                let left_shape = if left_shape == NodeShape::RoundRect
                    && ast.nodes.contains_key(&left_id)
                {
                    ast.nodes[&left_id].shape
                } else {
                    left_shape
                };
                let right_shape = if right_shape == NodeShape::RoundRect
                    && ast.nodes.contains_key(&right_id)
                {
                    ast.nodes[&right_id].shape
                } else {
                    right_shape
                };

                ensure_node(&mut ast, &left_id, left_display, left_shape);
                ensure_node(&mut ast, &right_id, right_display, right_shape);
                add_node_to_subgraph(&mut ast, &subgraph_stack, &left_id);
                add_node_to_subgraph(&mut ast, &subgraph_stack, &right_id);

                ast.add_edge(DiagramEdge {
                    from: left_id,
                    to: right_id,
                    label,
                    start_label: None,
                    end_label: None,
                    directed: meta.directed,
                    arrow_start: if meta.arrow_start { Some(crate::diagram::EdgeArrowhead::Arrow) } else { None },
                    arrow_end: if meta.arrow_end { Some(crate::diagram::EdgeArrowhead::Arrow) } else { None },
                    start_decoration: None,
                    end_decoration: None,
                    style: meta.style,
                });
                continue;
            }

            // description: stateId : description text
            if let Some((id, desc)) = parse_state_description_line(line) {
                let label = label_override.clone().unwrap_or(desc);
                descriptions.entry(id.clone()).or_default().push(label);
                let display = state_display_label(&id, &labels, &descriptions);
                let shape = state_shape.unwrap_or(NodeShape::RoundRect);
                ensure_node(&mut ast, &id, display, shape);
                add_node_to_subgraph(&mut ast, &subgraph_stack, &id);
                continue;
            }

            // note left of / right of
            if let Some((position, target, note_label)) = parse_state_note(line) {
                if !target.is_empty() {
                    if !ast.nodes.contains_key(target.as_str()) {
                        ensure_node(&mut ast, &target, None, NodeShape::RoundRect);
                    }
                    // Create a note node
                    let note_id = format!("__note_{}_{}__", position, target);
                    let note_shape = NodeShape::Rectangle;
                    ensure_node(&mut ast, &note_id, Some(note_label), note_shape);
                    add_node_to_subgraph(&mut ast, &subgraph_stack, &note_id);
                }
                continue;
            }

            // simple state: state id
            if let Some(id) = parse_state_simple(line) {
                if let Some(label) = label_override.clone() {
                    labels.insert(id.clone(), label);
                }
                let display = state_display_label_option(&id, &labels, &descriptions);
                let shape = state_shape.unwrap_or(NodeShape::RoundRect);
                ensure_node(&mut ast, &id, display, shape);
                add_node_to_subgraph(&mut ast, &subgraph_stack, &id);
                continue;
            }

            // bare state id (no keyword) — treat as implicit state declaration
            let id = strip_quotes(line);
            if !id.is_empty() && id != "{" && id != "}" {
                let display = state_display_label_option(&id, &labels, &descriptions);
                let shape = state_shape.unwrap_or(NodeShape::RoundRect);
                ensure_node(&mut ast, &id, display, shape);
                add_node_to_subgraph(&mut ast, &subgraph_stack, &id);
            }
        }
    }

    Ok(ast)
}

// ---------------------------------------------------------------------------
// Helper: edge metadata extracted from arrow tokens
// ---------------------------------------------------------------------------

struct EdgeMeta {
    directed: bool,
    arrow_start: bool,
    arrow_end: bool,
    style: EdgeStyle,
}

fn edge_meta_from_token(token: &str) -> EdgeMeta {
    let arrow_start = token.contains('<');
    let arrow_end = token.contains('>');
    let directed = arrow_start || arrow_end;
    let style = if token.contains("..") {
        EdgeStyle::Dotted
    } else {
        EdgeStyle::Solid
    };
    EdgeMeta {
        directed,
        arrow_start,
        arrow_end,
        style,
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Split a line that may contain multiple statements separated by `;`
fn split_statements(line: &str) -> Vec<&str> {
    // Split on semicolons (but not inside quotes)
    let mut result = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    for (i, ch) in line.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == ';' && !in_quotes {
            result.push(&line[start..i]);
            start = i + 1;
        }
    }
    if start < line.len() {
        result.push(&line[start..]);
    }
    if result.is_empty() {
        result.push(line);
    }
    result
}

/// Parse `direction LR` etc.
fn parse_direction_line(line: &str) -> Option<Direction> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("direction") {
        return match parts[1].to_ascii_uppercase().as_str() {
            "LR" => Some(Direction::LeftToRight),
            "RL" => Some(Direction::RightToLeft),
            "TB" | "TD" => Some(Direction::TopDown),
            "BT" => Some(Direction::BottomUp),
            _ => None,
        };
    }
    None
}

/// Parse stereotype: `state id <<fork>>` → (cleaned_line, shape, label_override)
fn parse_state_stereotype(line: &str) -> (String, Option<NodeShape>, Option<String>) {
    let trimmed = line.trim();
    if !trimmed.starts_with("state ") {
        return (trimmed.to_string(), None, None);
    }
    let Some(start) = trimmed.find("<<") else {
        return (trimmed.to_string(), None, None);
    };
    let Some(end) = trimmed[start + 2..].find(">>") else {
        return (trimmed.to_string(), None, None);
    };
    let stereo_raw = &trimmed[start + 2..start + 2 + end];
    let stereo = stereo_raw.trim().to_ascii_lowercase();

    let before = trimmed[..start].trim_end();
    let after = trimmed[start + 2 + end + 2..].trim_start();
    let cleaned = if after.is_empty() {
        before.to_string()
    } else if before.is_empty() {
        after.to_string()
    } else {
        format!("{before} {after}")
    };

    let (shape, label_override) = match stereo.as_str() {
        "choice" => (Some(NodeShape::Diamond), None),
        "fork" | "join" => (Some(NodeShape::Rectangle), Some(String::new())),
        _ => (None, None),
    };

    (cleaned, shape, label_override)
}

/// Parse `state "Label" as id` → (id, label)
fn parse_state_alias_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("state ") {
        return None;
    }
    if trimmed.contains('{') {
        return None;
    }
    let rest = trimmed.trim_start_matches("state ").trim();
    if !rest.starts_with('"') {
        return None;
    }
    let end_quote = rest[1..].find('"')? + 1;
    let label = rest[1..end_quote].to_string();
    let remaining = rest[end_quote + 1..].trim();
    if !remaining.to_ascii_lowercase().starts_with("as ") {
        return None;
    }
    let id = remaining[3..].trim().to_string();
    if id.is_empty() {
        return None;
    }
    Some((id, label))
}

/// Parse composite state header: `state "name" {` or `state id {`
/// Returns (Option<id>, label, tail_after_brace)
fn parse_state_container_header(line: &str) -> Option<(Option<String>, String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("state ") {
        return None;
    }
    let brace_idx = trimmed.find('{')?;
    let head = trimmed[..brace_idx].trim();
    let tail = trimmed[brace_idx + 1..].trim().to_string();

    let rest = head.trim_start_matches("state ").trim();
    if rest.is_empty() {
        return None;
    }

    // state "Label" as id {
    if rest.starts_with('"') {
        let end_quote = rest[1..].find('"')? + 1;
        let label = rest[1..end_quote].to_string();
        let remaining = rest[end_quote + 1..].trim();
        if remaining.to_ascii_lowercase().starts_with("as ") {
            let id = remaining[3..].trim().to_string();
            if id.is_empty() {
                return None;
            }
            return Some((Some(id), label, tail));
        }
        return Some((None, label, tail));
    }

    // state "Label" {
    // state id {
    let lower = rest.to_ascii_lowercase();
    if let Some(as_idx) = lower.find(" as ") {
        let id_part = rest[..as_idx].trim();
        let label_part = rest[as_idx + 4..].trim();
        if id_part.is_empty() || label_part.is_empty() {
            return None;
        }
        return Some((Some(strip_quotes(id_part)), strip_quotes(label_part), tail));
    }

    let id = strip_quotes(rest);
    Some((Some(id.clone()), id, tail))
}

/// Parse transition: `A --> B : label`
fn parse_state_transition(line: &str) -> Option<(String, EdgeMeta, String, Option<String>)> {
    let tokens = ["<-->", "<--", "-->", "<->", "->", "<-", "..>", "<.."];
    for token in tokens {
        if let Some(pos) = line.find(token) {
            let left = line[..pos].trim();
            let right_part = line[pos + token.len()..].trim();
            if left.is_empty() || right_part.is_empty() {
                continue;
            }
            let (right, label) = split_label(right_part);
            let meta = edge_meta_from_token(token);
            return Some((left.to_string(), meta, right, label));
        }
    }
    None
}

/// Split `B : label` into (B, Some(label)) or (B, None)
fn split_label(input: &str) -> (String, Option<String>) {
    if let Some((left, right)) = input.split_once(':') {
        let label = right.trim();
        let target = left.trim();
        if !label.is_empty() {
            return (target.to_string(), Some(label.to_string()));
        }
        return (target.to_string(), None);
    }
    (input.trim().to_string(), None)
}

/// Normalize `[*]` start/end tokens
fn normalize_state_token(
    token: &str,
    is_start: bool,
    start_states: &mut HashMap<String, String>,
    end_states: &mut HashMap<String, String>,
    scope: &str,
) -> (String, NodeShape, Option<String>) {
    let trimmed = token.trim();
    if trimmed == "[*]" || trimmed == "*" {
        let (id, shape) = if is_start {
            let id = start_states
                .entry(scope.to_string())
                .or_insert_with(|| format!("__start_{}__", scope))
                .clone();
            (id, NodeShape::Circle)
        } else {
            let id = end_states
                .entry(scope.to_string())
                .or_insert_with(|| format!("__end_{}__", scope))
                .clone();
            (id, NodeShape::DoubleCircle)
        };
        return (id, shape, Some(String::new()));
    }
    (strip_quotes(trimmed), NodeShape::RoundRect, None)
}

/// Parse description: `stateId : description text`
fn parse_state_description_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.to_ascii_lowercase().starts_with("note ") {
        return None;
    }
    let rest = if trimmed.starts_with("state ") {
        trimmed[6..].trim()
    } else {
        trimmed
    };
    if rest.to_ascii_lowercase().contains(" as ") {
        return None;
    }

    // Find first colon that isn't part of :::
    let mut sep = None;
    let bytes = rest.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b':' {
            if idx + 2 < bytes.len() && bytes[idx + 1] == b':' && bytes[idx + 2] == b':' {
                idx += 3;
                continue;
            }
            sep = Some(idx);
            break;
        }
        idx += 1;
    }
    let sep = sep?;
    let (id_part, desc_part) = rest.split_at(sep);
    let desc_part = desc_part.get(1..).unwrap_or("");
    let id = strip_quotes(id_part.trim());
    let desc = strip_quotes(desc_part.trim());
    if id.is_empty() || desc.is_empty() {
        return None;
    }
    Some((id, desc))
}

/// Parse note: `note left of A: text` / `note right of B: text`
fn parse_state_note(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("note ") {
        return None;
    }
    let rest = trimmed[4..].trim();
    let lower_rest = rest.to_ascii_lowercase();
    let (position, targets_part) = if lower_rest.starts_with("right of ") {
        ("right", rest[9..].trim())
    } else if lower_rest.starts_with("left of ") {
        ("left", rest[8..].trim())
    } else {
        return None;
    };
    let (target, label) = targets_part.split_once(':')?;
    let target = target.trim();
    let label = label.trim();
    if target.is_empty() || label.is_empty() {
        return None;
    }
    Some((position.to_string(), target.to_string(), label.to_string()))
}

/// Parse simple state: `state id`
fn parse_state_simple(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("state ") {
        return None;
    }
    if trimmed.contains('{') {
        return None;
    }
    let rest = trimmed.trim_start_matches("state ").trim();
    if rest.to_ascii_lowercase().contains(" as ") {
        return None;
    }
    if rest.is_empty() {
        return None;
    }
    let id = strip_quotes(rest);
    if id.is_empty() {
        return None;
    }
    Some(id)
}

/// Build the display label for a state node
fn state_display_label(
    id: &str,
    labels: &HashMap<String, String>,
    descriptions: &HashMap<String, Vec<String>>,
) -> Option<String> {
    if !labels.contains_key(id) && !descriptions.contains_key(id) {
        return None;
    }
    Some(state_display_label_force(id, labels, descriptions))
}

fn state_display_label_force(
    id: &str,
    labels: &HashMap<String, String>,
    descriptions: &HashMap<String, Vec<String>>,
) -> String {
    let title = labels.get(id).map(String::as_str).unwrap_or(id);
    let Some(descs) = descriptions.get(id) else {
        return title.to_string();
    };
    if descs.is_empty() {
        return title.to_string();
    }

    let mut label = String::with_capacity(
        title.len() + descs.iter().map(String::len).sum::<usize>() + descs.len() + 4,
    );
    label.push_str(title);
    label.push_str("\n---");
    for desc in descs {
        label.push('\n');
        label.push_str(desc);
    }
    label
}

fn state_display_label_option(
    id: &str,
    labels: &HashMap<String, String>,
    descriptions: &HashMap<String, Vec<String>>,
) -> Option<String> {
    state_display_label(id, labels, descriptions)
}

/// Strip surrounding quotes from a string
fn strip_quotes(input: &str) -> String {
    let trimmed = input.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Ensure a node exists in the AST (insert if absent)
fn ensure_node(ast: &mut DiagramAst, id: &str, label: Option<String>, shape: NodeShape) {
    if !ast.nodes.contains_key(id) {
        let node = DiagramNode {
            id: id.to_string(),
            label: label.unwrap_or_else(|| id.to_string()),
            shape,
            style: NodeStyle::default(),
            link: None,
            subgraph: None,
        };
        ast.add_node(node);
    }
}

/// Add a node to the current subgraph (if inside a composite state)
fn add_node_to_subgraph(ast: &mut DiagramAst, stack: &[usize], node_id: &str) {
    if let Some(&idx) = stack.last() {
        if let Some(sg) = ast.subgraphs.get_mut(idx) {
            if !sg.nodes.contains(&node_id.to_string()) {
                sg.nodes.push(node_id.to_string());
            }
        }
        // Also set the node's subgraph field
        if let Some(node) = ast.nodes.get_mut(node_id) {
            node.subgraph = Some(idx);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_transition() {
        let input = r#"stateDiagram-v2
    Idle --> Running : start
    Running --> Idle : stop
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::State);
        assert_eq!(ast.edge_count(), 2);
        assert_eq!(ast.node_count(), 2);
        assert!(ast.nodes.contains_key("Idle"));
        assert!(ast.nodes.contains_key("Running"));

        let edge = &ast.edges[0];
        assert_eq!(edge.from, "Idle");
        assert_eq!(edge.to, "Running");
        assert_eq!(edge.label.as_deref(), Some("start"));
    }

    #[test]
    fn test_start_end_states() {
        let input = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Running
    Running --> [*]
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.edge_count(), 3);
        // 4 nodes: __start_root__, Idle, Running, __end_root__
        assert_eq!(ast.node_count(), 4);

        // Start state should be Circle shape
        let start_id = "__start_root__";
        assert!(ast.nodes.contains_key(start_id));
        assert_eq!(ast.nodes[start_id].shape, NodeShape::Circle);
        assert!(ast.nodes[start_id].label.is_empty());

        // End state should be DoubleCircle shape
        let end_id = "__end_root__";
        assert!(ast.nodes.contains_key(end_id));
        assert_eq!(ast.nodes[end_id].shape, NodeShape::DoubleCircle);
        assert!(ast.nodes[end_id].label.is_empty());

        // First edge: [*] --> Idle
        assert_eq!(ast.edges[0].from, start_id);
        assert_eq!(ast.edges[0].to, "Idle");

        // Last edge: Running --> [*]
        assert_eq!(ast.edges[2].from, "Running");
        assert_eq!(ast.edges[2].to, end_id);
    }

    #[test]
    fn test_composite_state() {
        let input = r#"stateDiagram-v2
    state "Active" as active {
        Idle --> Running
    }
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.subgraphs.len(), 1);
        let sg = &ast.subgraphs[0];
        assert_eq!(sg.id, "active");
        assert_eq!(sg.label, "Active");
        assert!(sg.nodes.contains(&"Idle".to_string()));
        assert!(sg.nodes.contains(&"Running".to_string()));
        assert_eq!(ast.edge_count(), 1);
    }

    #[test]
    fn test_fork_join() {
        let input = r#"stateDiagram-v2
    state fork_state <<fork>>
    state join_state <<join>>
    Idle --> fork_state
    fork_state --> Running
    Running --> join_state
    join_state --> Done
"#;
        let ast = parse_state(input).unwrap();
        // Fork/join should use Rectangle shape
        assert_eq!(ast.nodes["fork_state"].shape, NodeShape::Rectangle);
        assert_eq!(ast.nodes["join_state"].shape, NodeShape::Rectangle);
    }

    #[test]
    fn test_choice() {
        let input = r#"stateDiagram-v2
    state choice_state <<choice>>
    Idle --> choice_state
    choice_state --> Running : yes
    choice_state --> Stopped : no
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.nodes["choice_state"].shape, NodeShape::Diamond);
    }

    #[test]
    fn test_notes() {
        let input = r#"stateDiagram-v2
    Idle
    note left of Idle: Initial state
"#;
        let ast = parse_state(input).unwrap();
        assert!(ast.nodes.contains_key("Idle"));
        assert!(ast.nodes.contains_key("__note_left_Idle__"));
        assert_eq!(ast.nodes["__note_left_Idle__"].label, "Initial state");
    }

    #[test]
    fn test_description() {
        let input = r#"stateDiagram-v2
    Idle : Waiting for input
    Running : Processing data
"#;
        let ast = parse_state(input).unwrap();
        // Descriptions are appended to labels
        assert!(ast.nodes["Idle"].label.contains("Waiting for input"));
        assert!(ast.nodes["Running"].label.contains("Processing data"));
    }

    #[test]
    fn test_direction() {
        let input = r#"stateDiagram-v2
    direction LR
    A --> B
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.direction, Direction::LeftToRight);
    }

    #[test]
    fn test_bidirectional_arrow() {
        let input = r#"stateDiagram-v2
    A <-> B
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        let edge = &ast.edges[0];
        assert!(edge.directed);
        assert!(edge.arrow_start.is_some());
        assert!(edge.arrow_end.is_some());
    }

    #[test]
    fn test_state_alias() {
        let input = r#"stateDiagram-v2
    state "Waiting" as W
    state "Running" as R
    W --> R
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.nodes["W"].label, "Waiting");
        assert_eq!(ast.nodes["R"].label, "Running");
    }

    #[test]
    fn test_comments_and_blank_lines() {
        let input = r#"stateDiagram-v2
    %% This is a comment
    A --> B

    %% Another comment
    B --> C
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.edge_count(), 2);
    }

    #[test]
    fn test_backward_arrow() {
        let input = r#"stateDiagram-v2
    A <-- B
"#;
        let ast = parse_state(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        let edge = &ast.edges[0];
        assert_eq!(edge.from, "A");
        assert_eq!(edge.to, "B");
        // <-- has arrow_start=true, arrow_end=false
        assert!(edge.arrow_start.is_some());
    }
}
