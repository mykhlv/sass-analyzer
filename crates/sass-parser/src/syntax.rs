use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SassLanguage {}

impl rowan::Language for SassLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::from(raw.0)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<SassLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<SassLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<SassLanguage>;

/// Format a CST rooted at `node` as an indented debug string.
///
/// Each node prints `KIND@RANGE` and each token prints `KIND@RANGE "text"`.
pub fn debug_tree(node: &SyntaxNode) -> String {
    let mut buf = String::new();
    debug_tree_rec(node, &mut buf, 0);
    buf
}

fn debug_tree_rec(node: &SyntaxNode, buf: &mut String, indent: usize) {
    use std::fmt::Write;
    let kind = node.kind();
    let range = node.text_range();
    let _ = writeln!(buf, "{:indent$}{kind:?}@{range:?}", "");
    for child in node.children_with_tokens() {
        match child {
            rowan::NodeOrToken::Node(n) => debug_tree_rec(&n, buf, indent + 2),
            rowan::NodeOrToken::Token(t) => {
                let kind = t.kind();
                let range = t.text_range();
                let text = t.text();
                let _ = writeln!(
                    buf,
                    "{:indent$}{kind:?}@{range:?} {text:?}",
                    "",
                    indent = indent + 2
                );
            }
        }
    }
}
