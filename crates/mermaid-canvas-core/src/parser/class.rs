//! Class diagram 解析器
//!
//! 解析 Mermaid classDiagram 语法为 DiagramAst。
//! 支持：类声明、成员/方法、泛型标注、关系（继承/组合/聚合/依赖/实现/关联）、标签。

use std::collections::HashMap;

use crate::diagram::{
    DiagramAst, DiagramEdge, DiagramKind, DiagramNode, Direction, EdgeArrowhead,
    EdgeDecoration, EdgeStyle, NodeShape, NodeStyle,
};
use crate::error::CoreError;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse a Mermaid classDiagram into `DiagramAst`.
pub fn parse_class(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::Class);
    ast.direction = Direction::TopDown;

    let mut members: HashMap<String, Vec<String>> = HashMap::new();
    let mut stereotypes: HashMap<String, Vec<String>> = HashMap::new();
    let mut labels: HashMap<String, String> = HashMap::new();
    let mut current_class: Option<String> = None;

    for raw_line in input.lines() {
        let line = raw_line.trim();

        // blank / comment
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        let lower = line.to_ascii_lowercase();

        // header: classDiagram [direction]
        if lower.starts_with("classdiagram") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                if let Some(dir) = direction_from_token(parts[1]) {
                    ast.direction = dir;
                }
            }
            continue;
        }

        // direction LR / TD etc.
        if let Some(dir) = parse_direction_line(line) {
            ast.direction = dir;
            continue;
        }

        // inside a class body block (between { and })
        if let Some(ref active) = current_class.clone() {
            if let Some(end_idx) = line.find('}') {
                let fragment = line[..end_idx].trim();
                if !fragment.is_empty() {
                    classify_body_entry(active, fragment, &mut members, &mut stereotypes);
                }
                current_class = None;
            } else if is_class_stereotype(line) {
                stereotypes
                    .entry(active.clone())
                    .or_default()
                    .push(line.to_string());
            } else {
                members
                    .entry(active.clone())
                    .or_default()
                    .push(line.to_string());
            }
            continue;
        }

        // relation line — try before class declaration so `class` inside a name doesn't trigger
        if let Some(edge) = parse_class_relation_line(line, &mut ast, &mut labels) {
            ast.add_edge(edge);
            continue;
        }

        // class declaration
        if line.starts_with("class ") {
            let rest = line.trim_start_matches("class ").trim();
            if let Some((id, label, body, open_body)) = parse_class_declaration(rest) {
                if let Some(ref lbl) = label {
                    labels.insert(id.clone(), lbl.clone());
                }
                ensure_node(&mut ast, &id, labels.get(&id).cloned());
                if let Some(body_str) = body {
                    for entry in split_class_body(&body_str) {
                        classify_body_entry(&id, &entry, &mut members, &mut stereotypes);
                    }
                }
                if open_body {
                    current_class = Some(id);
                }
                continue;
            }
        }

        // standalone member line: ClassName : +String name
        if let Some((id, member)) = parse_class_member_line(line) {
            if is_class_stereotype(&member) {
                stereotypes.entry(id).or_default().push(member);
            } else {
                members.entry(id).or_default().push(member);
            }
            continue;
        }
    }

    // Build final labels (stereotype + name + --- + attrs + --- + methods)
    let node_ids: Vec<String> = ast.node_order.clone();
    for id in &node_ids {
        if let Some(node) = ast.nodes.get_mut(id) {
            let class_name = labels
                .get(id)
                .cloned()
                .unwrap_or_else(|| node.label.clone());

            let mut lines: Vec<String> = Vec::new();

            // stereotypes first
            if let Some(st) = stereotypes.get(id) {
                lines.extend(st.iter().cloned());
            }

            lines.push(class_name);

            if let Some(items) = members.get(id) {
                if !items.is_empty() {
                    let mut attrs = Vec::new();
                    let mut methods = Vec::new();
                    for entry in items {
                        let trimmed = entry.trim();
                        if trimmed.contains('(') && trimmed.contains(')') {
                            methods.push(normalize_method_signature(trimmed));
                        } else {
                            attrs.push(trimmed.to_string());
                        }
                    }
                    if !attrs.is_empty() || !methods.is_empty() {
                        lines.push("---".to_string());
                        if !attrs.is_empty() {
                            lines.extend(attrs);
                            if !methods.is_empty() {
                                lines.push("---".to_string());
                                lines.extend(methods);
                            }
                        } else {
                            lines.extend(methods);
                        }
                    }
                }
            }

            node.label = lines.join("\n");
        }
    }

    Ok(ast)
}

