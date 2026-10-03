//! Differential check of `parse_sass_layout` against `parse_sass_legacy`.
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

/// Both renderings when the layout pipeline and the legacy one disagree on `source`.
pub fn layout_mismatch(source: &str) -> Option<(String, String)> {
    let legacy = render(sass_parser::parse_sass_legacy, source);
    let layout = render(sass_parser::parse_sass_layout, source);
    (legacy != layout).then_some((legacy, layout))
}

/// A short report of the first lines where two renderings part.
pub fn first_difference(legacy: &str, layout: &str) -> String {
    let a: Vec<&str> = legacy.lines().collect();
    let b: Vec<&str> = layout.lines().collect();
    let at = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let from = at.saturating_sub(3);
    let mut out = String::new();
    let _ = writeln!(out, "--- legacy (line {at})");
    for line in a.iter().skip(from).take(10) {
        let _ = writeln!(out, "  {line}");
    }
    let _ = writeln!(out, "--- layout");
    for line in b.iter().skip(from).take(10) {
        let _ = writeln!(out, "  {line}");
    }
    out
}
