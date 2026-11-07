/// An explicit rooting annotation, as written by tools such as BEAST or
/// FigTree: `[&R]` (rooted) or `[&U]` (unrooted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootingFlag {
    Rooted,
    Unrooted,
}

fn parse_flag(comment: &str) -> Option<RootingFlag> {
    match comment.trim().to_ascii_uppercase().as_str() {
        "&R" => Some(RootingFlag::Rooted),
        "&U" => Some(RootingFlag::Unrooted),
        _ => None,
    }
}

/// Detects and removes a rooting annotation placed before the tree (the usual
/// position) or right before the final `;`.
///
/// The Newick parser attaches a leading annotation to the first leaf instead
/// of the root, so it must be removed from the text before parsing.
pub fn split_rooting_annotation(newick: &str) -> (Option<RootingFlag>, String) {
    let text = newick.trim();

    if let Some(rest) = text.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            if let Some(flag) = parse_flag(&rest[..end]) {
                return (Some(flag), rest[end + 1..].trim_start().to_string());
            }
        }
    }

    let (body, semicolon) = match text.strip_suffix(';') {
        Some(body) => (body.trim_end(), ";"),
        None => (text, ""),
    };

    if let Some(without_bracket) = body.strip_suffix(']') {
        if let Some(start) = without_bracket.rfind('[') {
            if let Some(flag) = parse_flag(&without_bracket[start + 1..]) {
                return (
                    Some(flag),
                    format!("{}{semicolon}", without_bracket[..start].trim_end()),
                );
            }
        }
    }

    (None, text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_unrooted_annotation_is_removed() {
        assert_eq!(
            split_rooting_annotation("[&U] (A:1,B:1,C:1);\n"),
            (Some(RootingFlag::Unrooted), "(A:1,B:1,C:1);".to_string())
        );
    }

    #[test]
    fn leading_rooted_annotation_is_case_insensitive() {
        assert_eq!(
            split_rooting_annotation("[&r]((A:1,B:1):1,C:1);"),
            (Some(RootingFlag::Rooted), "((A:1,B:1):1,C:1);".to_string())
        );
    }

    #[test]
    fn trailing_annotation_is_removed() {
        assert_eq!(
            split_rooting_annotation("((A:1,B:1):1,C:1)[&R];"),
            (Some(RootingFlag::Rooted), "((A:1,B:1):1,C:1);".to_string())
        );
    }

    #[test]
    fn text_without_annotation_is_unchanged() {
        assert_eq!(
            split_rooting_annotation("  ((A,B),C);  "),
            (None, "((A,B),C);".to_string())
        );
    }

    #[test]
    fn other_comments_are_kept() {
        assert_eq!(
            split_rooting_annotation("[a comment]((A,B),C)[x];"),
            (None, "[a comment]((A,B),C)[x];".to_string())
        );
    }
}
