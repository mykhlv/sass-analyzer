use std::fmt::Write;

use sass_parser::syntax::{GreenNode, SyntaxNode, debug_tree};
use sass_parser::text_range::TextRange;

#[path = "common/layout_allow_list.rs"]
mod layout_allow_list;
#[path = "common/layout_differential.rs"]
mod layout_differential;
#[path = "common/line_facts.rs"]
mod line_facts;

type ParseFn = fn(&str) -> (GreenNode, Vec<(String, TextRange)>);

/// The tree and errors rendered together, so one comparison covers both.
fn render(parse: ParseFn, source: &str) -> String {
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

#[track_caller]
fn check_same(source: &str) {
    assert_eq!(
        render(sass_parser::parse_sass_legacy, source),
        render(sass_parser::parse_sass, source),
        "parse_sass_legacy and parse_sass disagree on {source:?}",
    );
    let mismatches = line_facts::line_fact_mismatches(source);
    assert!(
        mismatches.is_empty(),
        "layout facts disagree with the legacy lexer on {source:?}: {mismatches:#?}",
    );
    if let Some((legacy, layout)) = layout_differential::layout_mismatch(source) {
        assert!(
            layout_allow_list::allowed(source).is_some(),
            "parse_sass_layout disagrees with parse_sass_legacy on {source:?}:\n{}",
            layout_differential::first_difference(&legacy, &layout),
        );
    }
}

#[test]
fn legacy_matches_parse_sass_on_nested_rules() {
    check_same(".nav\n  color: red\n  .item\n    margin: 0\n.footer\n  padding: 0\n");
}

#[test]
fn legacy_matches_parse_sass_on_continued_headers_and_comments() {
    check_same("// c\n@mixin m($a,\n  $b)\n  // body comment\n  width: $a\n@include m(1,\n  2)\n");
}

#[test]
fn legacy_matches_parse_sass_on_empty_input() {
    check_same("");
}

#[test]
fn legacy_matches_parse_sass_on_invalid_input() {
    let source = ".a\n  b: fn(1\n.c\n  d: e\n";
    assert!(
        !sass_parser::parse_sass_legacy(source).1.is_empty(),
        "the input must produce errors for the comparison to cover them",
    );
    check_same(source);
}
