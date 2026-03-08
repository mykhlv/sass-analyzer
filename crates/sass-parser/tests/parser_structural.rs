use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::*;

fn parse(
    source: &str,
) -> (
    SyntaxNode,
    Vec<(String, sass_parser::text_range::TextRange)>,
) {
    let (green, errors) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    (tree, errors)
}

// ═══════════════════════════════════════════════════════════════════════
// 2.17: Structural assertion tests
// Non-snapshot tests that survive Phase 3 tree shape changes.
// Guard Phase 2 invariants across phases.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn rule_set_contains_selector_list_and_block() {
    let (tree, errors) = parse("div { color: red; }");
    assert!(errors.is_empty());

    let rule_set = tree
        .children()
        .find(|n| n.kind() == RULE_SET)
        .expect("SOURCE_FILE should contain RULE_SET");

    assert!(
        rule_set.children().any(|n| n.kind() == SELECTOR_LIST),
        "RULE_SET should contain SELECTOR_LIST"
    );
    assert!(
        rule_set.children().any(|n| n.kind() == BLOCK),
        "RULE_SET should contain BLOCK"
    );
}

#[test]
fn block_contains_declaration() {
    let (tree, errors) = parse("div { color: red; }");
    assert!(errors.is_empty());

    let block = tree
        .descendants()
        .find(|n| n.kind() == BLOCK)
        .expect("should have BLOCK");

    assert!(
        block.children().any(|n| n.kind() == DECLARATION),
        "BLOCK should contain DECLARATION"
    );
}

#[test]
fn declaration_has_property_and_value() {
    let (tree, _) = parse("p { color: red; }");

    let decl = tree
        .descendants()
        .find(|n| n.kind() == DECLARATION)
        .expect("should have DECLARATION");

    assert!(
        decl.children().any(|n| n.kind() == PROPERTY),
        "DECLARATION should contain PROPERTY"
    );
    assert!(
        decl.children().any(|n| n.kind() == VALUE),
        "DECLARATION should contain VALUE"
    );
}

#[test]
fn nested_rule_inside_block() {
    let (tree, _) = parse("nav { a { } }");

    let outer_block = tree
        .descendants()
        .find(|n| n.kind() == BLOCK)
        .expect("should have outer BLOCK");

    assert!(
        outer_block.children().any(|n| n.kind() == RULE_SET),
        "outer BLOCK should contain nested RULE_SET"
    );
}

#[test]
fn selector_list_contains_selectors() {
    let (tree, _) = parse("h1, h2 { }");

    let sel_list = tree
        .descendants()
        .find(|n| n.kind() == SELECTOR_LIST)
        .expect("should have SELECTOR_LIST");

    let selector_count = sel_list.children().filter(|n| n.kind() == SELECTOR).count();
    assert_eq!(
        selector_count, 2,
        "selector list h1, h2 should have 2 selectors"
    );
}

#[test]
fn compound_selector_has_multiple_simple_selectors() {
    let (tree, _) = parse("div.class { }");

    let selector = tree
        .descendants()
        .find(|n| n.kind() == SELECTOR)
        .expect("should have SELECTOR");

    let simple_count = selector
        .children()
        .filter(|n| n.kind() == SIMPLE_SELECTOR)
        .count();
    assert!(
        simple_count >= 2,
        "div.class should have >= 2 SIMPLE_SELECTOR children"
    );
}

#[test]
fn pseudo_selector_node_exists() {
    let (tree, _) = parse("a:hover { }");

    assert!(
        tree.descendants().any(|n| n.kind() == PSEUDO_SELECTOR),
        "a:hover should produce a PSEUDO_SELECTOR"
    );
}

#[test]
fn attr_selector_node_exists() {
    let (tree, _) = parse("[disabled] { }");

    assert!(
        tree.descendants().any(|n| n.kind() == ATTR_SELECTOR),
        "[disabled] should produce ATTR_SELECTOR"
    );
}

#[test]
fn custom_property_decl_node() {
    let (tree, _) = parse(":root { --color: red; }");

    assert!(
        tree.descendants().any(|n| n.kind() == CUSTOM_PROPERTY_DECL),
        "--color declaration should produce CUSTOM_PROPERTY_DECL"
    );
}

#[test]
fn nested_property_node() {
    let (tree, _) = parse("p { font: { weight: bold; } }");

    assert!(
        tree.descendants().any(|n| n.kind() == NESTED_PROPERTY),
        "font: {{}} should produce NESTED_PROPERTY"
    );
}

#[test]
fn interpolation_node_in_selector() {
    let (tree, _) = parse("#{$tag} { }");

    assert!(
        tree.descendants().any(|n| n.kind() == INTERPOLATION),
        "#{{$tag}} should produce INTERPOLATION"
    );
}

#[test]
fn combinator_node_exists() {
    let (tree, _) = parse("ul > li { }");

    assert!(
        tree.descendants().any(|n| n.kind() == COMBINATOR),
        "> should produce COMBINATOR"
    );
}

