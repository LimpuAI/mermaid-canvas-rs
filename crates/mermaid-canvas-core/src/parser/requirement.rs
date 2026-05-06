//! Requirement 图解析器
//!
//! 解析 Mermaid requirementDiagram 语法为 DiagramAst。

use std::collections::HashMap;

use crate::diagram::{
    DiagramAst, DiagramEdge, DiagramKind, DiagramNode, Direction, EdgeStyle, NodeShape, NodeStyle,
};
use crate::error::CoreError;

/// 解析 Requirement 图语法
pub fn parse_requirement(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::Requirement);
    ast.direction = Direction::TopDown;

    let mut attributes: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_id: Option<String> = None;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if line.to_ascii_lowercase().starts_with("requirementdiagram") {
            continue;
        }
        if let Some(dir) = try_parse_direction(line) {
            ast.direction = dir;
            continue;
        }

        // Inside element body
        if let Some(active) = current_id.clone() {
            if let Some(end_idx) = line.find('}') {
                let fragment = line[..end_idx].trim();
                if !fragment.is_empty() {
                    attributes.entry(active.clone()).or_default().push(fragment.to_string());
                }
                current_id = None;
            } else {
                attributes.entry(active.clone()).or_default().push(line.to_string());
            }
            continue;
        }

        // Relation: A -type-> B or B <-type- A
        if let Some((from, rel, to)) = parse_relation_line(line) {
            ensure_req_node(&mut ast, &from, "");
            ensure_req_node(&mut ast, &to, "");
            let is_contains = rel == "contains";
            ast.add_edge(DiagramEdge {
                from,
                to,
                label: Some(rel),
                start_label: None,
                end_label: None,
                directed: true,
                arrow_start: None,
                arrow_end: Some(crate::diagram::EdgeArrowhead::Arrow),
                start_decoration: None,
                end_decoration: None,
                style: EdgeStyle::Solid,
            });
            continue;
        }

        // Block header: requirement id { ... } or element id { ... }
        if let Some(open_idx) = line.find('{') {
            let header = line[..open_idx].trim();
            if let Some((kind, id)) = parse_req_header(header) {
                push_req_node(&mut ast, &kind, &id);
                current_id = Some(id.clone());
                let tail = line[open_idx + 1..].trim();
                if let Some(close_idx) = tail.find('}') {
                    let fragment = tail[..close_idx].trim();
                    if !fragment.is_empty() {
                        attributes.entry(id).or_default().push(fragment.to_string());
                    }
                    current_id = None;
                } else if !tail.is_empty() {
                    attributes.entry(id).or_default().push(tail.to_string());
                }
            }
            continue;
        }

        // Standalone: requirement id / element id
        if let Some((kind, id)) = parse_req_header(line) {
            push_req_node(&mut ast, &kind, &id);
        }
    }

    // Merge attributes into node labels
    let node_ids: Vec<String> = ast.nodes.keys().cloned().collect();
    for id in node_ids {
        if let Some(attrs) = attributes.get(&id) {
            if !attrs.is_empty() {
                if let Some(node) = ast.nodes.get_mut(&id) {
                    let mut lines = vec![node.label.clone()];
                    lines.extend(attrs.iter().map(|a| normalize_attr(a)));
                    node.label = lines.join("\n");
                }
            }
        }
    }

    Ok(ast)
}

fn ensure_req_node(ast: &mut DiagramAst, id: &str, _kind: &str) {
    if !ast.nodes.contains_key(id) {
        ast.add_node(DiagramNode {
            id: id.to_string(),
            label: id.to_string(),
            shape: NodeShape::Rectangle,
            style: NodeStyle::default(),
            link: None,
            subgraph: None,
        });
    }
}

fn push_req_node(ast: &mut DiagramAst, kind: &str, id: &str) {
    let kind_label = kind_label(kind);
    let label = if kind_label.is_empty() {
        id.to_string()
    } else {
        format!("<<{}>>\n{}", kind_label, id)
    };
    if !ast.nodes.contains_key(id) {
        ast.add_node(DiagramNode {
            id: id.to_string(),
            label,
            shape: NodeShape::Rectangle,
            style: NodeStyle::default(),
            link: None,
            subgraph: None,
        });
    }
}

fn kind_label(kind: &str) -> String {
    match kind.to_ascii_lowercase().as_str() {
        "requirement" => "Requirement".to_string(),
        "functionalrequirement" => "Functional Requirement".to_string(),
        "interfacerequirement" => "Interface Requirement".to_string(),
        "performancerequirement" => "Performance Requirement".to_string(),
        "physicalrequirement" => "Physical Requirement".to_string(),
        "designconstraint" => "Design Constraint".to_string(),
        "element" => "Element".to_string(),
        other => other.to_string(),
    }
}