// ---------------------------------------------------------------------------
// Helper: classify body entry (stereotype vs member)
// ---------------------------------------------------------------------------

fn classify_body_entry(
    class_id: &str,
    entry: &str,
    members: &mut HashMap<String, Vec<String>>,
    stereotypes: &mut HashMap<String, Vec<String>>,
) {
    if is_class_stereotype(entry) {
        stereotypes
            .entry(class_id.to_string())
            .or_default()
            .push(entry.to_string());
    } else {
        members
            .entry(class_id.to_string())
            .or_default()
            .push(entry.to_string());
    }
}

// ---------------------------------------------------------------------------
// Class declaration
// ---------------------------------------------------------------------------

/// Parse `Animal`, `Animal["Animal Class"]`, `Animal { +String name }`, etc.
/// Returns (id, optional_label, optional_inline_body, body_is_open).
fn parse_class_declaration(input: &str) -> Option<(String, Option<String>, Option<String>, bool)> {
    let mut rest = input.trim();
    if rest.is_empty() {
        return None;
    }

    let mut body: Option<String> = None;
    let mut open_body = false;

    if let Some(open_idx) = rest.find('{') {
        let header = rest[..open_idx].trim();
        let tail = rest[open_idx + 1..].trim();
        if let Some(close_idx) = tail.find('}') {
            let body_str = tail[..close_idx].trim();
            if !body_str.is_empty() {
                body = Some(body_str.to_string());
            }
        } else {
            open_body = true;
        }
        rest = header;
    }

    // "class Label as Id" pattern
    let lower = rest.to_ascii_lowercase();
    if let Some(as_idx) = lower.find(" as ") {
        let label_part = rest[..as_idx].trim();
        let id_part = rest[as_idx + 4..].trim();
        if !id_part.is_empty() {
            let label = strip_quotes(label_part);
            return Some((id_part.to_string(), Some(label), body, open_body));
        }
    }

    // Quoted label: "My Class"
    if let Some(label) = extract_quoted_label(rest) {
        let id = sanitize_id(&label);
        return Some((id, Some(label), body, open_body));
    }

    // Label in brackets: ClassName["Label"]
    if let Some(bracket_idx) = rest.find('[') {
        let id_part = rest[..bracket_idx].trim();
        if !id_part.is_empty() {
            let label_part = rest[bracket_idx + 1..].trim_end_matches(']').trim();
            let label = strip_quotes(label_part);
            return Some((id_part.to_string(), Some(label), body, open_body));
        }
    }

    let id = strip_quotes(rest);
    Some((id, None, body, open_body))
}

// ---------------------------------------------------------------------------
// Class body splitting
// ---------------------------------------------------------------------------