#[test]
fn source_file_is_root() {
    let (tree, _) = parse("div { }");
    assert_eq!(tree.kind(), SOURCE_FILE);
}

#[test]
fn multiple_rules_are_children_of_source_file() {
    let (tree, _) = parse("h1 { } h2 { }");

    let rule_count = tree.children().filter(|n| n.kind() == RULE_SET).count();
    assert_eq!(
        rule_count, 2,
        "two rules should be direct children of SOURCE_FILE"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 2.18: Round-trip tests
// Every test verifies: tree.text() == original input (lossless)
// ═══════════════════════════════════════════════════════════════════════

fn assert_round_trip(source: &str) {
    let (tree, _) = parse(source);
    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed for: {source:?}"
    );
}

#[test]
fn round_trip_simple_rule() {
    assert_round_trip("div { color: red; }");
}

#[test]
fn round_trip_nested_rules() {
    assert_round_trip("nav { ul { margin: 0; } }");
}

#[test]
fn round_trip_selector_list() {
    assert_round_trip("h1, h2, h3 { }");
}

#[test]
fn round_trip_compound_selector() {
    assert_round_trip("div.class#id[attr]:hover::before { }");
}

#[test]
fn round_trip_combinators() {
    assert_round_trip("ul > li + p ~ span { }");
}

#[test]
fn round_trip_parent_selector() {
    assert_round_trip(".btn { &:hover { } &-primary { } }");
}

#[test]
fn round_trip_placeholder() {
    assert_round_trip("%placeholder { color: red; }");
}

#[test]
fn round_trip_important() {
    assert_round_trip("p { color: red !important; }");
}

#[test]
fn round_trip_custom_property() {
    assert_round_trip(":root { --color: red; }");
}

#[test]
fn round_trip_nested_property() {
    assert_round_trip("p { font: { weight: bold; } }");
}

#[test]
fn round_trip_interpolation() {
    assert_round_trip("#{$tag} { #{$prop}: #{$val}; }");
}

#[test]
fn round_trip_comments() {
    assert_round_trip("/* heading */ h1 { // inline\n  color: red; }");
}

#[test]
fn round_trip_empty() {
    assert_round_trip("");
}

#[test]
fn round_trip_whitespace_only() {
    assert_round_trip("   \n\t  ");
}

#[test]
fn round_trip_error_input() {
    // Even broken input must round-trip
    assert_round_trip("div { color: red");
    assert_round_trip("@@@ h1 { }");
    assert_round_trip("} div { }");
    assert_round_trip(",, { }");
}

#[test]
fn round_trip_bom() {
    assert_round_trip("\u{FEFF}div { }");
}

#[test]
fn round_trip_url() {
    assert_round_trip("p { background: url(img.png); }");
}

#[test]
fn round_trip_function_call() {
    assert_round_trip("p { color: rgba(255, 0, 0, 0.5); }");
}

// ═══════════════════════════════════════════════════════════════════════
// 2.19: Boundary tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn boundary_empty_file() {
    let (tree, errors) = parse("");
    assert_eq!(tree.kind(), SOURCE_FILE);
    assert!(errors.is_empty());
    assert_eq!(tree.text().to_string(), "");
}

#[test]
fn boundary_whitespace_only() {
    let (tree, _) = parse("   \n\t  ");
    assert_eq!(tree.kind(), SOURCE_FILE);
    assert_eq!(tree.text().to_string(), "   \n\t  ");
}

#[test]
fn boundary_deeply_nested_at_limit() {
    // Each nesting level uses 2 depth guards (rule_set + block), so
    // MAX_DEPTH(256) / 2 = 128 nesting levels should succeed.
    let mut input = String::new();
    for _ in 0..128 {
        input.push_str("a { ");
    }
    for _ in 0..128 {
        input.push_str("} ");
    }
    let (tree, errors) = parse(&input);
    assert_eq!(tree.text().to_string(), input, "round-trip must hold");
    assert!(
        !errors.iter().any(|e| e.0.contains("nesting too deep")),
        "128 nesting levels should not trigger depth error"
    );
}

#[test]
fn boundary_deeply_nested_over_limit() {
    // 129 nesting levels (258 depth guards) exceeds MAX_DEPTH(256),
    // should trigger "nesting too deep" error, not stack overflow.
    let mut input = String::new();
    for _ in 0..129 {
        input.push_str("a { ");
    }
    for _ in 0..129 {
        input.push_str("} ");
    }
    let (tree, errors) = parse(&input);
    assert_eq!(tree.text().to_string(), input, "round-trip must hold");
    assert!(
        errors.iter().any(|e| e.0.contains("nesting too deep")),
        "should report nesting too deep"
    );
}

