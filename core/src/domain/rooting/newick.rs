/// A rooted tree ready to be written as Newick.
///
/// For a leaf, `label` is its name. For an internal node, it is the support
/// of the edge above the node (or the node's name when there is no support).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RootedNode {
    pub label: Option<String>,
    pub length: Option<f64>,
    pub children: Vec<RootedNode>,
}

impl RootedNode {
    /// Sorts every child list by the smallest leaf name below each child, so
    /// that the output does not depend on the input order.
    pub fn sort(&mut self) -> String {
        if self.children.is_empty() {
            return self.label.clone().unwrap_or_default();
        }

        let mut keyed = std::mem::take(&mut self.children)
            .into_iter()
            .map(|mut child| (child.sort(), child))
            .collect::<Vec<_>>();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));

        let min = keyed[0].0.clone();
        self.children = keyed.into_iter().map(|(_, child)| child).collect();
        min
    }
}

/// Formats a number with the shortest representation that parses back to
/// the same value; integral values are written without a fraction.
pub(crate) fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }

    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }

    format!("{value}")
}

/// Quotes a label when it contains characters with a meaning in Newick.
pub(crate) fn quote_label(label: &str) -> String {
    let needs_quotes = label
        .chars()
        .any(|c| matches!(c, '(' | ')' | '[' | ']' | '\'' | ':' | ';' | ',' | ' ' | '\t'));

    if needs_quotes {
        format!("'{}'", label.replace('\'', "''"))
    } else {
        label.to_string()
    }
}

fn write_node(node: &RootedNode, out: &mut String) {
    if !node.children.is_empty() {
        out.push('(');
        for (index, child) in node.children.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            write_node(child, out);
        }
        out.push(')');
    }

    if let Some(label) = &node.label {
        out.push_str(&quote_label(label));
    }

    if let Some(length) = node.length {
        out.push(':');
        out.push_str(&format_number(length));
    }
}

/// Writes the tree as a single Newick line terminated by `;`.
pub(crate) fn write_newick(root: &RootedNode) -> String {
    let mut out = String::new();
    write_node(root, &mut out);
    out.push(';');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(name: &str, length: f64) -> RootedNode {
        RootedNode {
            label: Some(name.to_string()),
            length: Some(length),
            children: vec![],
        }
    }

    fn node(
        label: Option<&str>,
        length: Option<f64>,
        children: Vec<RootedNode>,
    ) -> RootedNode {
        RootedNode {
            label: label.map(str::to_string),
            length,
            children,
        }
    }

    #[test]
    fn numbers_use_the_shortest_round_trip_form() {
        assert_eq!(format_number(100.0), "100");
        assert_eq!(format_number(-1.0), "-1");
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(0.1), "0.1");
        assert_eq!(format_number(1e-8), "0.00000001");
        assert_eq!(format_number(0.35).parse::<f64>().unwrap(), 0.35);
    }

    #[test]
    fn labels_with_newick_characters_are_quoted() {
        assert_eq!(quote_label("Bacillus_subtilis"), "Bacillus_subtilis");
        assert_eq!(quote_label("B. subtilis"), "'B. subtilis'");
        assert_eq!(quote_label("it's"), "'it''s'");
        assert_eq!(quote_label("a:b"), "'a:b'");
    }

    #[test]
    fn writes_supports_as_internal_labels_and_omits_root_length() {
        let tree = node(
            None,
            None,
            vec![
                node(Some("90"), Some(0.5), vec![leaf("A", 0.1), leaf("B", 0.2)]),
                leaf("C", 0.3),
            ],
        );

        assert_eq!(write_newick(&tree), "((A:0.1,B:0.2)90:0.5,C:0.3);");
    }

    #[test]
    fn sorting_makes_the_output_independent_of_input_order() {
        let mut one = node(
            None,
            None,
            vec![
                leaf("D", 1.0),
                node(None, Some(1.0), vec![leaf("C", 1.0), leaf("A", 1.0)]),
            ],
        );
        let mut two = node(
            None,
            None,
            vec![
                node(None, Some(1.0), vec![leaf("A", 1.0), leaf("C", 1.0)]),
                leaf("D", 1.0),
            ],
        );

        one.sort();
        two.sort();

        assert_eq!(write_newick(&one), write_newick(&two));
        assert_eq!(write_newick(&one), "((A:1,C:1):1,D:1);");
    }
}
