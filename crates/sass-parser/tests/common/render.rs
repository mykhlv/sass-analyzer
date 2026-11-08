//! A parser's tree and errors on one input, rendered as one string.
//!
//! Included with `#[path]` by the test binaries that run it, rather than through
//! `common/mod.rs`, so that binaries which do not are not left with dead code.

use std::fmt::Write;

use sass_parser::syntax::{GreenNode, SyntaxNode, debug_tree};
use sass_parser::text_range::TextRange;

pub type ParseFn = fn(&str) -> (GreenNode, Vec<(String, TextRange)>);

/// The tree and errors of `parse` on `source`, rendered together so one comparison
/// covers both.
pub fn render(parse: ParseFn, source: &str) -> String {
    let (green, errors) = parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed"
    );
    let mut buf = debug_tree(&tree);
    for (msg, range) in &errors {
        let _ = writeln!(buf, "error {range:?}: {msg}");
    }
    buf
}