#[test]
fn boundary_large_generated_file() {
    // 1MB file should complete without panic
    let mut input = String::new();
    for i in 0..10_000 {
        input.push_str(&format!(".class{i} {{ color: red; }}\n"));
    }
    let start = std::time::Instant::now();
    let (tree, _) = parse(&input);
    let elapsed = start.elapsed();

    assert_eq!(tree.text().to_string(), input, "round-trip must hold");
    assert!(
        elapsed.as_secs() < 10,
        "parsing ~{} bytes took {:?}, should be < 10s",
        input.len(),
        elapsed
    );
}

#[test]
fn boundary_long_single_value() {
    // Very long value (100KB) — no panic
    let value = "x".repeat(100_000);
    let input = format!("p {{ prop: {value}; }}");
    let (tree, _) = parse(&input);
    assert_eq!(tree.text().to_string(), input, "round-trip must hold");
}

#[test]
fn boundary_no_newlines() {
    let input = "a{color:red}b{font-size:14px}c{margin:0}";
    let (tree, _) = parse(input);
    assert_eq!(tree.text().to_string(), input, "round-trip must hold");
}

#[test]
fn boundary_bom_alone() {
    let (tree, _) = parse("\u{FEFF}");
    assert_eq!(tree.text().to_string(), "\u{FEFF}");
}

// ═══════════════════════════════════════════════════════════════════════
// 2.20: Real-world CSS parsing
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn parse_normalize_css() {
    let source = include_str!("fixtures/normalize.css");
    let (tree, errors) = parse(source);

    assert_eq!(tree.kind(), SOURCE_FILE);
    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed for normalize.css"
    );
    assert!(
        errors.is_empty(),
        "normalize.css should parse without errors, got: {errors:?}"
    );
    // Should contain rule sets
    let rule_count = tree.children().filter(|n| n.kind() == RULE_SET).count();
    assert!(
        rule_count > 10,
        "normalize.css should have many rules, got {rule_count}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Golden file: comprehensive SCSS with all parser features
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn golden_scss_round_trip() {
    let source = include_str!("fixtures/golden.scss");
    let (tree, errors) = parse(source);

    assert_eq!(tree.kind(), SOURCE_FILE);
    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed for golden.scss"
    );
    assert!(
        errors.is_empty(),
        "golden.scss should parse without errors, got: {errors:?}"
    );
}

#[test]
fn golden_scss_has_expected_nodes() {
    let source = include_str!("fixtures/golden.scss");
    let (tree, _errors) = parse(source);

    fn count_kind(node: &SyntaxNode, kind: SyntaxKind) -> usize {
        let mut n = if node.kind() == kind { 1 } else { 0 };
        for child in node.children() {
            n += count_kind(&child, kind);
        }
        n
    }

    // Variables
    assert!(
        count_kind(&tree, VARIABLE_DECL) >= 10,
        "expected >= 10 variable declarations"
    );
    // Mixins
    assert!(
        count_kind(&tree, MIXIN_RULE) >= 4,
        "expected >= 4 @mixin rules"
    );
    // Functions
    assert!(
        count_kind(&tree, FUNCTION_RULE) >= 3,
        "expected >= 3 @function rules"
    );
    // Includes
    assert!(
        count_kind(&tree, INCLUDE_RULE) >= 3,
        "expected >= 3 @include rules"
    );
    // @if
    assert!(count_kind(&tree, IF_RULE) >= 2, "expected >= 2 @if rules");
    // @each
    assert!(
        count_kind(&tree, EACH_RULE) >= 1,
        "expected >= 1 @each rule"
    );
    // @for
    assert!(count_kind(&tree, FOR_RULE) >= 1, "expected >= 1 @for rule");
    // @while
    assert!(
        count_kind(&tree, WHILE_RULE) >= 1,
        "expected >= 1 @while rule"
    );
    // Keyframes
    assert!(
        count_kind(&tree, KEYFRAMES_RULE) >= 2,
        "expected >= 2 @keyframes rules"
    );
    // Media
    assert!(
        count_kind(&tree, MEDIA_RULE) >= 3,
        "expected >= 3 @media rules"
    );
    // @extend
    assert!(
        count_kind(&tree, EXTEND_RULE) >= 2,
        "expected >= 2 @extend rules"
    );
    // Interpolations
    assert!(
        count_kind(&tree, INTERPOLATION) >= 5,
        "expected >= 5 interpolations"
    );
    // Custom property declarations
    assert!(
        count_kind(&tree, CUSTOM_PROPERTY_DECL) >= 5,
        "expected >= 5 custom property declarations"
    );
    // Nested properties
    assert!(
        count_kind(&tree, NESTED_PROPERTY) >= 2,
        "expected >= 2 nested properties"
    );
    // @use
    assert!(count_kind(&tree, USE_RULE) >= 3, "expected >= 3 @use rules");
    // @forward
    assert!(
        count_kind(&tree, FORWARD_RULE) >= 2,
        "expected >= 2 @forward rules"
    );
    // Calculations
    assert!(
        count_kind(&tree, CALCULATION) >= 2,
        "expected >= 2 calculation expressions"
    );
}
