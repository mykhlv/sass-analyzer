//! The inputs on which `parse_sass` may differ from `parse_sass_legacy`, each a
//! known defect of the legacy indented frontend, with the tree the layout frontend gives
//! instead. Any other difference is a regression of the layout frontend.
//!
//! An entry lists one defect, by what goes wrong in legacy, and the result checked
//! against Dart Sass 1.105.1. Entries whose defect legacy has since been fixed stay as
//! pins: there both frontends agree, and on the target.
//!
//! Included with `#[path]` by the test binaries that run it, rather than through
//! `common/mod.rs`, so that binaries which do not are not left with dead code.

use expect_test::{Expect, expect};

// Each test binary reads only the fields it reports on.
#[allow(dead_code)]
pub struct Allowed {
    pub input: &'static str,
    /// What legacy does wrong here, and what the layout frontend does instead.
    pub change: &'static str,
    /// `parse_sass`'s tree and errors.
    pub expected: Expect,
}

/// The entry for `source`, if it is on the list.
pub fn allowed(source: &str) -> Option<Allowed> {
    allow_list().into_iter().find(|entry| entry.input == source)
}

#[allow(clippy::too_many_lines)]
pub fn allow_list() -> Vec<Allowed> {
    vec![
        Allowed {
            input: ".a:hover > *\n  color: red\n",
            change: "a trailing universal selector after a pseudo-class used to read as \
                     multiplication and take the declaration below into the selector; both \
                     frontends now give the rule its declaration (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..26
                  RULE_SET@0..25
                    SELECTOR_LIST@0..12
                      SELECTOR@0..12
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                        PSEUDO_SELECTOR@2..8
                          COLON@2..3 ":"
                          IDENT@3..8 "hover"
                        COMBINATOR@8..10
                          WHITESPACE@8..9 " "
                          GT@9..10 ">"
                        SIMPLE_SELECTOR@10..12
                          WHITESPACE@10..11 " "
                          STAR@11..12 "*"
                    BLOCK@12..25
                      LBRACE@12..12 ""
                      DECLARATION@12..25
                        PROPERTY@12..20
                          WHITESPACE@12..15 "\n  "
                          IDENT@15..20 "color"
                        COLON@20..21 ":"
                        VALUE@21..25
                          VALUE@21..25
                            WHITESPACE@21..22 " "
                            IDENT@22..25 "red"
                        SEMICOLON@25..25 ""
                      RBRACE@25..25 ""
                  WHITESPACE@25..26 "\n"
            "#]],
        },
        Allowed {
            input: ".a:focus,\n.b *\n  c: d\n",
            change: "a pseudo-class on an earlier line of the selector list used to turn \
                     a trailing `*` into an operator; both frontends now give the rule its \
                     declaration (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..22
                  RULE_SET@0..21
                    SELECTOR_LIST@0..14
                      SELECTOR@0..8
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                        PSEUDO_SELECTOR@2..8
                          COLON@2..3 ":"
                          IDENT@3..8 "focus"
                      COMMA@8..9 ","
                      SELECTOR@9..14
                        SIMPLE_SELECTOR@9..12
                          WHITESPACE@9..10 "\n"
                          DOT@10..11 "."
                          IDENT@11..12 "b"
                        SIMPLE_SELECTOR@12..14
                          WHITESPACE@12..13 " "
                          STAR@13..14 "*"
                    BLOCK@14..21
                      LBRACE@14..14 ""
                      DECLARATION@14..21
                        PROPERTY@14..18
                          WHITESPACE@14..17 "\n  "
                          IDENT@17..18 "c"
                        COLON@18..19 ":"
                        VALUE@19..21
                          VALUE@19..21
                            WHITESPACE@19..20 " "
                            IDENT@20..21 "d"
                        SEMICOLON@21..21 ""
                      RBRACE@21..21 ""
                  WHITESPACE@21..22 "\n"
            "#]],
        },
        Allowed {
            input: ".a:hover .not\n  b: c\n",
            change: "`not` ending a selector after a pseudo-class used to continue the line \
                     as a boolean operator; both frontends now give the rule its \
                     declaration (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..21
                  RULE_SET@0..20
                    SELECTOR_LIST@0..13
                      SELECTOR@0..13
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                        PSEUDO_SELECTOR@2..8
                          COLON@2..3 ":"
                          IDENT@3..8 "hover"
                        SIMPLE_SELECTOR@8..13
                          WHITESPACE@8..9 " "
                          DOT@9..10 "."
                          IDENT@10..13 "not"
                    BLOCK@13..20
                      LBRACE@13..13 ""
                      DECLARATION@13..20
                        PROPERTY@13..17
                          WHITESPACE@13..16 "\n  "
                          IDENT@16..17 "b"
                        COLON@17..18 ":"
                        VALUE@18..20
                          VALUE@18..20
                            WHITESPACE@18..19 " "
                            IDENT@19..20 "c"
                        SEMICOLON@20..20 ""
                      RBRACE@20..20 ""
                  WHITESPACE@20..21 "\n"
            "#]],
        },
        Allowed {
            input: ".a\n  content: not\n  b: c\n",
            change: "legacy continues the line after a value ending in `not` and silently \
                     takes `b: c` into it; the layout frontend goes on after `not` only \
                     into a line that could be its operand, so `b: c` is a declaration of \
                     its own (Dart reports `expected newline.`; reporting it here needs \
                     `not` to be an operator in CSS values, which would change SCSS)",
            expected: expect![[r#"
                SOURCE_FILE@0..25
                  RULE_SET@0..24
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..24
                      LBRACE@2..2 ""
                      DECLARATION@2..17
                        PROPERTY@2..12
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..12 "content"
                        COLON@12..13 ":"
                        VALUE@13..17
                          VALUE@13..17
                            WHITESPACE@13..14 " "
                            IDENT@14..17 "not"
                        SEMICOLON@17..17 ""
                      DECLARATION@17..24
                        PROPERTY@17..21
                          WHITESPACE@17..20 "\n  "
                          IDENT@20..21 "b"
                        COLON@21..22 ":"
                        VALUE@22..24
                          VALUE@22..24
                            WHITESPACE@22..23 " "
                            IDENT@23..24 "c"
                        SEMICOLON@24..24 ""
                      RBRACE@24..24 ""
                  WHITESPACE@24..25 "\n"
            "#]],
        },
        Allowed {
            input: ".a\n  b: c and\n    d: e\n",
            change: "legacy continues the line after a value ending in `and` and silently \
                     takes `d: e` into it; the layout frontend goes on after `and` or `or` \
                     only into a line that could be its operand, so the deeper `d: e` is a \
                     nested declaration, as without the `and` (Dart reports `expected \
                     newline.`, as for `not`)",
            expected: expect![[r#"
                SOURCE_FILE@0..23
                  RULE_SET@0..22
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..22
                      LBRACE@2..2 ""
                      NESTED_PROPERTY@2..22
                        PROPERTY@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "b"
                        COLON@6..7 ":"
                        VALUE@7..13
                          VALUE@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
                          VALUE@9..13
                            WHITESPACE@9..10 " "
                            IDENT@10..13 "and"
                        BLOCK@13..22
                          LBRACE@13..13 ""
                          DECLARATION@13..22
                            PROPERTY@13..19
                              WHITESPACE@13..18 "\n    "
                              IDENT@18..19 "d"
                            COLON@19..20 ":"
                            VALUE@20..22
                              VALUE@20..22
                                WHITESPACE@20..21 " "
                                IDENT@21..22 "e"
                            SEMICOLON@22..22 ""
                          RBRACE@22..22 ""
                      RBRACE@22..22 ""
                  WHITESPACE@22..23 "\n"
            "#]],
        },
        Allowed {
            input: ".a\n  b: c or\n  d: e\n",
            change: "legacy continues the line after a value ending in `or` and silently \
                     takes `d: e` into it; the layout frontend ends the declaration there, \
                     so `d: e` is a declaration of its own (Dart reports `expected \
                     newline.`, as for `not`)",
            expected: expect![[r#"
                SOURCE_FILE@0..20
                  RULE_SET@0..19
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..19
                      LBRACE@2..2 ""
                      DECLARATION@2..12
                        PROPERTY@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "b"
                        COLON@6..7 ":"
                        VALUE@7..12
                          VALUE@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
                          VALUE@9..12
                            WHITESPACE@9..10 " "
                            IDENT@10..12 "or"
                        SEMICOLON@12..12 ""
                      DECLARATION@12..19
                        PROPERTY@12..16
                          WHITESPACE@12..15 "\n  "
                          IDENT@15..16 "d"
                        COLON@16..17 ":"
                        VALUE@17..19
                          VALUE@17..19
                            WHITESPACE@17..18 " "
                            IDENT@18..19 "e"
                        SEMICOLON@19..19 ""
                      RBRACE@19..19 ""
                  WHITESPACE@19..20 "\n"
            "#]],
        },
        Allowed {
            input: ".a\n  content: and\n  b: c\n",
            change: "legacy continues the line after any value ending in `and` and silently \
                     takes `b: c` into it; with no operand before it `and` is a plain word, \
                     so the layout frontend ends the declaration there (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..25
                  RULE_SET@0..24
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..24
                      LBRACE@2..2 ""
                      DECLARATION@2..17
                        PROPERTY@2..12
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..12 "content"
                        COLON@12..13 ":"
                        VALUE@13..17
                          VALUE@13..17
                            WHITESPACE@13..14 " "
                            IDENT@14..17 "and"
                        SEMICOLON@17..17 ""
                      DECLARATION@17..24
                        PROPERTY@17..21
                          WHITESPACE@17..20 "\n  "
                          IDENT@20..21 "b"
                        COLON@21..22 ":"
                        VALUE@22..24
                          VALUE@22..24
                            WHITESPACE@22..23 " "
                            IDENT@23..24 "c"
                        SEMICOLON@24..24 ""
                      RBRACE@24..24 ""
                  WHITESPACE@24..25 "\n"
            "#]],
        },
        Allowed {
            input: "$to: 3\n.x\n  @for $i from 1 through $to\n    a: $i\n",
            change: "legacy reads the name in `$to` as the word `to`, which leaves an `@for` \
                     header unfinished, and silently takes the line below into the header, \
                     so the loop has no body; the layout frontend counts only a word of its \
                     own, and `a: $i` is the loop's body (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..49
                  VARIABLE_DECL@0..6
                    DOLLAR@0..1 "$"
                    IDENT@1..3 "to"
                    COLON@3..4 ":"
                    NUMBER_LITERAL@4..6
                      WHITESPACE@4..5 " "
                      NUMBER@5..6 "3"
                    SEMICOLON@6..6 ""
                  RULE_SET@6..48
                    SELECTOR_LIST@6..9
                      SELECTOR@6..9
                        SIMPLE_SELECTOR@6..9
                          WHITESPACE@6..7 "\n"
                          DOT@7..8 "."
                          IDENT@8..9 "x"
                    BLOCK@9..48
                      LBRACE@9..9 ""
                      FOR_RULE@9..48
                        WHITESPACE@9..12 "\n  "
                        AT@12..13 "@"
                        IDENT@13..16 "for"
                        WHITESPACE@16..17 " "
                        DOLLAR@17..18 "$"
                        IDENT@18..19 "i"
                        WHITESPACE@19..20 " "
                        IDENT@20..24 "from"
                        NUMBER_LITERAL@24..26
                          WHITESPACE@24..25 " "
                          NUMBER@25..26 "1"
                        WHITESPACE@26..27 " "
                        IDENT@27..34 "through"
                        VARIABLE_REF@34..38
                          WHITESPACE@34..35 " "
                          DOLLAR@35..36 "$"
                          IDENT@36..38 "to"
                        BLOCK@38..48
                          LBRACE@38..38 ""
                          DECLARATION@38..48
                            PROPERTY@38..44
                              WHITESPACE@38..43 "\n    "
                              IDENT@43..44 "a"
                            COLON@44..45 ":"
                            VALUE@45..48
                              VARIABLE_REF@45..48
                                WHITESPACE@45..46 " "
                                DOLLAR@46..47 "$"
                                IDENT@47..48 "i"
                            SEMICOLON@48..48 ""
                          RBRACE@48..48 ""
                      RBRACE@48..48 ""
                  WHITESPACE@48..49 "\n"
            "#]],
        },
        Allowed {
            input: "$in: 1 2\n.x\n  @each $v in $in\n    a: $v\n",
            change: "legacy reads the name in `$in` as the word `in` and takes the line \
                     below into the `@each` header, with four errors; the layout frontend \
                     gives the loop its body (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..40
                  VARIABLE_DECL@0..8
                    DOLLAR@0..1 "$"
                    IDENT@1..3 "in"
                    COLON@3..4 ":"
                    NUMBER_LITERAL@4..6
                      WHITESPACE@4..5 " "
                      NUMBER@5..6 "1"
                    NUMBER_LITERAL@6..8
                      WHITESPACE@6..7 " "
                      NUMBER@7..8 "2"
                    SEMICOLON@8..8 ""
                  RULE_SET@8..39
                    SELECTOR_LIST@8..11
                      SELECTOR@8..11
                        SIMPLE_SELECTOR@8..11
                          WHITESPACE@8..9 "\n"
                          DOT@9..10 "."
                          IDENT@10..11 "x"
                    BLOCK@11..39
                      LBRACE@11..11 ""
                      EACH_RULE@11..39
                        WHITESPACE@11..14 "\n  "
                        AT@14..15 "@"
                        IDENT@15..19 "each"
                        WHITESPACE@19..20 " "
                        DOLLAR@20..21 "$"
                        IDENT@21..22 "v"
                        WHITESPACE@22..23 " "
                        IDENT@23..25 "in"
                        VARIABLE_REF@25..29
                          WHITESPACE@25..26 " "
                          DOLLAR@26..27 "$"
                          IDENT@27..29 "in"
                        BLOCK@29..39
                          LBRACE@29..29 ""
                          DECLARATION@29..39
                            PROPERTY@29..35
                              WHITESPACE@29..34 "\n    "
                              IDENT@34..35 "a"
                            COLON@35..36 ":"
                            VALUE@36..39
                              VARIABLE_REF@36..39
                                WHITESPACE@36..37 " "
                                DOLLAR@37..38 "$"
                                IDENT@38..39 "v"
                            SEMICOLON@39..39 ""
                          RBRACE@39..39 ""
                      RBRACE@39..39 ""
                  WHITESPACE@39..40 "\n"
            "#]],
        },
        Allowed {
            input: "$and: true\n.x\n  @if $and\n    a: b\n",
            change: "legacy reads the name in `$and` as the word `and` and silently takes \
                     the line below into the `@if` condition; the layout frontend gives the \
                     rule its body (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..34
                  VARIABLE_DECL@0..10
                    DOLLAR@0..1 "$"
                    IDENT@1..4 "and"
                    COLON@4..5 ":"
                    BOOL_LITERAL@5..10
                      WHITESPACE@5..6 " "
                      IDENT@6..10 "true"
                    SEMICOLON@10..10 ""
                  RULE_SET@10..33
                    SELECTOR_LIST@10..13
                      SELECTOR@10..13
                        SIMPLE_SELECTOR@10..13
                          WHITESPACE@10..11 "\n"
                          DOT@11..12 "."
                          IDENT@12..13 "x"
                    BLOCK@13..33
                      LBRACE@13..13 ""
                      IF_RULE@13..33
                        WHITESPACE@13..16 "\n  "
                        AT@16..17 "@"
                        IDENT@17..19 "if"
                        VARIABLE_REF@19..24
                          WHITESPACE@19..20 " "
                          DOLLAR@20..21 "$"
                          IDENT@21..24 "and"
                        BLOCK@24..33
                          LBRACE@24..24 ""
                          DECLARATION@24..33
                            PROPERTY@24..30
                              WHITESPACE@24..29 "\n    "
                              IDENT@29..30 "a"
                            COLON@30..31 ":"
                            VALUE@31..33
                              VALUE@31..33
                                WHITESPACE@31..32 " "
                                IDENT@32..33 "b"
                            SEMICOLON@33..33 ""
                          RBRACE@33..33 ""
                      RBRACE@33..33 ""
                  WHITESPACE@33..34 "\n"
            "#]],
        },
        Allowed {
            input: "@function f($if)\n  @return $if\n.a\n  b: f(1)\n",
            change: "legacy reads the name in `$if` as the word `if` and takes the next \
                     rule into the `@return` value, with an error; the layout frontend ends \
                     the function there (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..44
                  FUNCTION_RULE@0..30
                    AT@0..1 "@"
                    IDENT@1..9 "function"
                    WHITESPACE@9..10 " "
                    IDENT@10..11 "f"
                    PARAM_LIST@11..16
                      LPAREN@11..12 "("
                      PARAM@12..15
                        DOLLAR@12..13 "$"
                        IDENT@13..15 "if"
                      RPAREN@15..16 ")"
                    BLOCK@16..30
                      LBRACE@16..16 ""
                      RETURN_RULE@16..30
                        WHITESPACE@16..19 "\n  "
                        AT@19..20 "@"
                        IDENT@20..26 "return"
                        VARIABLE_REF@26..30
                          WHITESPACE@26..27 " "
                          DOLLAR@27..28 "$"
                          IDENT@28..30 "if"
                        SEMICOLON@30..30 ""
                      RBRACE@30..30 ""
                  RULE_SET@30..43
                    SELECTOR_LIST@30..33
                      SELECTOR@30..33
                        SIMPLE_SELECTOR@30..33
                          WHITESPACE@30..31 "\n"
                          DOT@31..32 "."
                          IDENT@32..33 "a"
                    BLOCK@33..43
                      LBRACE@33..33 ""
                      DECLARATION@33..43
                        PROPERTY@33..37
                          WHITESPACE@33..36 "\n  "
                          IDENT@36..37 "b"
                        COLON@37..38 ":"
                        VALUE@38..43
                          FUNCTION_CALL@38..43
                            WHITESPACE@38..39 " "
                            IDENT@39..40 "f"
                            ARG_LIST@40..43
                              LPAREN@40..41 "("
                              ARG@41..42
                                NUMBER_LITERAL@41..42
                                  NUMBER@41..42 "1"
                              RPAREN@42..43 ")"
                        SEMICOLON@43..43 ""
                      RBRACE@43..43 ""
                  WHITESPACE@43..44 "\n"
            "#]],
        },
        Allowed {
            input: ".a\n  b: c\n@x:*\n.d\n  e: f\n",
            change: "legacy goes on after a `*` that ends a line in any statement with a \
                     colon before it, as if it were multiplication, and silently takes the \
                     rule `.d` into the at-rule's prelude; the layout frontend ends the \
                     at-rule at the line break (Dart: same, `@x :*;` and the rule)",
            expected: expect![[r#"
                SOURCE_FILE@0..25
                  RULE_SET@0..9
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..9
                      LBRACE@2..2 ""
                      DECLARATION@2..9
                        PROPERTY@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "b"
                        COLON@6..7 ":"
                        VALUE@7..9
                          VALUE@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
                        SEMICOLON@9..9 ""
                      RBRACE@9..9 ""
                  GENERIC_AT_RULE@9..14
                    WHITESPACE@9..10 "\n"
                    AT@10..11 "@"
                    IDENT@11..12 "x"
                    COLON@12..13 ":"
                    STAR@13..14 "*"
                    SEMICOLON@14..14 ""
                  RULE_SET@14..24
                    SELECTOR_LIST@14..17
                      SELECTOR@14..17
                        SIMPLE_SELECTOR@14..17
                          WHITESPACE@14..15 "\n"
                          DOT@15..16 "."
                          IDENT@16..17 "d"
                    BLOCK@17..24
                      LBRACE@17..17 ""
                      DECLARATION@17..24
                        PROPERTY@17..21
                          WHITESPACE@17..20 "\n  "
                          IDENT@20..21 "e"
                        COLON@21..22 ":"
                        VALUE@22..24
                          VALUE@22..24
                            WHITESPACE@22..23 " "
                            IDENT@23..24 "f"
                        SEMICOLON@24..24 ""
                      RBRACE@24..24 ""
                  WHITESPACE@24..25 "\n"
            "#]],
        },
        Allowed {
            input: "/* loud\n  still comment\n.a\n  b: c\n.d\n  e: f\n",
            change: "a loud comment that starts a statement ends at the first line no \
                     deeper than its own, not at the end of the file; both frontends now \
                     give one comment and two rules (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..44
                  RULE_SET@0..33
                    SELECTOR_LIST@0..26
                      SELECTOR@0..26
                        SIMPLE_SELECTOR@0..26
                          MULTI_LINE_COMMENT@0..23 "/* loud\n  still comment"
                          WHITESPACE@23..24 "\n"
                          DOT@24..25 "."
                          IDENT@25..26 "a"
                    BLOCK@26..33
                      LBRACE@26..26 ""
                      DECLARATION@26..33
                        PROPERTY@26..30
                          WHITESPACE@26..29 "\n  "
                          IDENT@29..30 "b"
                        COLON@30..31 ":"
                        VALUE@31..33
                          VALUE@31..33
                            WHITESPACE@31..32 " "
                            IDENT@32..33 "c"
                        SEMICOLON@33..33 ""
                      RBRACE@33..33 ""
                  RULE_SET@33..43
                    SELECTOR_LIST@33..36
                      SELECTOR@33..36
                        SIMPLE_SELECTOR@33..36
                          WHITESPACE@33..34 "\n"
                          DOT@34..35 "."
                          IDENT@35..36 "d"
                    BLOCK@36..43
                      LBRACE@36..36 ""
                      DECLARATION@36..43
                        PROPERTY@36..40
                          WHITESPACE@36..39 "\n  "
                          IDENT@39..40 "e"
                        COLON@40..41 ":"
                        VALUE@41..43
                          VALUE@41..43
                            WHITESPACE@41..42 " "
                            IDENT@42..43 "f"
                        SEMICOLON@43..43 ""
                      RBRACE@43..43 ""
                  WHITESPACE@43..44 "\n"
            "#]],
        },
        Allowed {
            input: "/* Single-line\n\n/*\n  Multi\n  line\n\n/* Multiline starting on\n   the \
                    first line\n\n/* Preserves\n\n   empty\n\n\n   lines\n\n/* Ignores comment \
                    closer */\n\n/*\n  Handles\n         weird\n   indentation\n       \
                    gracefully\n\n/* Even\n      when\n     it starts\n         on the\n       \
                    first line\n\n/* Allows interpolation: #{1 + 1}\n",
            change: "sass-spec `non_conformant/sass/comment/loud`: eight loud comments, each \
                     ending by layout; both frontends now give all eight (Dart: same)",
            expected: expect![[r#"
                SOURCE_FILE@0..309
                  MULTI_LINE_COMMENT@0..14 "/* Single-line"
                  WHITESPACE@14..16 "\n\n"
                  MULTI_LINE_COMMENT@16..33 "/*\n  Multi\n  line"
                  WHITESPACE@33..35 "\n\n"
                  MULTI_LINE_COMMENT@35..77 "/* Multiline starting on\n   the first line"
                  WHITESPACE@77..79 "\n\n"
                  MULTI_LINE_COMMENT@79..112 "/* Preserves\n\n   empty\n\n\n   lines"
                  WHITESPACE@112..114 "\n\n"
                  MULTI_LINE_COMMENT@114..142 "/* Ignores comment closer */"
                  WHITESPACE@142..144 "\n\n"
                  MULTI_LINE_COMMENT@144..204 "/*\n  Handles\n         weird\n   indentation\n       gracefully"
                  WHITESPACE@204..206 "\n\n"
                  MULTI_LINE_COMMENT@206..273 "/* Even\n      when\n     it starts\n         on the\n       first line"
                  WHITESPACE@273..275 "\n\n"
                  MULTI_LINE_COMMENT@275..308 "/* Allows interpolation: #{1 + 1}"
                  WHITESPACE@308..309 "\n"
            "#]],
        },
        Allowed {
            input: ".a\n\tcolor: red\n        b: c\n",
            change: "a line indented with spaces in a file indented with tabs: the layout \
                     frontend reports it (Dart: `Expected tabs, was spaces.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..28
                  RULE_SET@0..27
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..27
                      LBRACE@2..2 ""
                      NESTED_PROPERTY@2..27
                        PROPERTY@2..9
                          WHITESPACE@2..4 "\n\t"
                          IDENT@4..9 "color"
                        COLON@9..10 ":"
                        VALUE@10..14
                          VALUE@10..14
                            WHITESPACE@10..11 " "
                            IDENT@11..14 "red"
                        BLOCK@14..27
                          LBRACE@14..14 ""
                          DECLARATION@14..27
                            PROPERTY@14..24
                              WHITESPACE@14..23 "\n        "
                              IDENT@23..24 "b"
                            COLON@24..25 ":"
                            VALUE@25..27
                              VALUE@25..27
                                WHITESPACE@25..26 " "
                                IDENT@26..27 "c"
                            SEMICOLON@27..27 ""
                          RBRACE@27..27 ""
                      RBRACE@27..27 ""
                  WHITESPACE@27..28 "\n"
                error 15..23: expected tabs, was spaces
            "#]],
        },
        Allowed {
            input: ".a\n  \tcolor: red\n  \tb: c\n",
            change: "indentation mixing tabs and spaces: the layout frontend reports it, \
                     once (Dart: `Tabs and spaces may not be mixed.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..25
                  RULE_SET@0..24
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..24
                      LBRACE@2..2 ""
                      DECLARATION@2..16
                        PROPERTY@2..11
                          WHITESPACE@2..6 "\n  \t"
                          IDENT@6..11 "color"
                        COLON@11..12 ":"
                        VALUE@12..16
                          VALUE@12..16
                            WHITESPACE@12..13 " "
                            IDENT@13..16 "red"
                        SEMICOLON@16..16 ""
                      DECLARATION@16..24
                        PROPERTY@16..21
                          WHITESPACE@16..20 "\n  \t"
                          IDENT@20..21 "b"
                        COLON@21..22 ":"
                        VALUE@22..24
                          VALUE@22..24
                            WHITESPACE@22..23 " "
                            IDENT@23..24 "c"
                        SEMICOLON@24..24 ""
                      RBRACE@24..24 ""
                  WHITESPACE@24..25 "\n"
                error 3..6: tabs and spaces may not be mixed
            "#]],
        },
        Allowed {
            input: ".a\n    .b\n      c: d\n  .e\n    f: g\n",
            change: "a dedent to a column no open block has: the layout frontend reports \
                     it and closes the blocks deeper than the line, as legacy silently did \
                     (Dart: `Inconsistent indentation, expected 4 spaces.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..35
                  RULE_SET@0..20
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..20
                      LBRACE@2..2 ""
                      RULE_SET@2..20
                        SELECTOR_LIST@2..9
                          SELECTOR@2..9
                            SIMPLE_SELECTOR@2..9
                              WHITESPACE@2..7 "\n    "
                              DOT@7..8 "."
                              IDENT@8..9 "b"
                        BLOCK@9..20
                          LBRACE@9..9 ""
                          DECLARATION@9..20
                            PROPERTY@9..17
                              WHITESPACE@9..16 "\n      "
                              IDENT@16..17 "c"
                            COLON@17..18 ":"
                            VALUE@18..20
                              VALUE@18..20
                                WHITESPACE@18..19 " "
                                IDENT@19..20 "d"
                            SEMICOLON@20..20 ""
                          RBRACE@20..20 ""
                      RBRACE@20..20 ""
                  RULE_SET@20..34
                    SELECTOR_LIST@20..25
                      SELECTOR@20..25
                        SIMPLE_SELECTOR@20..25
                          WHITESPACE@20..23 "\n  "
                          DOT@23..24 "."
                          IDENT@24..25 "e"
                    BLOCK@25..34
                      LBRACE@25..25 ""
                      DECLARATION@25..34
                        PROPERTY@25..31
                          WHITESPACE@25..30 "\n    "
                          IDENT@30..31 "f"
                        COLON@31..32 ":"
                        VALUE@32..34
                          VALUE@32..34
                            WHITESPACE@32..33 " "
                            IDENT@33..34 "g"
                        SEMICOLON@34..34 ""
                      RBRACE@34..34 ""
                  WHITESPACE@34..35 "\n"
                error 21..23: inconsistent indentation, expected 4 spaces
            "#]],
        },
        Allowed {
            input: "a\n    b: c\n d: e\n",
            change: "a dedent to a column no open block has, from sass-spec \
                     `non_conformant/sass/indentation/error/inconsistent`: reported (Dart: \
                     `Inconsistent indentation, expected 4 spaces.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..17
                  RULE_SET@0..10
                    SELECTOR_LIST@0..1
                      SELECTOR@0..1
                        SIMPLE_SELECTOR@0..1
                          IDENT@0..1 "a"
                    BLOCK@1..10
                      LBRACE@1..1 ""
                      DECLARATION@1..10
                        PROPERTY@1..7
                          WHITESPACE@1..6 "\n    "
                          IDENT@6..7 "b"
                        COLON@7..8 ":"
                        VALUE@8..10
                          VALUE@8..10
                            WHITESPACE@8..9 " "
                            IDENT@9..10 "c"
                        SEMICOLON@10..10 ""
                      RBRACE@10..10 ""
                  RULE_SET@10..16
                    SELECTOR_LIST@10..16
                      SELECTOR@10..16
                        SIMPLE_SELECTOR@10..13
                          WHITESPACE@10..12 "\n "
                          IDENT@12..13 "d"
                        PSEUDO_SELECTOR@13..16
                          COLON@13..14 ":"
                          WHITESPACE@14..15 " "
                          IDENT@15..16 "e"
                  SEMICOLON@16..16 ""
                  WHITESPACE@16..17 "\n"
                error 11..12: inconsistent indentation, expected 4 spaces
            "#]],
        },
        Allowed {
            input: "a\u{c}  b\r c",
            change: "a dedent to a column no open block has, after a form feed and a \
                     carriage return, from the line-facts tests: reported (Dart: \
                     `Inconsistent indentation, expected 2 spaces.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..8
                  RULE_SET@0..5
                    SELECTOR_LIST@0..1
                      SELECTOR@0..1
                        SIMPLE_SELECTOR@0..1
                          IDENT@0..1 "a"
                    BLOCK@1..5
                      LBRACE@1..1 ""
                      RULE_SET@1..5
                        SELECTOR_LIST@1..5
                          SELECTOR@1..5
                            SIMPLE_SELECTOR@1..5
                              WHITESPACE@1..4 "\u{c}  "
                              IDENT@4..5 "b"
                      SEMICOLON@5..5 ""
                      RBRACE@5..5 ""
                  RULE_SET@5..8
                    SELECTOR_LIST@5..8
                      SELECTOR@5..8
                        SIMPLE_SELECTOR@5..8
                          WHITESPACE@5..7 "\r "
                          IDENT@7..8 "c"
                  SEMICOLON@8..8 ""
                error 6..7: inconsistent indentation, expected 2 spaces
            "#]],
        },
        Allowed {
            input: ".a\n\tb: c\n\t\t.d\n  \te: f\n",
            change: "indentation mixing tabs and spaces, from the line-facts tests: \
                     reported (Dart stops earlier, at `expected \":\".`)",
            expected: expect![[r#"
                SOURCE_FILE@0..22
                  RULE_SET@0..21
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..21
                      LBRACE@2..2 ""
                      NESTED_PROPERTY@2..21
                        PROPERTY@2..5
                          WHITESPACE@2..4 "\n\t"
                          IDENT@4..5 "b"
                        COLON@5..6 ":"
                        VALUE@6..8
                          VALUE@6..8
                            WHITESPACE@6..7 " "
                            IDENT@7..8 "c"
                        BLOCK@8..21
                          LBRACE@8..8 ""
                          RULE_SET@8..21
                            SELECTOR_LIST@8..13
                              SELECTOR@8..13
                                SIMPLE_SELECTOR@8..13
                                  WHITESPACE@8..11 "\n\t\t"
                                  DOT@11..12 "."
                                  IDENT@12..13 "d"
                            BLOCK@13..21
                              LBRACE@13..13 ""
                              DECLARATION@13..21
                                PROPERTY@13..18
                                  WHITESPACE@13..17 "\n  \t"
                                  IDENT@17..18 "e"
                                COLON@18..19 ":"
                                VALUE@19..21
                                  VALUE@19..21
                                    WHITESPACE@19..20 " "
                                    IDENT@20..21 "f"
                                SEMICOLON@21..21 ""
                              RBRACE@21..21 ""
                          RBRACE@21..21 ""
                      RBRACE@21..21 ""
                  WHITESPACE@21..22 "\n"
                error 14..17: tabs and spaces may not be mixed
            "#]],
        },
        Allowed {
            input: ".a\n  b: c\n    ",
            change: "whitespace indented deeper at the end of the input opened a block, \
                     turning the declaration into a nested property; the input ends at \
                     column 0, so it stays a declaration (Dart: `.a { b: c; }`)",
            expected: expect![[r#"
                SOURCE_FILE@0..14
                  RULE_SET@0..14
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..14
                      LBRACE@2..2 ""
                      DECLARATION@2..9
                        PROPERTY@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "b"
                        COLON@6..7 ":"
                        VALUE@7..9
                          VALUE@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
                        SEMICOLON@9..9 ""
                      WHITESPACE@9..14 "\n    "
                      RBRACE@14..14 ""
            "#]],
        },
        Allowed {
            input: "a {\n  b: c\n}\n  ",
            change: "whitespace indented deeper at the end of the input, from sass-spec \
                     `parser/indentation/error/mixed_syntax/block`: no block opens there \
                     (Dart rejects the braces, as both frontends do)",
            expected: expect![[r#"
                SOURCE_FILE@0..15
                  RULE_SET@0..10
                    SELECTOR_LIST@0..1
                      SELECTOR@0..1
                        SIMPLE_SELECTOR@0..1
                          IDENT@0..1 "a"
                    BLOCK@1..10
                      WHITESPACE@1..2 " "
                      LBRACE@2..3 "{"
                      ERROR@3..3
                        LBRACE@3..3 ""
                      DECLARATION@3..10
                        PROPERTY@3..7
                          WHITESPACE@3..6 "\n  "
                          IDENT@6..7 "b"
                        COLON@7..8 ":"
                        VALUE@8..10
                          VALUE@8..10
                            WHITESPACE@8..9 " "
                            IDENT@9..10 "c"
                        SEMICOLON@10..10 ""
                      RBRACE@10..10 ""
                  ERROR@10..12
                    WHITESPACE@10..11 "\n"
                    RBRACE@11..12 "}"
                  SEMICOLON@12..12 ""
                  WHITESPACE@12..15 "\n  "
                error 3..3: expected declaration or nested rule
                error 11..12: expected rule
            "#]],
        },
        Allowed {
            input: "// c\n\n  more\n.a\n  b: c\n",
            change: "a silent comment stopped at a blank line, so the deeper line after it \
                     became a rule of its own; it now takes that line, as in Dart, which \
                     compiles this to `.a { b: c; }`",
            expected: expect![[r#"
                SOURCE_FILE@0..23
                  RULE_SET@0..22
                    SELECTOR_LIST@0..15
                      SELECTOR@0..15
                        SIMPLE_SELECTOR@0..15
                          SINGLE_LINE_COMMENT@0..4 "// c"
                          WHITESPACE@4..8 "\n\n  "
                          SINGLE_LINE_COMMENT@8..12 "more"
                          WHITESPACE@12..13 "\n"
                          DOT@13..14 "."
                          IDENT@14..15 "a"
                    BLOCK@15..22
                      LBRACE@15..15 ""
                      DECLARATION@15..22
                        PROPERTY@15..19
                          WHITESPACE@15..18 "\n  "
                          IDENT@18..19 "b"
                        COLON@19..20 ":"
                        VALUE@20..22
                          VALUE@20..22
                            WHITESPACE@20..21 " "
                            IDENT@21..22 "c"
                        SEMICOLON@22..22 ""
                      RBRACE@22..22 ""
                  WHITESPACE@22..23 "\n"
            "#]],
        },
        Allowed {
            input: ".a /* x\n  y\n.b\n  c: d\n",
            change: "a loud comment that does not start its line and never closes was \
                     silently taken as a comment to the end of the file; it is now \
                     reported (Dart: `expected more input.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..22
                  RULE_SET@0..2
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                  WHITESPACE@2..3 " "
                  ERROR@3..22 "/* x\n  y\n.b\n  c: d\n"
                  SEMICOLON@22..22 ""
                error 3..22: unterminated comment
            "#]],
        },
        Allowed {
            input: ".a\n  b: c /* never closed\n  d: e\n",
            change: "an unterminated loud comment after a value, from the line-facts \
                     tests: reported (Dart: `expected more input.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..33
                  RULE_SET@0..33
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..33
                      LBRACE@2..2 ""
                      DECLARATION@2..33
                        PROPERTY@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "b"
                        COLON@6..7 ":"
                        VALUE@7..9
                          VALUE@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
                        WHITESPACE@9..10 " "
                        ERROR@10..33 "/* never closed\n  d: e\n"
                        SEMICOLON@33..33 ""
                      RBRACE@33..33 ""
                error 10..33: unterminated comment
            "#]],
        },
        Allowed {
            input: "$a: 1 +\n  /* c\n    d\n    2\n.b\n  c: $a\n",
            change: "a loud comment on a continued line is an ordinary one, so one that \
                     never closes runs to the end of the file; legacy took it as a comment, \
                     silently but for the missing operand; the layout frontend reports the \
                     missing operand and the unterminated comment, and nothing else, as the \
                     statement ends where the comment starts (Dart: `expected more \
                     input.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..38
                  VARIABLE_DECL@0..38
                    DOLLAR@0..1 "$"
                    IDENT@1..2 "a"
                    COLON@2..3 ":"
                    BINARY_EXPR@3..7
                      NUMBER_LITERAL@3..5
                        WHITESPACE@3..4 " "
                        NUMBER@4..5 "1"
                      WHITESPACE@5..6 " "
                      PLUS@6..7 "+"
                    WHITESPACE@7..10 "\n  "
                    ERROR@10..38 "/* c\n    d\n    2\n.b\n  c: $a\n"
                    SEMICOLON@38..38 ""
                error 38..38: expected expression
                error 10..38: unterminated comment
            "#]],
        },
        Allowed {
            input: "a\n  b: c /* d\n",
            change: "an unterminated loud comment after a value, from sass-spec \
                     `css/comment/error/loud/unterminated/sass`: reported (Dart: `expected \
                     more input.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..14
                  RULE_SET@0..14
                    SELECTOR_LIST@0..1
                      SELECTOR@0..1
                        SIMPLE_SELECTOR@0..1
                          IDENT@0..1 "a"
                    BLOCK@1..14
                      LBRACE@1..1 ""
                      DECLARATION@1..14
                        PROPERTY@1..5
                          WHITESPACE@1..4 "\n  "
                          IDENT@4..5 "b"
                        COLON@5..6 ":"
                        VALUE@6..8
                          VALUE@6..8
                            WHITESPACE@6..7 " "
                            IDENT@7..8 "c"
                        WHITESPACE@8..9 " "
                        ERROR@9..14 "/* d\n"
                        SEMICOLON@14..14 ""
                      RBRACE@14..14 ""
                error 9..14: unterminated comment
            "#]],
        },
        Allowed {
            input: ".a\n  b: fn(1\n.c\n  d: e\n.f\n  g: h\n",
            change: "an unclosed bracket made every later line break plain whitespace, so \
                     the rest of the file joined the declaration; once its error is \
                     reported, the bracket ends at the next line no deeper than the one it \
                     opened on (Dart: one error, `Expected digit.`)",
            expected: expect![[r#"
                SOURCE_FILE@0..33
                  RULE_SET@0..12
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..12
                      LBRACE@2..2 ""
                      DECLARATION@2..12
                        PROPERTY@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "b"
                        COLON@6..7 ":"
                        VALUE@7..12
                          FUNCTION_CALL@7..12
                            WHITESPACE@7..8 " "
                            IDENT@8..10 "fn"
                            ARG_LIST@10..12
                              LPAREN@10..11 "("
                              ARG@11..12
                                NUMBER_LITERAL@11..12
                                  NUMBER@11..12 "1"
                        SEMICOLON@12..12 ""
                      RBRACE@12..12 ""
                  RULE_SET@12..22
                    SELECTOR_LIST@12..15
                      SELECTOR@12..15
                        SIMPLE_SELECTOR@12..15
                          WHITESPACE@12..13 "\n"
                          DOT@13..14 "."
                          IDENT@14..15 "c"
                    BLOCK@15..22
                      LBRACE@15..15 ""
                      DECLARATION@15..22
                        PROPERTY@15..19
                          WHITESPACE@15..18 "\n  "
                          IDENT@18..19 "d"
                        COLON@19..20 ":"
                        VALUE@20..22
                          VALUE@20..22
                            WHITESPACE@20..21 " "
                            IDENT@21..22 "e"
                        SEMICOLON@22..22 ""
                      RBRACE@22..22 ""
                  RULE_SET@22..32
                    SELECTOR_LIST@22..25
                      SELECTOR@22..25
                        SIMPLE_SELECTOR@22..25
                          WHITESPACE@22..23 "\n"
                          DOT@23..24 "."
                          IDENT@24..25 "f"
                    BLOCK@25..32
                      LBRACE@25..25 ""
                      DECLARATION@25..32
                        PROPERTY@25..29
                          WHITESPACE@25..28 "\n  "
                          IDENT@28..29 "g"
                        COLON@29..30 ":"
                        VALUE@30..32
                          VALUE@30..32
                            WHITESPACE@30..31 " "
                            IDENT@31..32 "h"
                        SEMICOLON@32..32 ""
                      RBRACE@32..32 ""
                  WHITESPACE@32..33 "\n"
                error 13..14: expected RPAREN
            "#]],
        },
        Allowed {
            input: ".a\n  b: fn(1\n.c\n  d: e\n",
            change: "an unclosed bracket, as above, from the `parse_sass_legacy` tests",
            expected: expect![[r#"
                SOURCE_FILE@0..23
                  RULE_SET@0..12
                    SELECTOR_LIST@0..2
                      SELECTOR@0..2
                        SIMPLE_SELECTOR@0..2
                          DOT@0..1 "."
                          IDENT@1..2 "a"
                    BLOCK@2..12
                      LBRACE@2..2 ""
                      DECLARATION@2..12
                        PROPERTY@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "b"
                        COLON@6..7 ":"
                        VALUE@7..12
                          FUNCTION_CALL@7..12
                            WHITESPACE@7..8 " "
                            IDENT@8..10 "fn"
                            ARG_LIST@10..12
                              LPAREN@10..11 "("
                              ARG@11..12
                                NUMBER_LITERAL@11..12
                                  NUMBER@11..12 "1"
                        SEMICOLON@12..12 ""
                      RBRACE@12..12 ""
                  RULE_SET@12..22
                    SELECTOR_LIST@12..15
                      SELECTOR@12..15
                        SIMPLE_SELECTOR@12..15
                          WHITESPACE@12..13 "\n"
                          DOT@13..14 "."
                          IDENT@14..15 "c"
                    BLOCK@15..22
                      LBRACE@15..15 ""
                      DECLARATION@15..22
                        PROPERTY@15..19
                          WHITESPACE@15..18 "\n  "
                          IDENT@18..19 "d"
                        COLON@19..20 ":"
                        VALUE@20..22
                          VALUE@20..22
                            WHITESPACE@20..21 " "
                            IDENT@21..22 "e"
                        SEMICOLON@22..22 ""
                      RBRACE@22..22 ""
                  WHITESPACE@22..23 "\n"
                error 13..14: expected RPAREN
            "#]],
        },
    ]
}
