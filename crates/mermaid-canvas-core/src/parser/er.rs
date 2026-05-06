//! ER 图解析器
//!
//! 解析 Mermaid erDiagram 语法为 DiagramAst。

use std::collections::HashMap;

use crate::diagram::{
    DiagramAst, DiagramEdge, DiagramKind, DiagramNode, Direction, EdgeStyle, NodeShape, NodeStyle,
};
use crate::error::CoreError;

/// 解析 ER 图语法
pub fn parse_er(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::Er);
    ast.direction = Direction::TopDown;

    let mut members: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_entity: Option<String> = None;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // Skip header
        if line.to_ascii_lowercase().starts_with("erdiagram") {
            continue;
        }

        // Direction override
        if let Some(dir) = try_parse_direction(line) {
            ast.direction = dir;
            continue;
        }

        // Inside entity body
        if let Some(active) = current_entity.clone() {
            if let Some(end_idx) = line.find('}') {
                let fragment = line[..end_idx].trim();
                if !fragment.is_empty() {
                    members.entry(active.clone()).or_default().push(fragment.to_string());
                }
                current_entity = None;
            } else {
                members.entry(active.clone()).or_default().push(line.to_string());
            }
            continue;
        }

        // Try relation: EntityA ||--o{ EntityB : "label"
        if let Some((left, right, label, left_card, right_card, style)) = parse_er_relation(line) {
            ensure_er_node(&mut ast, &left);
            ensure_er_node(&mut ast, &right);

            // Build edge label from cardinality
            let edge_label = match (label, left_card.as_deref(), right_card.as_deref()) {
                (Some(l), _, _) => Some(l),
                (None, Some(lc), Some(rc)) => Some(format!("{} : {}", lc, rc)),
                (None, Some(c), None) | (None, None, Some(c)) => Some(c.to_string()),
                _ => None,
            };

            ast.add_edge(DiagramEdge {
                from: left,
                to: right,
                label: edge_label,
                start_label: None,
                end_label: None,
                directed: false,
                arrow_start: None,
                arrow_end: None,
                start_decoration: None,
                end_decoration: None,
                style,
            });
            continue;
        }

        // Entity with body: EntityName {
        if let Some(open_idx) = line.find('{') {
            let name = strip_quotes(line[..open_idx].trim());
            if !name.is_empty() {
                ensure_er_node(&mut ast, &name);
                current_entity = Some(name.clone());
                let tail = line[open_idx + 1..].trim();
                if let Some(close_idx) = tail.find('}') {
                    let fragment = tail[..close_idx].trim();
                    if !fragment.is_empty() {
                        members.entry(name).or_default().push(fragment.to_string());
                    }
                    current_entity = None;
                } else if !tail.is_empty() {
                    members.entry(name).or_default().push(tail.to_string());
                }
            }
            continue;
        }

        // Standalone entity name
        let entity = strip_quotes(line);
        if !entity.is_empty() {
            ensure_er_node(&mut ast, &entity);
        }
    }

    // Merge members into node labels
    let node_ids: Vec<String> = ast.nodes.keys().cloned().collect();
    for id in node_ids {
        if let Some(attrs) = members.get(&id) {
            if !attrs.is_empty() {
                if let Some(node) = ast.nodes.get_mut(&id) {
                    let mut lines = vec![node.label.clone()];
                    lines.push("---".to_string());
                    lines.extend(attrs.iter().cloned());
                    node.label = lines.join("\n");
                }
            }
        }
    }

    Ok(ast)
}

fn ensure_er_node(ast: &mut DiagramAst, id: &str) {
    if !ast.nodes.contains_key(id) {
        ast.add_node(DiagramNode {
            id: id.to_string(),
            label: id.to_string(),
            shape: NodeShape::RoundRect,
            style: NodeStyle::default(),
            link: None,
            subgraph: None,
        });
    }
}