fn split_class_body(body: &str) -> Vec<String> {
    let mut entries = Vec::new();
    for part in body.split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        for line in trimmed.lines() {
            let line_trim = line.trim();
            if !line_trim.is_empty() {
                entries.push(line_trim.to_string());
            }
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Standalone member line: `ClassName : +String name`
// ---------------------------------------------------------------------------

fn parse_class_member_line(line: &str) -> Option<(String, String)> {
    let (left, right) = line.split_once(':')?;
    let id = left.trim();
    let member = right.trim();
    if id.is_empty() || member.is_empty() {
        return None;
    }
    // class IDs don't contain spaces
    if id.contains(' ') {
        return None;
    }
    Some((id.to_string(), member.to_string()))
}

// ---------------------------------------------------------------------------
// Relation parsing
// ---------------------------------------------------------------------------

/// Relation token priority — longest first to avoid prefix collisions.
const RELATION_TOKENS: &[&str] = &[
    "<|..",
    "..|>",
    "<|--",
    "--|>",
    "*--",
    "--*",
    "o--",
    "--o",
    "<..",
    "..>",
    "<--",
    "-->",
    "..",
    "--",
];

struct RelationMeta {
    directed: bool,
    arrow_start: Option<EdgeArrowhead>,
    arrow_end: Option<EdgeArrowhead>,
    start_decoration: Option<EdgeDecoration>,
    end_decoration: Option<EdgeDecoration>,
    style: EdgeStyle,
}

fn edge_meta_from_token(token: &str) -> RelationMeta {
    let has_open_triangle_start = token.contains('<');
    let has_open_triangle_end = token.contains('>');

    // <|.. or <|-- → open triangle on start
    // ..|> or --|> → open triangle on end
    let has_pipe_start = token.starts_with("<|");
    let has_pipe_end = token.contains("|>");

    let directed = has_open_triangle_start || has_open_triangle_end;

    let style = if token.contains("..") {
        EdgeStyle::Dotted
    } else {
        EdgeStyle::Solid
    };

    // Composition (*-- / --*) uses EdgeArrowhead::Diamond for filled diamond
    // Aggregation (o-- / --o) uses EdgeDecoration::Circle for hollow diamond
    // These are mutually exclusive on each side.

    let mut arrow_start = None;
    let mut arrow_end = None;
    let mut start_decoration = None;
    let mut end_decoration = None;

    // *-- → filled diamond at start (composition)
    if token.starts_with('*') {
        arrow_start = Some(EdgeArrowhead::Diamond);
    }
    // --* → filled diamond at end
    if token.ends_with('*') {
        arrow_end = Some(EdgeArrowhead::Diamond);
    }

    // o-- → hollow diamond at start (aggregation)
    if token.starts_with('o') {
        start_decoration = Some(EdgeDecoration::Circle);
    }
    // --o → hollow diamond at end
    if token.ends_with('o') {
        end_decoration = Some(EdgeDecoration::Circle);
    }

    // Triangle / arrow heads (inheritance, dependency, realization, association)
    if has_open_triangle_start && arrow_start.is_none() {
        arrow_start = if has_pipe_start {
            Some(EdgeArrowhead::OpenTriangle)
        } else {
            Some(EdgeArrowhead::Arrow)
        };
    }
    if has_open_triangle_end && arrow_end.is_none() {
        arrow_end = if has_pipe_end {
            Some(EdgeArrowhead::OpenTriangle)
        } else {
            Some(EdgeArrowhead::Arrow)
        };
    }

    RelationMeta {
        directed,
        arrow_start,
        arrow_end,
        start_decoration,
        end_decoration,
        style,
    }
}

/// Parse a relation line and return a `DiagramEdge`.
/// Also ensures nodes exist in `ast`.
fn parse_class_relation_line(
    line: &str,
    ast: &mut DiagramAst,
    labels: &mut HashMap<String, String>,
) -> Option<DiagramEdge> {
    for token in RELATION_TOKENS {
        if let Some(pos) = line.find(token) {
            let left_part = line[..pos].trim();
            let right_part = line[pos + token.len()..].trim();

            if left_part.is_empty() || right_part.is_empty() {
                continue;
            }

            // Extract label after `:` on the full right side
            let (right_part, label) = split_label(right_part);

            // Multiplicity: "1" A  -->  B "many"
            let (left_id, start_label) = split_multiplicity_left(left_part);
            let (right_id, end_label) = split_multiplicity_right(&right_part);

            let (left_id, left_label) = normalize_class_id(&left_id);
            let (right_id, right_label) = normalize_class_id(&right_id);

            if let Some(lbl) = left_label {
                labels.insert(left_id.clone(), lbl);
            }
            if let Some(lbl) = right_label {
                labels.insert(right_id.clone(), lbl);
            }

            ensure_node(ast, &left_id, labels.get(&left_id).cloned());
            ensure_node(ast, &right_id, labels.get(&right_id).cloned());

            let meta = edge_meta_from_token(token);

            return Some(DiagramEdge {
                from: left_id,
                to: right_id,
                label,
                start_label,
                end_label,
                directed: meta.directed,
                arrow_start: meta.arrow_start,
                arrow_end: meta.arrow_end,
                start_decoration: meta.start_decoration,
                end_decoration: meta.end_decoration,
                style: meta.style,
            });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Multiplicity / quoted labels on relations
// ---------------------------------------------------------------------------

/// `"1" Animal` → (Animal, Some("1"))
fn split_multiplicity_left(input: &str) -> (String, Option<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    if let Some((before, value)) = split_trailing_quoted(trimmed) {
        let before = before.trim();
        if !before.is_empty() && !value.is_empty() {
            return (before.to_string(), Some(value.to_string()));
        }
    }
    (trimmed.to_string(), None)
}

/// `Animal "many"` → (Animal, Some("many"))
fn split_multiplicity_right(input: &str) -> (String, Option<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    if let Some((value, rest)) = split_leading_quoted(trimmed) {
        let rest = rest.trim();
        if !rest.is_empty() && !value.is_empty() {
            return (rest.to_string(), Some(value.to_string()));
        }
    }
    (trimmed.to_string(), None)
}

/// Find trailing `"value"` and return (before, value).
fn split_trailing_quoted(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim_end();
    let quote = trimmed.chars().last()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut iter = trimmed.char_indices().rev();
    let _ = iter.next(); // skip closing quote
    for (idx, ch) in iter {
        if ch == quote {
            let before = &trimmed[..idx];
            let value = &trimmed[idx + 1..trimmed.len() - 1];
            return Some((before, value));
        }
    }
    None
}

/// Find leading `"value"` and return (value, rest).
fn split_leading_quoted(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim_start();
    let mut iter = trimmed.char_indices();
    let (_, quote) = iter.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    for (idx, ch) in iter {
        if ch == quote {
            let value = &trimmed[1..idx];
            let rest = &trimmed[idx + 1..];
            return Some((value, rest));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Label splitting (after relation)
// ---------------------------------------------------------------------------

/// `B : label` → (B, Some(label))
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

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

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

fn extract_quoted_label(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Some(strip_quotes(trimmed))
    } else {
        None
    }
}

/// Produce a valid ID from a label (keep alphanumeric + underscore).
fn sanitize_id(label: &str) -> String {
    label
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Normalize a class ID token — strip quotes, return optional label.
fn normalize_class_id(token: &str) -> (String, Option<String>) {
    let trimmed = token.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        let label = strip_quotes(trimmed);
        (sanitize_id(&label), Some(label))
    } else {
        (trimmed.to_string(), None)
    }
}

fn is_class_stereotype(entry: &str) -> bool {
    let trimmed = entry.trim();
    trimmed.starts_with("<<") && trimmed.ends_with(">>") && trimmed.len() > 4
}

/// Normalize method signature: `+foo() String` → `+foo() : String`
fn normalize_method_signature(entry: &str) -> String {
    let trimmed = entry.trim();
    let Some(close_idx) = trimmed.find(')') else {
        return trimmed.to_string();
    };
    let (sig, rest) = trimmed.split_at(close_idx + 1);
    let rest = rest.trim();
    if rest.is_empty() {
        return trimmed.to_string();
    }
    if rest.starts_with(':') {
        return format!("{} {}", sig, rest);
    }
    if trimmed.contains("):") || trimmed.contains(") :") {
        return trimmed.to_string();
    }
    format!("{} : {}", sig, rest)
}

/// Parse a `direction LR` style line.
fn parse_direction_line(line: &str) -> Option<Direction> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() == 2 && parts[0].to_ascii_lowercase() == "direction" {
        return direction_from_token(parts[1]);
    }
    None
}

fn direction_from_token(token: &str) -> Option<Direction> {
    match token.to_ascii_uppercase().as_str() {
        "TD" | "TB" => Some(Direction::TopDown),
        "BT" => Some(Direction::BottomUp),
        "LR" => Some(Direction::LeftToRight),
        "RL" => Some(Direction::RightToLeft),
        _ => None,
    }
}

/// Ensure a class node exists with `Rectangle` shape.
fn ensure_node(ast: &mut DiagramAst, id: &str, label: Option<String>) {
    if !ast.nodes.contains_key(id) {
        ast.add_node(DiagramNode {
            id: id.to_string(),
            label: label.unwrap_or_else(|| id.to_string()),
            shape: NodeShape::Rectangle,
            style: NodeStyle::default(),
            link: None,
            subgraph: None,
        });
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_class() {
        let input = "classDiagram\n    class Animal";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Class);
        assert_eq!(ast.node_count(), 1);
        assert!(ast.nodes.contains_key("Animal"));
        assert_eq!(ast.nodes["Animal"].shape, NodeShape::Rectangle);
    }

    #[test]
    fn test_class_with_label() {
        let input = "classDiagram\n    class Animal[\"Animal Class\"]";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.node_count(), 1);
        assert_eq!(ast.nodes["Animal"].label, "Animal Class");
    }

    #[test]
    fn test_class_with_inline_body() {
        let input = "classDiagram\n    class Animal {\n        +String name\n        +void speak()\n    }";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.node_count(), 1);
        let label = &ast.nodes["Animal"].label;
        assert!(label.contains("Animal"));
        assert!(label.contains("+String name"));
        assert!(label.contains("+void speak()"));
    }

    #[test]
    fn test_stereotype() {
        let input = "classDiagram\n    class Shape {\n        <<interface>>\n        +draw()\n    }";
        let ast = parse_class(input).unwrap();
        let label = &ast.nodes["Shape"].label;
        assert!(label.contains("<<interface>>"));
        assert!(label.contains("+draw()"));
    }

    #[test]
    fn test_inheritance() {
        let input = "classDiagram\n    Animal <|-- Dog";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.edge_count(), 1);
        let edge = &ast.edges[0];
        assert_eq!(edge.from, "Animal");
        assert_eq!(edge.to, "Dog");
        assert!(edge.directed);
        assert_eq!(edge.arrow_start, Some(EdgeArrowhead::OpenTriangle));
    }

    #[test]
    fn test_composition() {
        let input = "classDiagram\n    Engine *-- Car";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.edge_count(), 1);
        let edge = &ast.edges[0];
        // composition uses Diamond arrowhead
        assert_eq!(edge.arrow_start, Some(EdgeArrowhead::Diamond));
        assert_eq!(edge.style, EdgeStyle::Solid);
    }

    #[test]
    fn test_aggregation() {
        let input = "classDiagram\n    Teacher o-- Course";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert_eq!(edge.start_decoration, Some(EdgeDecoration::Circle));
    }

    #[test]
    fn test_dependency() {
        let input = "classDiagram\n    UserService ..> Logger";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert!(edge.directed);
        assert_eq!(edge.arrow_end, Some(EdgeArrowhead::Arrow));
        assert_eq!(edge.style, EdgeStyle::Dotted);
    }

    #[test]
    fn test_realization() {
        let input = "classDiagram\n    Drawable ..|> Circle";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert!(edge.directed);
        assert_eq!(edge.arrow_end, Some(EdgeArrowhead::OpenTriangle));
        assert_eq!(edge.style, EdgeStyle::Dotted);
    }

    #[test]
    fn test_labeled_relation() {
        let input = "classDiagram\n    Animal <|-- Dog : extends";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert_eq!(edge.label.as_deref(), Some("extends"));
    }

    #[test]
    fn test_multiplicity_labels() {
        // Format: ClassName "multiplicity" --> "multiplicity" ClassName
        let input = "classDiagram\n    Author \"1\" --> \"many\" Book";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert_eq!(edge.start_label.as_deref(), Some("1"));
        assert_eq!(edge.end_label.as_deref(), Some("many"));
    }

    #[test]
    fn test_association() {
        let input = "classDiagram\n    Student --> Course";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert!(edge.directed);
        assert_eq!(edge.arrow_end, Some(EdgeArrowhead::Arrow));
        assert_eq!(edge.style, EdgeStyle::Solid);
    }

    #[test]
    fn test_solid_link() {
        let input = "classDiagram\n    A -- B";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert!(!edge.directed);
        assert_eq!(edge.style, EdgeStyle::Solid);
    }

    #[test]
    fn test_dotted_link() {
        let input = "classDiagram\n    A .. B";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert!(!edge.directed);
        assert_eq!(edge.style, EdgeStyle::Dotted);
    }

    #[test]
    fn test_standalone_member() {
        let input = "classDiagram\n    class Animal\n    Animal : +String name\n    Animal : +void speak()";
        let ast = parse_class(input).unwrap();
        let label = &ast.nodes["Animal"].label;
        assert!(label.contains("+String name"));
        assert!(label.contains("+void speak()"));
    }

    #[test]
    fn test_direction_header() {
        let input = "classDiagram LR\n    A --> B";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.direction, Direction::LeftToRight);
    }

    #[test]
    fn test_multiple_classes_and_relations() {
        let input = "classDiagram\n    class Animal {\n        +String name\n    }\n    class Dog\n    Animal <|-- Dog";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.edge_count(), 1);
        assert!(ast.nodes["Animal"].label.contains("+String name"));
    }

    #[test]
    fn test_comments_and_blank_lines() {
        let input = "classDiagram\n    %% This is a comment\n\n    class Foo\n    %% Another comment\n    Foo --> Bar";
        let ast = parse_class(input).unwrap();
        assert_eq!(ast.node_count(), 2);
    }

    #[test]
    fn test_class_with_semicolons() {
        let input = "classDiagram\n    class Person {\n        +String name; +int age\n    }";
        let ast = parse_class(input).unwrap();
        let label = &ast.nodes["Person"].label;
        assert!(label.contains("+String name"));
        assert!(label.contains("+int age"));
    }

    #[test]
    fn test_reverse_relation_tokens() {
        let input = "classDiagram\n    Dog --|> Animal";
        let ast = parse_class(input).unwrap();
        let edge = &ast.edges[0];
        assert_eq!(edge.from, "Dog");
        assert_eq!(edge.to, "Animal");
        assert_eq!(edge.arrow_end, Some(EdgeArrowhead::OpenTriangle));
    }

    #[test]
    fn test_method_return_type_normalization() {
        let input = "classDiagram\n    class Service {\n        +String getName()\n    }";
        let ast = parse_class(input).unwrap();
        let label = &ast.nodes["Service"].label;
        assert!(label.contains("getName()"));
    }

    // ----- unit tests for helpers -----

    #[test]
    fn test_is_class_stereotype() {
        assert!(is_class_stereotype("<<interface>>"));
        assert!(is_class_stereotype("<<abstract>>"));
        assert!(!is_class_stereotype("<<>>"));
        assert!(!is_class_stereotype("interface"));
    }

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn test_split_class_body() {
        let body = "+String name; +int age";
        let entries = split_class_body(body);
        assert_eq!(entries, vec!["+String name", "+int age"]);
    }

    #[test]
    fn test_parse_class_declaration_simple() {
        let result = parse_class_declaration("Animal").unwrap();
        assert_eq!(result.0, "Animal");
        assert!(result.1.is_none());
        assert!(result.2.is_none());
        assert!(!result.3);
    }

    #[test]
    fn test_parse_class_declaration_with_label() {
        let result = parse_class_declaration("Animal[\"My Animal\"]").unwrap();
        assert_eq!(result.0, "Animal");
        assert_eq!(result.1, Some("My Animal".to_string()));
    }

    #[test]
    fn test_parse_class_declaration_with_body() {
        let result = parse_class_declaration("Animal { +String name }").unwrap();
        assert_eq!(result.0, "Animal");
        assert_eq!(result.2, Some("+String name".to_string()));
        assert!(!result.3);
    }

    #[test]
    fn test_parse_class_declaration_open_body() {
        let result = parse_class_declaration("Animal {").unwrap();
        assert_eq!(result.0, "Animal");
        assert!(result.2.is_none());
        assert!(result.3);
    }

    #[test]
    fn test_normalize_method_signature() {
        assert_eq!(normalize_method_signature("+foo()"), "+foo()");
        assert_eq!(normalize_method_signature("+foo() String"), "+foo() : String");
        assert_eq!(normalize_method_signature("+foo() : String"), "+foo() : String");
    }

    #[test]
    fn test_edge_meta_solid_arrow() {
        let meta = edge_meta_from_token("-->");
        assert!(meta.directed);
        assert_eq!(meta.arrow_end, Some(EdgeArrowhead::Arrow));
        assert_eq!(meta.style, EdgeStyle::Solid);
    }

    #[test]
    fn test_edge_meta_dotted_arrow() {
        let meta = edge_meta_from_token("..>");
        assert!(meta.directed);
        assert_eq!(meta.arrow_end, Some(EdgeArrowhead::Arrow));
        assert_eq!(meta.style, EdgeStyle::Dotted);
    }

    #[test]
    fn test_edge_meta_inheritance() {
        let meta = edge_meta_from_token("<|--");
        assert!(meta.directed);
        assert_eq!(meta.arrow_start, Some(EdgeArrowhead::OpenTriangle));
        assert_eq!(meta.style, EdgeStyle::Solid);
    }

    #[test]
    fn test_edge_meta_composition() {
        let meta = edge_meta_from_token("*--");
        assert_eq!(meta.arrow_start, Some(EdgeArrowhead::Diamond));
        assert_eq!(meta.style, EdgeStyle::Solid);
    }
}
