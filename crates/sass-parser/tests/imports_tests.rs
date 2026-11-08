use expect_test::{Expect, expect};
use sass_parser::imports::collect_imports;
use sass_parser::syntax::{SyntaxNode, debug_tree};

#[allow(clippy::needless_pass_by_value)]
fn check_tree(source: &str, expect: Expect) {
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
    expect.assert_eq(buf.trim_end());
}

#[allow(clippy::needless_pass_by_value)]
fn check_imports(source: &str, expect: Expect) {
    let (green, _) = sass_parser::parse_scss(source);
    let tree = SyntaxNode::new_root(green);
    let imports = collect_imports(&tree);

    let mut buf = String::new();
    for imp in &imports {
        use std::fmt::Write;
        let _ = writeln!(buf, "{:?} {:?} @ {:?}", imp.kind, imp.path, imp.range);
    }
    expect.assert_eq(buf.trim_end());
}

// ── meta.load-css() tree shape ───────────────────────────────────────

#[test]
fn load_css_in_expression() {
    // meta.load-css() in expression context is parsed as NAMESPACE_REF > FUNCTION_CALL
    check_tree(
        r#"$x: meta.load-css("utils");"#,
        expect![[r#"
            SOURCE_FILE@0..27
              VARIABLE_DECL@0..27
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                NAMESPACE_REF@3..26
                  WHITESPACE@3..4 " "
                  IDENT@4..8 "meta"
                  DOT@8..9 "."
                  FUNCTION_CALL@9..26
                    IDENT@9..17 "load-css"
                    ARG_LIST@17..26
                      LPAREN@17..18 "("
                      ARG@18..25
                        STRING_LITERAL@18..25
                          QUOTED_STRING@18..25 "\"utils\""
                      RPAREN@25..26 ")"
                SEMICOLON@26..27 ";""#]],
    );
}

#[test]
fn load_css_with_variable_arg() {
    // Variable argument — no static path to extract
    check_tree(
        "$x: meta.load-css($url);",
        expect![[r#"
            SOURCE_FILE@0..24
              VARIABLE_DECL@0..24
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                NAMESPACE_REF@3..23
                  WHITESPACE@3..4 " "
                  IDENT@4..8 "meta"
                  DOT@8..9 "."
                  FUNCTION_CALL@9..23
                    IDENT@9..17 "load-css"
                    ARG_LIST@17..23
                      LPAREN@17..18 "("
                      ARG@18..22
                        VARIABLE_REF@18..22
                          DOLLAR@18..19 "$"
                          IDENT@19..22 "url"
                      RPAREN@22..23 ")"
                SEMICOLON@23..24 ";""#]],
    );
}

#[test]
fn load_css_with_with_map() {
    check_tree(
        r#"$x: meta.load-css("theme", $with: ("color": red));"#,
        expect![[r#"
            SOURCE_FILE@0..50
              VARIABLE_DECL@0..50
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                NAMESPACE_REF@3..49
                  WHITESPACE@3..4 " "
                  IDENT@4..8 "meta"
                  DOT@8..9 "."
                  FUNCTION_CALL@9..49
                    IDENT@9..17 "load-css"
                    ARG_LIST@17..49
                      LPAREN@17..18 "("
                      ARG@18..25
                        STRING_LITERAL@18..25
                          QUOTED_STRING@18..25 "\"theme\""
                      COMMA@25..26 ","
                      ARG@26..48
                        WHITESPACE@26..27 " "
                        DOLLAR@27..28 "$"
                        IDENT@28..32 "with"
                        COLON@32..33 ":"
                        MAP_EXPR@33..48
                          WHITESPACE@33..34 " "
                          LPAREN@34..35 "("
                          MAP_ENTRY@35..47
                            STRING_LITERAL@35..42
                              QUOTED_STRING@35..42 "\"color\""
                            COLON@42..43 ":"
                            VALUE@43..47
                              WHITESPACE@43..44 " "
                              IDENT@44..47 "red"
                          RPAREN@47..48 ")"
                      RPAREN@48..49 ")"
                SEMICOLON@49..50 ";""#]],
    );
}

// ── collect_imports API ──────────────────────────────────────────────

#[test]
fn collect_use_forward_import() {
    check_imports(
        "@use \"sass:meta\";\n@forward \"mixins\";\n@import \"legacy\";\n",
        expect![[r#"
            Use "sass:meta" @ 0..17
            Forward "mixins" @ 17..36
            Import "legacy" @ 36..54"#]],
    );
}

#[test]
fn collect_load_css_in_variable() {
    check_imports(
        "@use \"sass:meta\";\n$x: meta.load-css(\"utils\");\n",
        expect![[r#"
            Use "sass:meta" @ 0..17
            LoadCss "utils" @ 21..44"#]],
    );
}

#[test]
fn collect_load_css_variable_arg_ignored() {
    // Variable argument — not a static import, cannot be resolved
    check_imports(
        "@use \"sass:meta\";\n$x: meta.load-css($url);\n",
        expect![[r#"
            Use "sass:meta" @ 0..17"#]],
    );
}

#[test]
fn collect_non_meta_namespace_ignored() {
    check_imports(r#"$x: foo.load-css("bar");"#, expect![""]);
}

#[test]
fn collect_meta_non_load_css_ignored() {
    check_imports(r#"$x: meta.inspect("hello");"#, expect![""]);
}

#[test]
fn collect_multiple_imports() {
    check_imports(
        "@import \"a\", \"b\";\n",
        expect![[r#"
            Import "a" @ 0..17
            Import "b" @ 0..17"#]],
    );
}

// ── URL ranges and plain-CSS imports ─────────────────────────────────

fn import_urls(green: sass_parser::syntax::GreenNode) -> String {
    let tree = SyntaxNode::new_root(green);
    let mut buf = String::new();
    for imp in collect_imports(&tree) {
        use std::fmt::Write;
        let url = &tree.text().to_string()[imp.path_range];
        let css = if imp.plain_css { " plain-css" } else { "" };
        let _ = writeln!(buf, "{:?} {url} @ {:?}{css}", imp.kind, imp.path_range);
    }
    buf.trim_end().to_owned()
}

#[allow(clippy::needless_pass_by_value)]
fn check_import_urls(source: &str, expect: Expect) {
    let (green, _) = sass_parser::parse_scss(source);
    assert_eq!(
        SyntaxNode::new_root(green.clone()).text().to_string(),
        source
    );
    expect.assert_eq(&import_urls(green));
}

#[test]
fn import_url_ranges_cover_the_string() {
    check_import_urls(
        "@use \"a\" as x;\n@forward 'b' show $c;\n@import \"d\", \"e\";\n",
        expect![[r#"
            Use "a" @ 5..8
            Forward 'b' @ 24..27
            Import "d" @ 45..48
            Import "e" @ 50..53"#]],
    );
}

#[test]
fn plain_css_imports() {
    check_import_urls(
        concat!(
            "@import \"a.css\";\n",
            "@import \"http://x/a\";\n",
            "@import \"https://x/a\";\n",
            "@import \"//x/a\";\n",
            "@import url(\"u\");\n",
            "@import \"m\" screen;\n",
            "@import \"s\" supports(display: grid);\n",
        ),
        expect![[r#"
            Import "a.css" @ 8..15 plain-css
            Import "http://x/a" @ 25..37 plain-css
            Import "https://x/a" @ 47..60 plain-css
            Import "//x/a" @ 70..77 plain-css
            Import "u" @ 91..94 plain-css
            Import "m" @ 105..108 plain-css
            Import "s" @ 125..128 plain-css"#]],
    );
}

#[test]
fn plain_css_import_after_a_sass_import_in_one_rule() {
    // Media queries belong to the URL they follow, as in Dart Sass.
    check_import_urls(
        "@import \"a\", \"b\" screen;\n",
        expect![[r#"
            Import "a" @ 8..11
            Import "b" @ 13..16 plain-css"#]],
    );
}

#[test]
fn use_of_a_css_url_is_not_a_plain_css_import() {
    check_import_urls(
        "@use \"a.css\";\n",
        expect![[r#"
            Use "a.css" @ 5..12"#]],
    );
}

#[test]
fn plain_css_import_in_indented_sass() {
    let source = "@import \"m\" screen\n@import \"n\"\n";
    let (green, _) = sass_parser::parse_sass(source);
    assert_eq!(
        SyntaxNode::new_root(green.clone()).text().to_string(),
        source
    );
    expect![[r#"
        Import "m" @ 8..11 plain-css
        Import "n" @ 27..30"#]]
    .assert_eq(&import_urls(green));
}

#[test]
fn use_without_url_has_no_import() {
    check_import_urls("@use ;\n", expect![""]);
}