/// Parse ER relation: `EntityA ||--o{ EntityB : label`
fn parse_er_relation(line: &str) -> Option<(String, String, Option<String>, Option<String>, Option<String>, EdgeStyle)> {
    let (relation_part, label) = if let Some((before, after)) = line.split_once(':') {
        let l = after.trim();
        (before.trim(), if l.is_empty() { None } else { Some(l.to_string()) })
    } else {
        (line.trim(), None)
    };

    let (sep, style) = if let Some(idx) = relation_part.find("--") {
        (idx, EdgeStyle::Solid)
    } else if let Some(idx) = relation_part.find("..") {
        (idx, EdgeStyle::Dotted)
    } else {
        return None;
    };

    let left_part = relation_part[..sep].trim();
    let right_part = relation_part[sep + 2..].trim();
    if left_part.is_empty() || right_part.is_empty() {
        return None;
    }

    let (left_entity, left_card) = split_cardinality_left(left_part);
    let (right_entity, right_card) = split_cardinality_right(right_part);

    if left_entity.is_empty() || right_entity.is_empty() {
        return None;
    }

    let left_id = strip_quotes(&left_entity);
    let right_id = strip_quotes(&right_entity);
    if left_id.is_empty() || right_id.is_empty() {
        return None;
    }

    let lc = left_card.map(|t| normalize_cardinality(&t));
    let rc = right_card.map(|t| normalize_cardinality(&t));

    Some((left_id, right_id, label, lc, rc, style))
}

fn split_cardinality_left(input: &str) -> (String, Option<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let len = chars.len();
    // Check last 2 chars
    if len >= 2 {
        let last_two: String = chars[len - 2..].iter().collect();
        if last_two.chars().all(is_er_card_char) {
            let entity: String = chars[..len - 2].iter().collect();
            return (entity.trim().to_string(), Some(last_two));
        }
    }
    // Check last 1 char
    if let Some(&last) = chars.last() {
        if is_er_card_char(last) {
            let entity: String = chars[..len - 1].iter().collect();
            return (entity.trim().to_string(), Some(last.to_string()));
        }
    }
    (trimmed.to_string(), None)
}

fn split_cardinality_right(input: &str) -> (String, Option<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let len = chars.len();
    if len >= 2 {
        let first_two: String = chars[..2].iter().collect();
        if first_two.chars().all(is_er_card_char) {
            let entity: String = chars[2..].iter().collect();
            return (entity.trim().to_string(), Some(first_two));
        }
    }
    if !chars.is_empty() && is_er_card_char(chars[0]) {
        let entity: String = chars[1..].iter().collect();
        return (entity.trim().to_string(), Some(chars[0].to_string()));
    }
    (trimmed.to_string(), None)
}

fn is_er_card_char(ch: char) -> bool {
    matches!(ch, '|' | 'o' | '{' | '}')
}

fn normalize_cardinality(token: &str) -> String {
    match token.trim() {
        "||" | "|" => "1".to_string(),
        "o|" | "|o" | "o" => "0..1".to_string(),
        "|{" | "}|" => "1..*".to_string(),
        "o{" | "}o" | "}" | "{" => "0..*".to_string(),
        other => other.to_string(),
    }
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim().trim_matches('"');
    t.to_string()
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
    fn test_simple_entity() {
        let input = "erDiagram\n    Customer";
        let ast = parse_er(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Er);
        assert_eq!(ast.node_count(), 1);
        assert!(ast.nodes.contains_key("Customer"));
    }

    #[test]
    fn test_entity_with_attributes() {
        let input = "erDiagram\n    Customer {\n        string name\n        int age\n    }";
        let ast = parse_er(input).unwrap();
        let node = ast.nodes.get("Customer").unwrap();
        assert!(node.label.contains("name"));
        assert!(node.label.contains("age"));
    }

    #[test]
    fn test_relation() {
        let input = "erDiagram\n    Customer ||--o{ Order : places";
        let ast = parse_er(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        let edge = &ast.edges[0];
        assert_eq!(edge.from, "Customer");
        assert_eq!(edge.to, "Order");
        assert!(!edge.directed);
        assert_eq!(edge.label.as_deref(), Some("places"));
    }

    #[test]
    fn test_relation_cardinality() {
        let input = "erDiagram\n    CAR ||--o{ PERSON : drives";
        let ast = parse_er(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
    }

    #[test]
    fn test_multiple_entities_and_relations() {
        let input = "\
erDiagram
    Customer ||--o{ Order : places
    Order ||--|{ LineItem : contains
    Customer {
        string name
    }
    Order {
        int id
    }";
        let ast = parse_er(input).unwrap();
        assert_eq!(ast.node_count(), 3);
        assert_eq!(ast.edge_count(), 2);
        assert!(ast.nodes.get("Customer").unwrap().label.contains("name"));
        assert!(ast.nodes.get("Order").unwrap().label.contains("id"));
    }
}
