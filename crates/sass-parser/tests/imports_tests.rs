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
