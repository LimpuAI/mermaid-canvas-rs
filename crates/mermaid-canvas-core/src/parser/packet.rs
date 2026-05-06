//! Packet 图解析器
//!
//! 解析 Mermaid packet 语法为 DiagramAst。

use crate::diagram::{
    DiagramAst, DiagramEdge, DiagramKind, DiagramNode, Direction, EdgeStyle, NodeShape, NodeStyle,
};
use crate::error::CoreError;

/// 解析 Packet 图语法
pub fn parse_packet(input: &str) -> Result<DiagramAst, CoreError> {
    let mut ast = DiagramAst::new(DiagramKind::Packet);
    ast.direction = Direction::LeftToRight;

    let mut last_node: Option<String> = None;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        // Skip header/title
        if line.to_ascii_lowercase().starts_with("packet") || line.to_ascii_lowercase().starts_with("title") {
            continue;
        }

        // Field: range : label  (e.g., 0-7 : source)
        if let Some((range, label)) = line.split_once(':') {
            let range = range.trim();
            let label = label.trim().trim_matches('"').to_string();
            if range.is_empty() {
                continue;
            }
            let node_id = format!("pkt_{}", ast.nodes.len());
            let node_label = if label.is_empty() {
                range.to_string()
            } else {
                format!("{}\n{}", range, label)
            };
            ast.add_node(DiagramNode {
                id: node_id.clone(),
                label: node_label,
                shape: NodeShape::Rectangle,
                style: NodeStyle::default(),
                link: None,
                subgraph: None,
            });

            // Chain nodes sequentially
            if let Some(prev) = last_node.take() {
                ast.add_edge(DiagramEdge {
                    from: prev,
                    to: node_id.clone(),
                    label: None,
                    start_label: None,
                    end_label: None,
                    directed: false,
                    arrow_start: None,
                    arrow_end: None,
                    start_decoration: None,
                    end_decoration: None,
                    style: EdgeStyle::Solid,
                });
            }
            last_node = Some(node_id);
        }
    }

    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_packet() {
        let input = "\
packet
    title Simple
    0-3 : source
    4-7 : dest";
        let ast = parse_packet(input).unwrap();
        assert_eq!(ast.kind, DiagramKind::Packet);
        assert_eq!(ast.node_count(), 2);
        assert_eq!(ast.edge_count(), 1);
        assert!(ast.nodes.get("pkt_0").unwrap().label.contains("source"));
        assert!(ast.nodes.get("pkt_1").unwrap().label.contains("dest"));
    }

    #[test]
    fn test_packet_sequential() {
        let input = "\
packet
    0-7 : header
    8-15 : payload
    16-23 : trailer";
        let ast = parse_packet(input).unwrap();
        assert_eq!(ast.node_count(), 3);
        assert_eq!(ast.edge_count(), 2);
        // pkt_0 -> pkt_1 -> pkt_2
        assert_eq!(ast.edges[0].from, "pkt_0");
        assert_eq!(ast.edges[0].to, "pkt_1");
        assert_eq!(ast.edges[1].from, "pkt_1");
        assert_eq!(ast.edges[1].to, "pkt_2");
    }

    #[test]
    fn test_packet_no_label() {
        let input = "packet\n    0-7 :";
        let ast = parse_packet(input).unwrap();
        assert_eq!(ast.node_count(), 1);
        assert_eq!(ast.nodes.get("pkt_0").unwrap().label, "0-7");
    }
}