fn normalize_attr(line: &str) -> String {
    if let Some((key_raw, value_raw)) = line.split_once(':') {
        let key = key_raw.trim().to_ascii_lowercase();
        let value = value_raw.trim().trim_matches('"').to_string();
        let pretty_key = match key.as_str() {
            "id" => "ID".to_string(),
            "text" => "Text".to_string(),
            "risk" => "Risk".to_string(),
            "verifymethod" | "verification" => "Verification".to_string(),
            "docref" => "Doc Ref".to_string(),
            other => kind_label(other),
        };
        if value.is_empty() {
            pretty_key
        } else {
            format!("{}: {}", pretty_key, value)
        }
    } else {
        line.trim().trim_matches('"').to_string()
    }
}

/// Parse: `A -relation-> B` or `B <-relation- A`
fn parse_relation_line(line: &str) -> Option<(String, String, String)> {
    // Forward: from -rel-> to
    if let Some((left, right)) = line.split_once("->") {
        let to = right.trim().trim_matches('"').to_string();
        let (from_part, rel_part) = left.trim().split_once('-')?;
        let from = from_part.trim().trim_matches('"').to_string();
        let rel = normalize_relation(rel_part)?;
        if from.is_empty() || to.is_empty() {
            return None;
        }
        return Some((from, rel, to));
    }

    // Backward: to <-rel- from
    if let Some((left, right)) = line.split_once("<-") {
        let to = left.trim().trim_matches('"').to_string();
        let (rel_part, from_part) = right.trim().split_once('-')?;
        let from = from_part.trim().trim_matches('"').to_string();
        let rel = normalize_relation(rel_part)?;
        if from.is_empty() || to.is_empty() {
            return None;
        }
        return Some((from, rel, to));
    }

    None
}

fn normalize_relation(raw: &str) -> Option<String> {
    let rel = raw.trim().trim_matches('-').trim().trim_start_matches('<').trim_end_matches('>').trim().to_ascii_lowercase();
    match rel.as_str() {
        "contains" | "copies" | "derives" | "satisfies" | "verifies" | "refines" | "traces" => Some(rel),
        _ => None,
    }
}

/// Parse: `requirement id` or `element id` etc.
fn parse_req_header(header: &str) -> Option<(String, String)> {
    let trimmed = header.trim();
    let split_at = trimmed.find(char::is_whitespace)?;
    let kind = trimmed[..split_at].trim().to_string();
    let rest = trimmed[split_at..].trim();
    if kind.is_empty() || rest.is_empty() {
        return None;
    }
    let id = rest.trim_matches('"').to_string();
    if id.is_empty() {
        return None;
    }
    Some((kind, id))
}

fn try_parse_direction(line: &str) -> Option<Direction> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() == 2 && parts[0] == "direction" {
        match parts[1] {
            "TD" | "TB" => Some(Direction::TopDown),
            "BT" => Some(Direction::BottomUp),
            "LR" => Some(Direction::LeftToRight),
            "RL" => Some(Direction::RightToLeft),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_requirement() {
        let input = "requirementDiagram\n    requirement test_req";
        let ast = parse_requirement(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Requirement);
        assert_eq!(ast.node_count(), 1);
        assert!(ast.nodes.contains_key("test_req"));
    }

    #[test]
    fn test_requirement_with_attributes() {
        let input = "\
requirementDiagram
    requirement test_req {
        id: 1
        text: the test text
        risk: high
    }";
        let ast = parse_requirement(input).unwrap();
        let node = ast.nodes.get("test_req").unwrap();
        assert!(node.label.contains("<<Requirement>>"));
        assert!(node.label.contains("ID: 1"));
        assert!(node.label.contains("Text: the test text"));
    }

    #[test]
    fn test_relation_traces() {
        let input = "\
requirementDiagram
    requirement req1
    requirement req2
    req1 -traces-> req2";
        let ast = parse_requirement(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.edges[0].label.as_deref(), Some("traces"));
        assert!(ast.edges[0].directed);
    }

    #[test]
    fn test_relation_contains() {
        let input = "\
requirementDiagram
    requirement parent
    requirement child
    parent -contains-> child";
        let ast = parse_requirement(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.edges[0].label.as_deref(), Some("contains"));
    }

    #[test]
    fn test_element() {
        let input = "\
requirementDiagram
    element my_element";
        let ast = parse_requirement(input).unwrap();
        assert_eq!(ast.node_count(), 1);
        assert!(ast.nodes.get("my_element").unwrap().label.contains("<<Element>>"));
    }

    #[test]
    fn test_backward_relation() {
        let input = "\
requirementDiagram
    requirement A
    requirement B
    B <-derives- A";
        let ast = parse_requirement(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        assert_eq!(ast.edges[0].from, "A");
        assert_eq!(ast.edges[0].to, "B");
        assert_eq!(ast.edges[0].label.as_deref(), Some("derives"));
    }
}
