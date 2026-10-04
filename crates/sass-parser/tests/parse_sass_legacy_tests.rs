#[path = "common/layout_allow_list.rs"]
mod layout_allow_list;
#[path = "common/layout_differential.rs"]
mod layout_differential;
#[path = "common/line_facts.rs"]
mod line_facts;

/// `parse_sass` gives legacy's tree and errors, unless `source` is on the allow-list.
#[track_caller]
fn check_same(source: &str) {
    let mismatches = line_facts::line_fact_mismatches(source);
    assert!(
        mismatches.is_empty(),
        "layout facts disagree with the legacy lexer on {source:?}: {mismatches:#?}",
    );
    if let Some((legacy, layout)) = layout_differential::layout_mismatch(source) {
        assert!(
            layout_allow_list::allowed(source).is_some(),
            "parse_sass disagrees with parse_sass_legacy on {source:?}:\n{}",
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
    let source = ".a\n  b: (1 +)\n.c\n  d: e\n";
    assert!(
        !sass_parser::parse_sass_legacy(source).1.is_empty(),
        "the input must produce errors for the comparison to cover them",
    );
    assert!(
        layout_differential::layout_mismatch(source).is_none(),
        "parse_sass and parse_sass_legacy must agree here, errors included",
    );
    check_same(source);
}

/// After an unclosed bracket legacy stops reading line breaks for the rest of the file;
/// `parse_sass` recovers at the next line no deeper than the bracket's, so `.c` keeps
/// its declaration.
#[test]
fn parse_sass_recovers_from_an_unclosed_bracket_where_legacy_does_not() {
    let source = ".a\n  b: fn(1\n.c\n  d: e\n";
    assert!(
        layout_differential::layout_mismatch(source).is_some(),
        "legacy now recovers here too; move this input off the allow-list",
    );
    check_same(source);
}
