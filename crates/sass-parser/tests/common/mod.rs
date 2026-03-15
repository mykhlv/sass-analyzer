use expect_test::Expect;
use sass_parser::syntax::{SyntaxNode, debug_tree};

/// Parse source, verify lossless round-trip, and compare the debug tree + errors
/// against an expected snapshot.
#[allow(clippy::needless_pass_by_value)]
pub fn check(source: &str, expect: Expect) {
    let (green, errors) = sass_parser::parse_scss(source);
    let tree = SyntaxNode::new_root(green);

    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed"
    );

    let mut buf = debug_tree(&tree);
    if !errors.is_empty() {
        buf.push_str("errors:\n");
        for (msg, range) in &errors {
            use std::fmt::Write;
            let _ = writeln!(buf, "  {range:?}: {msg}");
        }
    }
    expect.assert_eq(&buf);
}
