//! `parse_sass`: indented Sass with statement boundaries decided by the parser.

use expect_test::{Expect, expect};

#[path = "common/layout_fixes.rs"]
mod layout_fixes;
#[path = "common/layout_pins.rs"]
mod layout_pins;
#[path = "common/render.rs"]
mod render;

use render::render;
use sass_parser::parse_sass;

#[allow(clippy::needless_pass_by_value)]
#[track_caller]
fn check(source: &str, expect: Expect) {
    expect.assert_eq(&render(parse_sass, source));
}

/// The tree of an input pinned in `layout_pins`, where every change to it shows.
#[track_caller]
fn check_pinned(source: &str) -> String {
    assert!(
        layout_pins::PINNED.contains(&source),
        "{source:?} is not in layout_pins::PINNED"
    );
    render(parse_sass, source)
}

fn error_count(source: &str) -> usize {
    parse_sass(source).1.len()
}

// ── Pinned trees ─────────────────────────────────────────────────────

/// One line per pinned input: its `layout_pins::fingerprint`, then the input.
#[test]
fn pinned_inputs_keep_their_trees() {
    use std::fmt::Write as _;
    let mut buf = String::new();
    for source in layout_pins::PINNED {
        let (green, errors) = parse_sass(source);
        let errors: Vec<String> = errors
            .into_iter()
            .map(|(msg, range)| format!("{range:?}: {msg}"))
            .collect();
        let tree = sass_parser::syntax::SyntaxNode::new_root(green);
        let _ = writeln!(
            buf,
            "{} {source:?}",
            layout_pins::fingerprint(&tree, &errors)
        );
    }
    expect![[r#"
        0 2/1/2 eb96a6305a65f8d7 ".a\n  >\n    b: c\n"
        0 0/1/2 f6c32d0c821ff792 "@foo\n  10%,\n  20%\n    a: b\n"
        0 1/1/1 a6b3f009e09fa1ce ".a,\n\n.b\n  c: d\n"
        0 1/1/2 92fd432360cf806f ".x\n  @for $i from 1 through\n    3\n    a: $i\n"
        0 1/1/2 500acdf27a8386f2 ".x\n  @each $v in\n    1 2\n    a: $v\n"
        0 1/1/1 5c8554264622b340 ".a\n  b: c and\n    d\n"
        0 1/1/1 1b6da027c2b90344 ".a\n  b: c and\n  d\n"
        0 1/1/1 fa043016df87ae91 ".a\n  b: c or\n    d\n"
        0 1/1/1 83af0dd4febd320e ".a\n  b: 1 and\n    2\n"
        0 1/1/1 3f4eb5120f37b880 ".a\n  b: c not\n    d\n"
        0 1/1/1 7ad1415568924408 ".a\n  content: not\n    d\n"
        0 1/1/1 30e6fae66692689c ".a\n  b: c and\n    d e\n"
        0 1/1/1 f3d8d23d116150e8 ".a\n  b: c, d and\n  e\n"
        0 1/2/1 9eee026d190aa05b ".a\n  b: c and\n    d\n  e: f\n"
        2 0/0/1 0e590db948e18dca "+J@\n\t(\n"
        2 0/0/1 8ada089f97a0a26e "=@\n ,\n"
        2 0/0/1 c5e1d216dc8d69a4 "=,\u{c}@\u{c} ,"
        1 1/2/1 d1216e15a70589f2 ".a\n  b: fn(1, !\n    2)\n  c: d\n"
        1 1/0/0 59d58f4731cda17e ".a:not(.b\n  c: d\n"
        0 0/0/0 c76465eff96f3a72 "$a: 1 +\n  2\n"
        0 1/1/2 4d714f6bdea6e3ed "@each $a\n  , $b in (1 2)\n  .x\n    c: $a\n"
        0 0/0/1 9233f1ec933982a9 "@function f\n  ($a)\n  @return $a\n"
        0 0/1/1 d43687692a51fa27 "@if\n  $a\n  b: c\n"
        0 0/0/1 69616a77eed4fa8c "@function f($a)\n  @return $a +\n    1\n"
        0 0/0/0 632b785bec4c6176 "$m: (\na: 1\n)\n"
        0 1/1/1 7c528574d3a2ab16 ".a\n  b: fn(1,\n2)\n"
        0 1/1/2 67aa0abbb34db6fa "@for $i\n  from 1 through 3\n  .a\n    b: $i\n"
        0 1/1/1 dc09eac74d5aba34 ".a,\n.b\n  c: d\n"
        0 0/0/0 8ad48e992627dea2 "$a:\n  1px\n"
        0 1/1/1 8ae2a29994e49ad6 ".a,\n  .b\n  c: d\n"
        0 1/1/1 6c16506d0b30a6ba ".a,\n\n  .b\n  c: d\n"
        1 1/0/1 ab6a8a2c3e0077df ".a\n  @extend .b,\n    .c\n"
        1 1/0/1 d8b90313eda02eef ".a\n  @extend .b,\n\n    .c\n"
        0 0/1/1 0a657a989ebd52c3 "@each $a,\n  $b in c\n  d: e\n"
        0 0/1/1 e152ed8a7c7d9490 "@each $a,\n\n  $b in c\n  d: e\n"
        0 1/1/1 3af2482497054451 "@each $a in b,\n  c\n  d: e\n"
        0 1/1/1 205fb4f35efc999b "@each $a in b,\n\n  c\n  d: e\n"
        1 1/0/2 4fffdc09013420a9 ".a\n  b: c,\n    d\n"
        1 1/0/2 d499210d695003c2 ".a\n  b: c,\n\n    d\n"
        1 0/0/0 a477d847a06150da "$a: 1,\n  2\n"
        1 0/0/0 1cf70f55a7c95fca "$a: 1,\n\n  2\n"
        1 1/0/1 e1ab5e3ed1664589 "@use \"a\" show b,\n  c\n"
        1 1/0/1 50cd6980805fe1f9 "@use \"a\" show b,\n\n  c\n"
        1 0/0/0 fb1fba0ab0217c32 "@import \"a\",\n  \"b\"\n"
        1 0/0/0 b7c641213771173e "@import \"a\",\n\n  \"b\"\n"
        0 1/0/1 18c70eab41119431 "@layer a,\n  b\n"
        0 1/0/1 db158f83f276f721 "@layer a,\n\n  b\n"
        0 0/1/1 68b7ccf3f1cd483f "@keyframes k\n  10%,\n    20%\n    a: b\n"
        0 0/1/1 d1ac03e44c455bcb "@keyframes k\n  10%,\n\n    20%\n    a: b\n"
        0 0/1/2 cea8da6698d3d7d9 "@foo\n  10%,\n\n  20%\n    a: b\n"
        1 2/1/2 b1d8fb531096aa66 "@media screen,\n  print\n  .a\n    b: c\n"
        1 2/1/2 aa3df501091786f6 "@media screen,\n\n  print\n  .a\n    b: c\n"
        0 1/1/1 7771d78d090b1d3f "@foo a,\n  b\n  c: d\n"
        0 1/1/1 f2767fd1b57ac5d9 "@foo a,\n\n  b\n  c: d\n"
        1 1/0/1 617e4413ab1a3c67 ".a\n  --x: a,\n    b\n"
        1 1/0/1 8dafb50078494e0f ".a\n  --x: a,\n\n    b\n"
        1 1/1/3 5041afde7c5672c0 "@mixin foo($x: 1)\n  a: $x\n.a\n  @include foo\n    ($x)\n"
        1 0/0/0 3fdbf148fe9fa011 "@use \"x\"\n  as y\n"
        1 1/1/2 4e4f058e8864b1d3 "@media screen and\n  (min-width: 1px)\n  .a\n    b: c\n"
        1 1/1/2 1dae10ea34a46e7f "@container card (width > 400px) and\n  (height > 100px)\n  .a\n    b: c\n"
        1 1/1/2 060a5256bb43c5d9 "@container card\n  (width > 400px)\n  .a\n    b: c\n"
        1 1/1/2 bb3cb6da02d3fba8 "@scope (.a) to\n  (.b)\n  .c\n    d: e\n"
        1 2/1/3 25e9c8b4505ba733 ".a\n  @at-root\n    (without: media)\n    .b\n      c: d\n"
        1 1/2/2 047364f17e9bf257 "$b: true\n@if false\n  a: b\n@else\n  if $b\n  c: d\n"
        1 1/1/1 593c1878a5c72ed1 "$x: true\n$y: true\n@if $x\n  and $y\n  a: b\n"
        0 2/1/1 09c5119713d0b891 ".a\n  b: c,\n  d\n"
        1 0/0/1 fa0b5f7a5dc62802 "@function f()\n  @return 1,\n    2\n"
        1 0/0/0 135749537c7d163f "$a: 1\n  2\n"
        1 0/0/0 815f61f886e58e95 "$a: 1\n  , 2\n"
        0 1/1/1 2ea658a57d75dfaa "$m: (\n  a: 1,\n    b: 2\n)\n.x\n  c: map-get($m,\n'a')\n"
        0 1/1/2 61bca1e6e258d4c0 ".a\n  font:\n    family: fantasy\n"
        0 1/1/2 e047af0c0b7c55b2 ".a\n  font: bold\n    family: x\n"
        0 1/1/2 7b808659ee4a3575 "@mixin foo\n  // c\n.a\n  b: c\n"
        0 1/1/2 4cc0ceaa7dd88070 "@media screen\n  /* c */\n.a\n  b: c\n"
        0 2/0/1 085969a8dc7f06b0 ".a\n  color:red\n"
        3 0/0/0 d0203306c0abcb6a "@namespace svg\n  \"http://www.w3.org/2000/svg\"\n"
        0 0/1/1 a32eee13df33d86b "@each $a\n  , $b in (1 2)\n  c: $a\n"
        0 0/0/1 078b472a060189b4 "@function f\n  ($a)\n  @return $a +\n    1\n"
        0 1/1/1 1e088c2aea759b35 "// c\n  continued\n    deeper\n.a\n  // body\n\n  b: c\n"
        0 1/1/1 46dee23fa19c1dc5 "// c\r\n  crlf continuation\r\n.a\r\n  b: c\r\n"
        0 1/1/1 3374e016d8c6a60a "\u{feff}.a\n  b: c\n"
        0 1/1/1 eff37d3377ae61b0 ".a\n  b: c\n    \n"
        0 0/1/1 45626e74c20683c4 "@mixin m\n  b: c\n// dedented comment\n  d: e\n"
        0 1/1/2 592d1374622af270 "=m\n  b: c\n.a\n  +m\n"
        0 4/4/4 a6c1b6cf1959ff5a ".nav\n  color: red\n  .item\n    margin: 0\n    &:hover\n      color: blue\n.footer\n  padding: 0\n"
        1 2/1/2 c712add10c20f56a "@media screen and (min-width: 100px),\n  print\n  .a\n    color: red\n@include button($bg,\n  $border)\n"
        0 2/2/3 5d2a213a61970c5a "// leading comment\n// second line\n.a\n  // only a comment\n  color: red\n@mixin empty\n  // comment-only body\n.b\n  /* loud */\n  margin: 0\n"
        1 1/1/1 2b6c59de254654ba ".a\n  b: c /* d\n"
        1 1/1/1 c7981c7191786c7a ".a\n  b: c /* d\n  "
        0 1/1/2 61a093a4da9159f6 ".x\n  @if $and\n    a: b\n"
        3 0/0/0 fd1a77b8f8cab340 "\u{e}[:\n}+\n0"
        6 0/1/1 91b829e84a6296dd "$||{z:\u{c},,"
        0 0/0/0 4395014d94f85cb0 "//\n\n //\n 4"
        3 0/0/0 6d4bef7e27410221 "$0*\n"
        4 0/0/0 5f254e93c53cd367 "$||:\u{c}"
        1 1/0/0 837a7e16c16fb00b ":*\n"
        1 0/0/0 ee072256d86b4eaa "=\n\n\t//\n"
        4 0/0/0 d200374b7fef73eb "$+*//\n"
        5 0/0/1 ba4b05b7bc932812 "$@\n ''"
        5 1/0/0 b1b365469fd5b620 "$\n\t //\n #\n"
        1 0/0/1 0302966426c00a7a "=\u{8}\n\t //\n //\u{8}\n"
        0 0/0/0 141ae63eba4b2fae "@:*\n"
        1 1/0/0 dc1918d3e0e1fd82 "@:*\n:"
        3 0/0/0 7fb3f3bfa3f94699 "$/or//or\n"
        0 3/3/3 b950f789bff76a6b ".nav\n  color: red\n  .item\n    margin: 0\n.footer\n  padding: 0\n"
        0 0/1/1 c4f9bf72e55de71a "// c\n@mixin m($a,\n  $b)\n  // body comment\n  width: $a\n@include m(1,\n  2)\n"
        0 0/0/0 cfc2ddf24e38d619 ""
        1 2/2/2 66ebfc9b036c1548 ".a\n  b: (1 +)\n.c\n  d: e\n"
        1 1/2/1 88c2087d3d8aa0e6 ".a\n  b: \"x\n  c: d\n"
        1 1/1/1 22f81f99fbd14b7e ".a\n  b: 'x\n"
        1 1/2/1 e170556ce6aadb53 ".a\n  b: \"x #{$y} z\n  c: d\n"
        1 1/2/1 2ccda346b1993a86 ".a\n  b: \"x #{$y}\n  c: d\n"
        1 1/1/1 3bc3006473ecf3ef "$m: (\n  a: \"x,\n  b: 2,\n)\n.e\n  f: g\n"
        1 1/2/1 474c573be3d07685 ".a\n  b: f(\n    \"x\n    2\n  )\n  c: d\n"
        2 1/1/1 0a6ead4ef5c474e3 "@use \"a\n@import 'b\n.e\n  f: g\n"
        1 2/2/2 a3f5987746395a1e ".a\n  b: f(\"x, 1,\n    2)\n.e\n  f: g\n"
        0 1/1/1 be8549b3b0e3235c ".a\n  b: c\n"
        0 1/1/1 3f084470e2b6a84f ".a // c\n  b: c\n"
        0 1/1/1 c39f356ac37bdce1 ".a\n  b: c // d"
        0 3/2/3 fede4456c60d4005 ".a\n  .b\n    c: d\n.e\n  f: g"
        0 1/1/2 2393f568725d0e59 "@mixin m\n  // c\n.a\n  b: c\n"
        0 0/1/1 5bcb565f56e37c11 "@if $a\n  // c\n\n  b: c\n"
        1 1/0/1 d6c71266f0879257 ".a\n  @extend .b\n    .c\n"
        0 2/2/2 041de18e88d3754f ".a\r\n  b: c\r\n\r\n.d\r\n  e: f\r\n"
        0 1/1/1 ff5c2e5dc9eed55c "  \n.a\n  b: c\n    \n"
    "#]].assert_eq(&buf);
}

#[test]
fn pinned_inputs_are_unique() {
    let pins = layout_pins::PINNED;
    for (i, source) in pins.iter().enumerate() {
        assert!(!pins[..i].contains(source), "{source:?} is pinned twice");
    }
}

// ── Fixed defects ───────────────────────────────────────────────────

#[test]
fn fixed_defects_keep_their_trees() {
    for entry in layout_fixes::fixes() {
        entry.expected.assert_eq(&render(parse_sass, entry.input));
    }
}

#[test]
fn fixes_have_no_duplicates() {
    let list = layout_fixes::fixes();
    for (i, entry) in list.iter().enumerate() {
        assert!(
            list[..i].iter().all(|other| other.input != entry.input),
            "{:?} is on the list twice",
            entry.input,
        );
        assert!(
            layout_fixes::fix_for(entry.input).is_some_and(|found| !found.change.is_empty()),
            "{:?} has no description",
            entry.input,
        );
    }
}

/// The defects with a stated number of diagnostics.
#[test]
fn fixed_defects_report_what_they_should() {
    // A dedent to no open column, a line of the other indentation, mixed indentation.
    assert_eq!(error_count(".a\n    .b\n      c: d\n  .e\n    f: g\n"), 1);
    assert_eq!(error_count(".a\n\tcolor: red\n        b: c\n"), 1);
    assert_eq!(error_count(".a\n  \tcolor: red\n  \tb: c\n"), 1);
    // An unclosed bracket: one error, and the rules after it parse.
    let source = ".a\n  b: fn(1\n.c\n  d: e\n.f\n  g: h\n";
    assert_eq!(error_count(source), 1);
    let tree = render(parse_sass, source);
    assert_eq!(tree.matches("RULE_SET@").count(), 3, "{tree}");
    assert_eq!(tree.matches("DECLARATION@").count(), 3, "{tree}");
    // An unterminated loud comment: the error before it, and the comment itself.
    assert_eq!(
        error_count("$a: 1 +\n  /* c\n    d\n    2\n.b\n  c: $a\n"),
        2
    );
    assert_eq!(error_count(".a /* x\n  y\n.b\n  c: d\n"), 1);
    // The loud comments of sass-spec's `loud.hrx`.
    let loud = layout_fixes::fixes()
        .into_iter()
        .find(|entry| entry.input.starts_with("/* Single-line"))
        .unwrap();
    let tree = render(parse_sass, loud.input);
    assert_eq!(tree.matches("MULTI_LINE_COMMENT@").count(), 8, "{tree}");
    assert_eq!(error_count(loud.input), 0);
}

// ── Forms Dart Sass accepts ──────────────────────────────────────────

#[test]
fn dart_valid_continuations_parse_without_errors() {
    for source in [
        "$a: 1 +\n  2\n",
        "@each $a\n  , $b in (1 2)\n  .x\n    c: $a\n",
        "@function f\n  ($a)\n  @return $a\n",
        "@if\n  $a\n  b: c\n",
        "@function f($a)\n  @return $a +\n    1\n",
        "$m: (\na: 1\n)\n",
        ".a\n  b: fn(1,\n2)\n",
        "@for $i\n  from 1 through 3\n  .a\n    b: $i\n",
        ".a,\n.b\n  c: d\n",
        "$a:\n  1px\n",
    ] {
        let tree = check_pinned(source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

/// Where a header requires more, Dart Sass reads on across the line break: after the
/// name of a rule that takes an argument, after a word that needs one, and at the
/// positions of `@for`, `@each` and `@function` that cannot end the header.
#[test]
fn dart_valid_header_continuations_parse_without_errors() {
    for source in [
        ".x\n  @for $i\n    from 1 through 3\n    a: $i\n",
        ".x\n  @for $i from 1\n    through 3\n    a: $i\n",
        ".x\n  @for $i from 1\nto 3\n    a: $i\n",
        ".x\n  @each $v\n    in 1 2\n    a: $v\n",
        ".x\n  @each $a\n    , $b in (1 2)\n    a: $a\n",
        "@function f\n  ($a)\n  @return $a\n",
        "$a: 1\n.x\n  @if\n    not $a\n    b: c\n",
        "$a: 1\n.x\n  @if $a\n    b: c\n  @else if\n    $a\n    b: d\n",
        "@mixin foo\n  a: b\n.x\n  @include\n    foo\n",
        "@mixin foo\n  @content(1)\n.x\n  @include foo using\n    ($y)\n    b: $y\n",
        ".b\n  c: d\n.a\n  @extend\n    .b\n",
        "@use\n  \"sass:math\"\n",
        "@use \"sass:math\" as\n  m\n",
        "@forward \"a\" show b,\n  c\n",
        ".a\n  font: bold,\n    family: x\n",
        "$a\n  : b\n",
        "$a\n: b\n",
        "$a:\n\n  b\n",
    ] {
        let tree = render(parse_sass, source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

/// Forms Dart Sass rejects, where a line is indented beneath a statement that is complete
/// without it: one diagnostic each, and the rule after them parses.
#[test]
fn dart_rejected_continuations_report_one_error() {
    for source in [
        "@mixin foo($x: 1)\n  a: $x\n.a\n  @include foo\n    ($x)\n",
        "@mixin foo\n  @content\n.a\n  +foo\n    ($x)\n      b: c\n",
        "@use \"x\"\n  as y\n",
        "@use \"sass:map\"\n  with ($a: 1)\n",
        "@media screen and\n  (min-width: 1px)\n  .a\n    b: c\n",
        "@media screen,\n  print\n  .a\n    b: c\n",
        "@media\n  screen\n  .a\n    b: c\n",
        "@supports (a: b)\n  and (c: d)\n  .a\n    b: c\n",
        "@container card (width > 400px) and\n  (height > 100px)\n  .a\n    b: c\n",
        "@container card\n  (width > 400px)\n  .a\n    b: c\n",
        "@scope (.a) to\n  (.b)\n  .c\n    d: e\n",
        ".a\n  @at-root\n    (without: media)\n    .b\n      c: d\n",
        "$b: true\n.a\n  @if false\n    a: b\n  @else\n    if $b\n    c: d\n",
        "$x: true\n$y: true\n.a\n  @if $x\n    and $y\n    a: b\n",
        "$a: 1\n.x\n  @if $a\n    (1)\n    b: c\n",
        ".x\n  @for $i from 1 to 3\n    and 1\n    a: $i\n",
        ".x\n  @each $v in 1\n    , 2\n    a: $v\n",
        ".x\n  @each $v in 1,\n    2\n    a: $v\n",
        ".a\n  b: c,\n    d\n",
        "$a: 1,\n  2\n",
        "@function f()\n  @return 1,\n    2\n",
        "@function f()\n  @return 1\n    (2)\n",
        "@debug 1\n  (2)\n",
        "$a: 1\n  2\n",
        "$a: 1\n  , 2\n",
        "$a: 1\n  !default\n",
        ".b\n  c: d\n.a\n  @extend .b\n    , .c\n",
        ".b\n  c: d\n.a\n  @extend .b,\n    .c\n",
        "@mixin foo\n  @content\n    (1)\n",
        "@import \"a\",\n  \"b\"\n",
        "@import\n  \"a\"\n",
        "@charset\n  \"utf-8\"\n",
        ".x\n  --x: a,\n    b\n",
        ".x\n  --x: a\n    b\n",
    ] {
        let source = format!("{source}.z\n  y: x\n");
        let tree = render(parse_sass, &source);
        assert_eq!(error_count(&source), 1, "{source:?}:\n{tree}");
        assert!(ends_with_rule_z(&source), "{source:?}:\n{tree}");
    }
}

/// Whether the last statement of `source` is the rule `.z` with its declaration.
fn ends_with_rule_z(source: &str) -> bool {
    use sass_parser::syntax_kind::SyntaxKind;
    let root = sass_parser::syntax::SyntaxNode::new_root(parse_sass(source).0);
    root.children().last().is_some_and(|rule| {
        rule.kind() == SyntaxKind::RULE_SET
            && rule.text().to_string().trim() == ".z\n  y: x"
            && rule
                .descendants()
                .any(|node| node.kind() == SyntaxKind::DECLARATION)
    })
}

/// Forms Dart Sass accepts but reads otherwise than as a continued line: the line below
/// is a statement of its own, here an empty rule, or the block of the at-rule above.
#[test]
fn a_line_beneath_a_complete_statement_is_a_statement_of_its_own() {
    for source in [
        ".a\n  b: c,\n  d\n",
        ".x\n  --x: a,\n  b\n",
        "@layer a,\n  b\n",
        "@foo a,\n  b\n  c: d\n",
        "@foo a +\n  b\n  c: d\n",
        "$a: 1\n.x\n  @if $a\n    if\n    b: c\n",
    ] {
        let tree = render(parse_sass, source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

// ── Forms Dart Sass rejects or reads otherwise, as it does ──────────

#[test]
fn an_include_without_arguments_takes_the_deeper_line_as_its_content_block() {
    check(
        "@mixin foo($x: 1)\n  a: $x\n.a\n  @include foo\n    ($x)\n",
        expect![[r#"
            SOURCE_FILE@0..53
              MIXIN_RULE@0..25
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..10 "foo"
                PARAM_LIST@10..17
                  LPAREN@10..11 "("
                  PARAM@11..16
                    DOLLAR@11..12 "$"
                    IDENT@12..13 "x"
                    COLON@13..14 ":"
                    NUMBER_LITERAL@14..16
                      WHITESPACE@14..15 " "
                      NUMBER@15..16 "1"
                  RPAREN@16..17 ")"
                BLOCK@17..25
                  LBRACE@17..17 ""
                  DECLARATION@17..25
                    PROPERTY@17..21
                      WHITESPACE@17..20 "\n  "
                      IDENT@20..21 "a"
                    COLON@21..22 ":"
                    VALUE@22..25
                      VARIABLE_REF@22..25
                        WHITESPACE@22..23 " "
                        DOLLAR@23..24 "$"
                        IDENT@24..25 "x"
                    SEMICOLON@25..25 ""
                  RBRACE@25..25 ""
              RULE_SET@25..52
                SELECTOR_LIST@25..28
                  SELECTOR@25..28
                    SIMPLE_SELECTOR@25..28
                      WHITESPACE@25..26 "\n"
                      DOT@26..27 "."
                      IDENT@27..28 "a"
                BLOCK@28..52
                  LBRACE@28..28 ""
                  INCLUDE_RULE@28..52
                    WHITESPACE@28..31 "\n  "
                    AT@31..32 "@"
                    IDENT@32..39 "include"
                    WHITESPACE@39..40 " "
                    IDENT@40..43 "foo"
                    BLOCK@43..52
                      LBRACE@43..43 ""
                      ERROR@43..52
                        WHITESPACE@43..48 "\n    "
                        LPAREN@48..49 "("
                        DOLLAR@49..50 "$"
                        IDENT@50..51 "x"
                        RPAREN@51..52 ")"
                      SEMICOLON@52..52 ""
                      RBRACE@52..52 ""
                  RBRACE@52..52 ""
              WHITESPACE@52..53 "\n"
            error 48..49: expected declaration or nested rule
        "#]],
    );
}

#[test]
fn nothing_may_be_indented_beneath_a_use_rule() {
    check(
        "@use \"x\"\n  as y\n",
        expect![[r#"
            SOURCE_FILE@0..16
              USE_RULE@0..15
                AT@0..1 "@"
                IDENT@1..4 "use"
                WHITESPACE@4..5 " "
                QUOTED_STRING@5..8 "\"x\""
                ERROR@8..15
                  LBRACE@8..8 ""
                  WHITESPACE@8..11 "\n  "
                  IDENT@11..13 "as"
                  WHITESPACE@13..14 " "
                  IDENT@14..15 "y"
                  SEMICOLON@15..15 ""
                  RBRACE@15..15 ""
              WHITESPACE@15..16 "\n"
            error 8..8: nothing may be indented beneath `@use`
        "#]],
    );
}

#[test]
fn a_media_query_ending_with_and_does_not_read_on_below() {
    check(
        "@media screen and\n  (min-width: 1px)\n  .a\n    b: c\n",
        expect![[r#"
            SOURCE_FILE@0..51
              MEDIA_RULE@0..50
                AT@0..1 "@"
                IDENT@1..6 "media"
                MEDIA_QUERY@6..17
                  WHITESPACE@6..7 " "
                  IDENT@7..13 "screen"
                  WHITESPACE@13..14 " "
                  IDENT@14..17 "and"
                BLOCK@17..50
                  LBRACE@17..17 ""
                  ERROR@17..36
                    WHITESPACE@17..20 "\n  "
                    LPAREN@20..21 "("
                    IDENT@21..30 "min-width"
                    COLON@30..31 ":"
                    WHITESPACE@31..32 " "
                    NUMBER@32..33 "1"
                    IDENT@33..35 "px"
                    RPAREN@35..36 ")"
                  SEMICOLON@36..36 ""
                  RULE_SET@36..50
                    SELECTOR_LIST@36..41
                      SELECTOR@36..41
                        SIMPLE_SELECTOR@36..41
                          WHITESPACE@36..39 "\n  "
                          DOT@39..40 "."
                          IDENT@40..41 "a"
                    BLOCK@41..50
                      LBRACE@41..41 ""
                      DECLARATION@41..50
                        PROPERTY@41..47
                          WHITESPACE@41..46 "\n    "
                          IDENT@46..47 "b"
                        COLON@47..48 ":"
                        VALUE@48..50
                          VALUE@48..50
                            WHITESPACE@48..49 " "
                            IDENT@49..50 "c"
                        SEMICOLON@50..50 ""
                      RBRACE@50..50 ""
                  RBRACE@50..50 ""
              WHITESPACE@50..51 "\n"
            error 17..17: expected media query after `and`
        "#]],
    );
}

#[test]
fn a_container_query_ending_with_and_does_not_read_on_below() {
    check(
        "@container card (width > 400px) and\n  (height > 100px)\n  .a\n    b: c\n",
        expect![[r#"
            SOURCE_FILE@0..69
              CONTAINER_RULE@0..68
                AT@0..1 "@"
                IDENT@1..10 "container"
                WHITESPACE@10..11 " "
                IDENT@11..15 "card"
                WHITESPACE@15..16 " "
                LPAREN@16..17 "("
                IDENT@17..22 "width"
                WHITESPACE@22..23 " "
                GT@23..24 ">"
                WHITESPACE@24..25 " "
                NUMBER@25..28 "400"
                IDENT@28..30 "px"
                RPAREN@30..31 ")"
                WHITESPACE@31..32 " "
                IDENT@32..35 "and"
                BLOCK@35..68
                  LBRACE@35..35 ""
                  ERROR@35..54
                    WHITESPACE@35..38 "\n  "
                    LPAREN@38..39 "("
                    IDENT@39..45 "height"
                    WHITESPACE@45..46 " "
                    GT@46..47 ">"
                    WHITESPACE@47..48 " "
                    NUMBER@48..51 "100"
                    IDENT@51..53 "px"
                    RPAREN@53..54 ")"
                  SEMICOLON@54..54 ""
                  RULE_SET@54..68
                    SELECTOR_LIST@54..59
                      SELECTOR@54..59
                        SIMPLE_SELECTOR@54..59
                          WHITESPACE@54..57 "\n  "
                          DOT@57..58 "."
                          IDENT@58..59 "a"
                    BLOCK@59..68
                      LBRACE@59..59 ""
                      DECLARATION@59..68
                        PROPERTY@59..65
                          WHITESPACE@59..64 "\n    "
                          IDENT@64..65 "b"
                        COLON@65..66 ":"
                        VALUE@66..68
                          VALUE@66..68
                            WHITESPACE@66..67 " "
                            IDENT@67..68 "c"
                        SEMICOLON@68..68 ""
                      RBRACE@68..68 ""
                  RBRACE@68..68 ""
              WHITESPACE@68..69 "\n"
            error 35..35: expected container query after `and`
        "#]],
    );
}

#[test]
fn a_container_name_does_not_read_on_below() {
    check(
        "@container card\n  (width > 400px)\n  .a\n    b: c\n",
        expect![[r#"
            SOURCE_FILE@0..48
              CONTAINER_RULE@0..47
                AT@0..1 "@"
                IDENT@1..10 "container"
                WHITESPACE@10..11 " "
                IDENT@11..15 "card"
                BLOCK@15..47
                  LBRACE@15..15 ""
                  ERROR@15..33
                    WHITESPACE@15..18 "\n  "
                    LPAREN@18..19 "("
                    IDENT@19..24 "width"
                    WHITESPACE@24..25 " "
                    GT@25..26 ">"
                    WHITESPACE@26..27 " "
                    NUMBER@27..30 "400"
                    IDENT@30..32 "px"
                    RPAREN@32..33 ")"
                  SEMICOLON@33..33 ""
                  RULE_SET@33..47
                    SELECTOR_LIST@33..38
                      SELECTOR@33..38
                        SIMPLE_SELECTOR@33..38
                          WHITESPACE@33..36 "\n  "
                          DOT@36..37 "."
                          IDENT@37..38 "a"
                    BLOCK@38..47
                      LBRACE@38..38 ""
                      DECLARATION@38..47
                        PROPERTY@38..44
                          WHITESPACE@38..43 "\n    "
                          IDENT@43..44 "b"
                        COLON@44..45 ":"
                        VALUE@45..47
                          VALUE@45..47
                            WHITESPACE@45..46 " "
                            IDENT@46..47 "c"
                        SEMICOLON@47..47 ""
                      RBRACE@47..47 ""
                  RBRACE@47..47 ""
              WHITESPACE@47..48 "\n"
            error 18..19: expected declaration or nested rule
        "#]],
    );
}

#[test]
fn a_scope_ending_with_to_does_not_read_on_below() {
    check(
        "@scope (.a) to\n  (.b)\n  .c\n    d: e\n",
        expect![[r#"
            SOURCE_FILE@0..36
              SCOPE_RULE@0..35
                AT@0..1 "@"
                IDENT@1..6 "scope"
                WHITESPACE@6..7 " "
                LPAREN@7..8 "("
                DOT@8..9 "."
                IDENT@9..10 "a"
                RPAREN@10..11 ")"
                WHITESPACE@11..12 " "
                IDENT@12..14 "to"
                BLOCK@14..35
                  LBRACE@14..14 ""
                  ERROR@14..21
                    WHITESPACE@14..17 "\n  "
                    LPAREN@17..18 "("
                    DOT@18..19 "."
                    IDENT@19..20 "b"
                    RPAREN@20..21 ")"
                  SEMICOLON@21..21 ""
                  RULE_SET@21..35
                    SELECTOR_LIST@21..26
                      SELECTOR@21..26
                        SIMPLE_SELECTOR@21..26
                          WHITESPACE@21..24 "\n  "
                          DOT@24..25 "."
                          IDENT@25..26 "c"
                    BLOCK@26..35
                      LBRACE@26..26 ""
                      DECLARATION@26..35
                        PROPERTY@26..32
                          WHITESPACE@26..31 "\n    "
                          IDENT@31..32 "d"
                        COLON@32..33 ":"
                        VALUE@33..35
                          VALUE@33..35
                            WHITESPACE@33..34 " "
                            IDENT@34..35 "e"
                        SEMICOLON@35..35 ""
                      RBRACE@35..35 ""
                  RBRACE@35..35 ""
              WHITESPACE@35..36 "\n"
            error 14..14: expected scope boundary after `to`
        "#]],
    );
}

#[test]
fn a_bare_at_root_takes_the_deeper_line_as_its_block() {
    check(
        ".a\n  @at-root\n    (without: media)\n    .b\n      c: d\n",
        expect![[r#"
            SOURCE_FILE@0..53
              RULE_SET@0..52
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..52
                  LBRACE@2..2 ""
                  AT_ROOT_RULE@2..52
                    WHITESPACE@2..5 "\n  "
                    AT@5..6 "@"
                    IDENT@6..13 "at-root"
                    BLOCK@13..52
                      LBRACE@13..13 ""
                      ERROR@13..34
                        WHITESPACE@13..18 "\n    "
                        LPAREN@18..19 "("
                        IDENT@19..26 "without"
                        COLON@26..27 ":"
                        WHITESPACE@27..28 " "
                        IDENT@28..33 "media"
                        RPAREN@33..34 ")"
                      SEMICOLON@34..34 ""
                      RULE_SET@34..52
                        SELECTOR_LIST@34..41
                          SELECTOR@34..41
                            SIMPLE_SELECTOR@34..41
                              WHITESPACE@34..39 "\n    "
                              DOT@39..40 "."
                              IDENT@40..41 "b"
                        BLOCK@41..52
                          LBRACE@41..41 ""
                          DECLARATION@41..52
                            PROPERTY@41..49
                              WHITESPACE@41..48 "\n      "
                              IDENT@48..49 "c"
                            COLON@49..50 ":"
                            VALUE@50..52
                              VALUE@50..52
                                WHITESPACE@50..51 " "
                                IDENT@51..52 "d"
                            SEMICOLON@52..52 ""
                          RBRACE@52..52 ""
                      RBRACE@52..52 ""
                  RBRACE@52..52 ""
              WHITESPACE@52..53 "\n"
            error 18..19: expected declaration or nested rule
        "#]],
    );
}

#[test]
fn a_bare_else_takes_if_below_it_as_a_rule() {
    check(
        "$b: true\n@if false\n  a: b\n@else\n  if $b\n  c: d\n",
        expect![[r#"
            SOURCE_FILE@0..47
              VARIABLE_DECL@0..8
                DOLLAR@0..1 "$"
                IDENT@1..2 "b"
                COLON@2..3 ":"
                BOOL_LITERAL@3..8
                  WHITESPACE@3..4 " "
                  IDENT@4..8 "true"
                SEMICOLON@8..8 ""
              IF_RULE@8..46
                WHITESPACE@8..9 "\n"
                AT@9..10 "@"
                IDENT@10..12 "if"
                BOOL_LITERAL@12..18
                  WHITESPACE@12..13 " "
                  IDENT@13..18 "false"
                BLOCK@18..25
                  LBRACE@18..18 ""
                  DECLARATION@18..25
                    PROPERTY@18..22
                      WHITESPACE@18..21 "\n  "
                      IDENT@21..22 "a"
                    COLON@22..23 ":"
                    VALUE@23..25
                      VALUE@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "b"
                    SEMICOLON@25..25 ""
                  RBRACE@25..25 ""
                ELSE_CLAUSE@25..46
                  WHITESPACE@25..26 "\n"
                  AT@26..27 "@"
                  IDENT@27..31 "else"
                  BLOCK@31..46
                    LBRACE@31..31 ""
                    RULE_SET@31..36
                      SELECTOR_LIST@31..36
                        SELECTOR@31..36
                          SIMPLE_SELECTOR@31..36
                            WHITESPACE@31..34 "\n  "
                            IDENT@34..36 "if"
                    ERROR@36..39
                      WHITESPACE@36..37 " "
                      DOLLAR@37..38 "$"
                      IDENT@38..39 "b"
                    SEMICOLON@39..39 ""
                    DECLARATION@39..46
                      PROPERTY@39..43
                        WHITESPACE@39..42 "\n  "
                        IDENT@42..43 "c"
                      COLON@43..44 ":"
                      VALUE@44..46
                        VALUE@44..46
                          WHITESPACE@44..45 " "
                          IDENT@45..46 "d"
                      SEMICOLON@46..46 ""
                    RBRACE@46..46 ""
              WHITESPACE@46..47 "\n"
            error 37..38: expected newline
        "#]],
    );
}

#[test]
fn an_if_condition_does_not_read_on_at_and_below() {
    check(
        "$x: true\n$y: true\n@if $x\n  and $y\n  a: b\n",
        expect![[r#"
            SOURCE_FILE@0..41
              VARIABLE_DECL@0..8
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                BOOL_LITERAL@3..8
                  WHITESPACE@3..4 " "
                  IDENT@4..8 "true"
                SEMICOLON@8..8 ""
              VARIABLE_DECL@8..17
                WHITESPACE@8..9 "\n"
                DOLLAR@9..10 "$"
                IDENT@10..11 "y"
                COLON@11..12 ":"
                BOOL_LITERAL@12..17
                  WHITESPACE@12..13 " "
                  IDENT@13..17 "true"
                SEMICOLON@17..17 ""
              IF_RULE@17..40
                WHITESPACE@17..18 "\n"
                AT@18..19 "@"
                IDENT@19..21 "if"
                VARIABLE_REF@21..24
                  WHITESPACE@21..22 " "
                  DOLLAR@22..23 "$"
                  IDENT@23..24 "x"
                BLOCK@24..40
                  LBRACE@24..24 ""
                  RULE_SET@24..30
                    SELECTOR_LIST@24..30
                      SELECTOR@24..30
                        SIMPLE_SELECTOR@24..30
                          WHITESPACE@24..27 "\n  "
                          IDENT@27..30 "and"
                  ERROR@30..33
                    WHITESPACE@30..31 " "
                    DOLLAR@31..32 "$"
                    IDENT@32..33 "y"
                  SEMICOLON@33..33 ""
                  DECLARATION@33..40
                    PROPERTY@33..37
                      WHITESPACE@33..36 "\n  "
                      IDENT@36..37 "a"
                    COLON@37..38 ":"
                    VALUE@38..40
                      VALUE@38..40
                        WHITESPACE@38..39 " "
                        IDENT@39..40 "b"
                    SEMICOLON@40..40 ""
                  RBRACE@40..40 ""
              WHITESPACE@40..41 "\n"
            error 31..32: expected newline
        "#]],
    );
}

#[test]
fn a_trailing_comma_ends_a_value_before_a_deeper_line() {
    check(
        ".a\n  b: c,\n    d\n",
        expect![[r#"
            SOURCE_FILE@0..17
              RULE_SET@0..16
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..16
                  LBRACE@2..2 ""
                  NESTED_PROPERTY@2..16
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..10
                      VALUE@7..9
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "c"
                      COMMA@9..10 ","
                    BLOCK@10..16
                      LBRACE@10..10 ""
                      ERROR@10..16
                        WHITESPACE@10..15 "\n    "
                        IDENT@15..16 "d"
                      SEMICOLON@16..16 ""
                      RBRACE@16..16 ""
                  RBRACE@16..16 ""
              WHITESPACE@16..17 "\n"
            error 15..16: expected declaration
        "#]],
    );
}

#[test]
fn a_trailing_comma_ends_a_value_before_a_line_at_its_column() {
    check(
        ".a\n  b: c,\n  d\n",
        expect![[r#"
            SOURCE_FILE@0..15
              RULE_SET@0..14
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..14
                  LBRACE@2..2 ""
                  DECLARATION@2..10
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..10
                      VALUE@7..9
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "c"
                      COMMA@9..10 ","
                    SEMICOLON@10..10 ""
                  RULE_SET@10..14
                    SELECTOR_LIST@10..14
                      SELECTOR@10..14
                        SIMPLE_SELECTOR@10..14
                          WHITESPACE@10..13 "\n  "
                          IDENT@13..14 "d"
                  SEMICOLON@14..14 ""
                  RBRACE@14..14 ""
              WHITESPACE@14..15 "\n"
        "#]],
    );
}

#[test]
fn a_trailing_comma_ends_a_variable_value() {
    check(
        "$a: 1,\n  2\n",
        expect![[r#"
            SOURCE_FILE@0..11
              VARIABLE_DECL@0..10
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                LIST_EXPR@3..6
                  NUMBER_LITERAL@3..5
                    WHITESPACE@3..4 " "
                    NUMBER@4..5 "1"
                  COMMA@5..6 ","
                ERROR@6..10
                  LBRACE@6..6 ""
                  WHITESPACE@6..9 "\n  "
                  NUMBER@9..10 "2"
                  SEMICOLON@10..10 ""
                  RBRACE@10..10 ""
              WHITESPACE@10..11 "\n"
            error 6..6: nothing may be indented beneath a variable declaration
        "#]],
    );
}

#[test]
fn a_trailing_comma_ends_a_return_value() {
    check(
        "@function f()\n  @return 1,\n    2\n",
        expect![[r#"
            SOURCE_FILE@0..33
              FUNCTION_RULE@0..32
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..11 "f"
                PARAM_LIST@11..13
                  LPAREN@11..12 "("
                  RPAREN@12..13 ")"
                BLOCK@13..32
                  LBRACE@13..13 ""
                  RETURN_RULE@13..32
                    WHITESPACE@13..16 "\n  "
                    AT@16..17 "@"
                    IDENT@17..23 "return"
                    LIST_EXPR@23..26
                      NUMBER_LITERAL@23..25
                        WHITESPACE@23..24 " "
                        NUMBER@24..25 "1"
                      COMMA@25..26 ","
                    ERROR@26..32
                      LBRACE@26..26 ""
                      WHITESPACE@26..31 "\n    "
                      NUMBER@31..32 "2"
                      SEMICOLON@32..32 ""
                      RBRACE@32..32 ""
                  RBRACE@32..32 ""
              WHITESPACE@32..33 "\n"
            error 26..26: nothing may be indented beneath `@return`
        "#]],
    );
}

#[test]
fn nothing_may_be_indented_beneath_a_variable_declaration() {
    check(
        "$a: 1\n  2\n",
        expect![[r#"
            SOURCE_FILE@0..10
              VARIABLE_DECL@0..9
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                NUMBER_LITERAL@3..5
                  WHITESPACE@3..4 " "
                  NUMBER@4..5 "1"
                ERROR@5..9
                  LBRACE@5..5 ""
                  WHITESPACE@5..8 "\n  "
                  NUMBER@8..9 "2"
                  SEMICOLON@9..9 ""
                  RBRACE@9..9 ""
              WHITESPACE@9..10 "\n"
            error 5..5: nothing may be indented beneath a variable declaration
        "#]],
    );
}

#[test]
fn a_comma_does_not_continue_a_variable_value_from_below() {
    check(
        "$a: 1\n  , 2\n",
        expect![[r#"
            SOURCE_FILE@0..12
              VARIABLE_DECL@0..11
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                NUMBER_LITERAL@3..5
                  WHITESPACE@3..4 " "
                  NUMBER@4..5 "1"
                ERROR@5..11
                  LBRACE@5..5 ""
                  WHITESPACE@5..8 "\n  "
                  COMMA@8..9 ","
                  WHITESPACE@9..10 " "
                  NUMBER@10..11 "2"
                  SEMICOLON@11..11 ""
                  RBRACE@11..11 ""
              WHITESPACE@11..12 "\n"
            error 5..5: nothing may be indented beneath a variable declaration
        "#]],
    );
}

// ── A colon with no space after it ──────────────────────────────────

/// In the indented syntax Dart Sass reads `name:` followed directly by the start of a
/// name as a selector with a pseudo-class, as `a:hover` is, whatever the name: an empty
/// rule, which compiles to nothing.
#[test]
fn a_colon_followed_directly_by_a_name_is_a_selector() {
    for source in [
        ".a\n  color:red\n",
        ".a\n  color:red blue\n",
        ".a\n  color:-webkit-x\n",
        ".a\n  color:#{x}\n",
        ".a\n  #{b}:c\n",
        "@font-face\n  font-family:foo\n",
        ".a\n  font:bold\n    family: x\n",
    ] {
        let tree = render(parse_sass, source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
        assert!(!tree.contains("DECLARATION@2"), "{source:?}:\n{tree}");
        assert!(
            tree.matches("RULE_SET@").count() >= 1,
            "{source:?}:\n{tree}"
        );
    }
}

/// What does not start a name after the colon makes a declaration, as does a space, and
/// so does any colon in a nested property's block, which holds declarations only, and in
/// a custom property.
#[test]
fn a_colon_followed_by_a_value_is_a_declaration() {
    for source in [
        ".a\n  color:1px\n",
        ".a\n  color:#fff\n",
        ".a\n  color:\"x\"\n",
        ".a\n  color:$x\n",
        ".a\n  color:-1px\n",
        ".a\n  color:(1px)\n",
        ".a\n  color: red\n",
        ".a\n  color:\tred\n",
        ".a\n  --x:red\n",
        ".a\n  *zoom:1\n",
        ".a\n  font:\n    family:x\n",
    ] {
        let tree = render(parse_sass, source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
        assert!(
            tree.contains("DECLARATION@") || tree.contains("CUSTOM_PROPERTY_DECL@"),
            "{source:?}:\n{tree}"
        );
    }
}

#[test]
fn a_colon_rule_is_an_empty_rule() {
    check(
        ".a\n  color:red\n  d: e\n",
        expect![[r#"
            SOURCE_FILE@0..22
              RULE_SET@0..21
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..21
                  LBRACE@2..2 ""
                  RULE_SET@2..14
                    SELECTOR_LIST@2..14
                      SELECTOR@2..14
                        SIMPLE_SELECTOR@2..10
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..10 "color"
                        PSEUDO_SELECTOR@10..14
                          COLON@10..11 ":"
                          IDENT@11..14 "red"
                  SEMICOLON@14..14 ""
                  DECLARATION@14..21
                    PROPERTY@14..18
                      WHITESPACE@14..17 "\n  "
                      IDENT@17..18 "d"
                    COLON@18..19 ":"
                    VALUE@19..21
                      VALUE@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "e"
                    SEMICOLON@21..21 ""
                  RBRACE@21..21 ""
              WHITESPACE@21..22 "\n"
        "#]],
    );
}

/// Read as a selector, `color:red !important` has a stray `!important` on its line: one
/// error, as Dart Sass reports it, and the next statement parses.
#[test]
fn a_colon_rule_with_a_flag_is_one_error() {
    check(
        ".a\n  color:red !important\n  d: e\n",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..32
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..32
                  LBRACE@2..2 ""
                  RULE_SET@2..14
                    SELECTOR_LIST@2..14
                      SELECTOR@2..14
                        SIMPLE_SELECTOR@2..10
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..10 "color"
                        PSEUDO_SELECTOR@10..14
                          COLON@10..11 ":"
                          IDENT@11..14 "red"
                  ERROR@14..25
                    WHITESPACE@14..15 " "
                    BANG@15..16 "!"
                    IDENT@16..25 "important"
                  SEMICOLON@25..25 ""
                  DECLARATION@25..32
                    PROPERTY@25..29
                      WHITESPACE@25..28 "\n  "
                      IDENT@28..29 "d"
                    COLON@29..30 ":"
                    VALUE@30..32
                      VALUE@30..32
                        WHITESPACE@30..31 " "
                        IDENT@31..32 "e"
                    SEMICOLON@32..32 ""
                  RBRACE@32..32 ""
              WHITESPACE@32..33 "\n"
            error 15..16: expected newline
        "#]],
    );
}

// ── Blank lines in a continued statement ────────────────────────────

/// Where a header requires more, Dart Sass reads on across blank lines too: after the
/// rule's name and after a word that needs an argument, a comment line or not.
#[test]
fn a_header_goes_on_across_a_blank_line() {
    for source in [
        "@mixin\n\n  foo\n  a: b\n",
        "@mixin\n\n\n  foo\n  a: b\n",
        "@mixin\n  // c\n\n  foo\n  a: b\n",
        "=\n\n  foo\n  a: b\n",
        "@function\n\n  f()\n  @return 1\n",
        "@mixin foo\n  a: b\n.x\n  @include\n\n    foo\n",
        ".b\n  c: d\n.a\n  @extend\n\n    .b\n",
        ".x\n  @if\n\n    true\n    a: b\n",
        "@use \"sass:math\" as\n\n  m\n",
        "@mixin\n\nfoo\n  a: b\n.x\n  +foo\n",
        "=\n\nfoo\n  a: b\n.x\n  +foo\n",
        "@use \"sass:math\" as\n\nm\n.a\n  b: m.$pi\n",
        ".x\n  @each $v in\n\n    1 2\n    a: $v\n",
        "$a: 1\n.x\n  @if $a and\n\n    $a\n    b: c\n",
    ] {
        let tree = render(parse_sass, source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

#[test]
fn important_after_a_blank_line_ends_the_declaration() {
    check(
        ".a\n  b: c !\n\n  important\n",
        expect![[r#"
        SOURCE_FILE@0..25
          RULE_SET@0..24
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..24
              LBRACE@2..2 ""
              DECLARATION@2..24
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..9
                  VALUE@7..9
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "c"
                IMPORTANT@9..24
                  WHITESPACE@9..10 " "
                  BANG@10..11 "!"
                  WHITESPACE@11..15 "\n\n  "
                  IDENT@15..24 "important"
                SEMICOLON@24..24 ""
              RBRACE@24..24 ""
          WHITESPACE@24..25 "\n"
    "#]],
    );
}

#[test]
fn important_on_the_next_line_ends_the_declaration() {
    check(
        ".a\n  b: c !\n  important\n",
        expect![[r#"
        SOURCE_FILE@0..24
          RULE_SET@0..23
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..23
              LBRACE@2..2 ""
              DECLARATION@2..23
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..9
                  VALUE@7..9
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "c"
                IMPORTANT@9..23
                  WHITESPACE@9..10 " "
                  BANG@10..11 "!"
                  WHITESPACE@11..14 "\n  "
                  IDENT@14..23 "important"
                SEMICOLON@23..23 ""
              RBRACE@23..23 ""
          WHITESPACE@23..24 "\n"
    "#]],
    );
}

#[test]
fn bang_before_a_blank_line_and_a_rule_is_one_error() {
    check(
        ".a\n  b: c !\n\n.d\n  e: f\n",
        expect![[r#"
        SOURCE_FILE@0..23
          RULE_SET@0..11
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..11
              LBRACE@2..2 ""
              DECLARATION@2..11
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..11
                  VALUE@7..9
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "c"
                  ERROR@9..11
                    WHITESPACE@9..10 " "
                    BANG@10..11 "!"
                SEMICOLON@11..11 ""
              RBRACE@11..11 ""
          RULE_SET@11..22
            SELECTOR_LIST@11..15
              SELECTOR@11..15
                SIMPLE_SELECTOR@11..15
                  WHITESPACE@11..13 "\n\n"
                  DOT@13..14 "."
                  IDENT@14..15 "d"
            BLOCK@15..22
              LBRACE@15..15 ""
              DECLARATION@15..22
                PROPERTY@15..19
                  WHITESPACE@15..18 "\n  "
                  IDENT@18..19 "e"
                COLON@19..20 ":"
                VALUE@20..22
                  VALUE@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "f"
                SEMICOLON@22..22 ""
              RBRACE@22..22 ""
          WHITESPACE@22..23 "\n"
        error 11..11: expected `important`
    "#]],
    );
}

#[test]
fn variable_important_on_the_next_line() {
    check(
        "$a: 1 !\n  important\n.a\n  b: $a\n",
        expect![[r#"
        SOURCE_FILE@0..31
          VARIABLE_DECL@0..19
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            NUMBER_LITERAL@3..5
              WHITESPACE@3..4 " "
              NUMBER@4..5 "1"
            IMPORTANT@5..19
              WHITESPACE@5..6 " "
              BANG@6..7 "!"
              WHITESPACE@7..10 "\n  "
              IDENT@10..19 "important"
            SEMICOLON@19..19 ""
          RULE_SET@19..30
            SELECTOR_LIST@19..22
              SELECTOR@19..22
                SIMPLE_SELECTOR@19..22
                  WHITESPACE@19..20 "\n"
                  DOT@20..21 "."
                  IDENT@21..22 "a"
            BLOCK@22..30
              LBRACE@22..22 ""
              DECLARATION@22..30
                PROPERTY@22..26
                  WHITESPACE@22..25 "\n  "
                  IDENT@25..26 "b"
                COLON@26..27 ":"
                VALUE@27..30
                  VARIABLE_REF@27..30
                    WHITESPACE@27..28 " "
                    DOLLAR@28..29 "$"
                    IDENT@29..30 "a"
                SEMICOLON@30..30 ""
              RBRACE@30..30 ""
          WHITESPACE@30..31 "\n"
    "#]],
    );
}

#[test]
fn variable_default_after_a_blank_line_is_one_error() {
    check(
        "$a: 1 !\n\ndefault\n.a\n  b: $a\n",
        expect![[r#"
        SOURCE_FILE@0..28
          VARIABLE_DECL@0..7
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            NUMBER_LITERAL@3..5
              WHITESPACE@3..4 " "
              NUMBER@4..5 "1"
            ERROR@5..7
              WHITESPACE@5..6 " "
              BANG@6..7 "!"
            SEMICOLON@7..7 ""
          RULE_SET@7..16
            SELECTOR_LIST@7..16
              SELECTOR@7..16
                SIMPLE_SELECTOR@7..16
                  WHITESPACE@7..9 "\n\n"
                  IDENT@9..16 "default"
          SEMICOLON@16..16 ""
          RULE_SET@16..27
            SELECTOR_LIST@16..19
              SELECTOR@16..19
                SIMPLE_SELECTOR@16..19
                  WHITESPACE@16..17 "\n"
                  DOT@17..18 "."
                  IDENT@18..19 "a"
            BLOCK@19..27
              LBRACE@19..19 ""
              DECLARATION@19..27
                PROPERTY@19..23
                  WHITESPACE@19..22 "\n  "
                  IDENT@22..23 "b"
                COLON@23..24 ":"
                VALUE@24..27
                  VARIABLE_REF@24..27
                    WHITESPACE@24..25 " "
                    DOLLAR@25..26 "$"
                    IDENT@26..27 "a"
                SEMICOLON@27..27 ""
              RBRACE@27..27 ""
          WHITESPACE@27..28 "\n"
        error 7..7: expected `important`
    "#]],
    );
}

#[test]
fn variable_default_on_the_next_line_is_one_error() {
    check(
        "$a: 1 !\ndefault\n.a\n  b: $a\n",
        expect![[r#"
        SOURCE_FILE@0..27
          VARIABLE_DECL@0..7
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            NUMBER_LITERAL@3..5
              WHITESPACE@3..4 " "
              NUMBER@4..5 "1"
            ERROR@5..7
              WHITESPACE@5..6 " "
              BANG@6..7 "!"
            SEMICOLON@7..7 ""
          RULE_SET@7..15
            SELECTOR_LIST@7..15
              SELECTOR@7..15
                SIMPLE_SELECTOR@7..15
                  WHITESPACE@7..8 "\n"
                  IDENT@8..15 "default"
          SEMICOLON@15..15 ""
          RULE_SET@15..26
            SELECTOR_LIST@15..18
              SELECTOR@15..18
                SIMPLE_SELECTOR@15..18
                  WHITESPACE@15..16 "\n"
                  DOT@16..17 "."
                  IDENT@17..18 "a"
            BLOCK@18..26
              LBRACE@18..18 ""
              DECLARATION@18..26
                PROPERTY@18..22
                  WHITESPACE@18..21 "\n  "
                  IDENT@21..22 "b"
                COLON@22..23 ":"
                VALUE@23..26
                  VARIABLE_REF@23..26
                    WHITESPACE@23..24 " "
                    DOLLAR@24..25 "$"
                    IDENT@25..26 "a"
                SEMICOLON@26..26 ""
              RBRACE@26..26 ""
          WHITESPACE@26..27 "\n"
        error 7..7: expected `important`
    "#]],
    );
}

#[test]
fn a_mixin_name_after_a_blank_line_is_its_name() {
    check(
        "@mixin\n\n  foo\n  a: b\n",
        expect![[r#"
            SOURCE_FILE@0..21
              MIXIN_RULE@0..20
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..10 "\n\n  "
                IDENT@10..13 "foo"
                BLOCK@13..20
                  LBRACE@13..13 ""
                  DECLARATION@13..20
                    PROPERTY@13..17
                      WHITESPACE@13..16 "\n  "
                      IDENT@16..17 "a"
                    COLON@17..18 ":"
                    VALUE@18..20
                      VALUE@18..20
                        WHITESPACE@18..19 " "
                        IDENT@19..20 "b"
                    SEMICOLON@20..20 ""
                  RBRACE@20..20 ""
              WHITESPACE@20..21 "\n"
        "#]],
    );
}

/// An operator's operand may follow it after blank lines, as in Dart Sass, when the line
/// it is on starts like an expression.
#[test]
fn an_operator_goes_on_across_a_blank_line() {
    for source in [
        "$a: 1 +\n\n  2\n",
        "$a: 1 +\n\n2\n",
        ".x\n  b: 1 +\n\n    2\n",
        ".x\n  b: 1 -\n\n    2\n",
        ".x\n  b: c and\n\n    d\n",
        ".x\n  b: not\n\n    c\n",
        ".x\n  b: c and\n\n  d\n",
        ".x\n  b: not\n\n  c\n",
        ".x\n  $a: 1 +\n\n  2\n  d: $a\n",
        "$i: 6\n@while $i >\n\n  5\n  $i: $i - 1\n",
    ] {
        let tree = render(parse_sass, source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

#[test]
fn an_operand_after_a_blank_line_is_the_operand() {
    check(
        ".x\n  b: 1 +\n\n    2\n",
        expect![[r#"
            SOURCE_FILE@0..19
              RULE_SET@0..18
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "x"
                BLOCK@2..18
                  LBRACE@2..2 ""
                  DECLARATION@2..18
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..18
                      NUMBER_LITERAL@7..9
                        WHITESPACE@7..8 " "
                        NUMBER@8..9 "1"
                      UNARY_EXPR@9..18
                        WHITESPACE@9..10 " "
                        PLUS@10..11 "+"
                        NUMBER_LITERAL@11..18
                          WHITESPACE@11..17 "\n\n    "
                          NUMBER@17..18 "2"
                    SEMICOLON@18..18 ""
                  RBRACE@18..18 ""
              WHITESPACE@18..19 "\n"
        "#]],
    );
}

/// A header that ends at the end of the input, after a blank line, is still missing its
/// name: one error.
#[test]
fn a_header_with_only_blank_lines_after_it_is_one_error() {
    assert_eq!(error_count("@mixin\n\n"), 1);
    check(
        "@mixin\n\n.a\n  b: c\n",
        expect![[r#"
        SOURCE_FILE@0..18
          MIXIN_RULE@0..6
            AT@0..1 "@"
            IDENT@1..6 "mixin"
          SEMICOLON@6..6 ""
          RULE_SET@6..17
            SELECTOR_LIST@6..10
              SELECTOR@6..10
                SIMPLE_SELECTOR@6..10
                  WHITESPACE@6..8 "\n\n"
                  DOT@8..9 "."
                  IDENT@9..10 "a"
            BLOCK@10..17
              LBRACE@10..10 ""
              DECLARATION@10..17
                PROPERTY@10..14
                  WHITESPACE@10..13 "\n  "
                  IDENT@13..14 "b"
                COLON@14..15 ":"
                VALUE@15..17
                  VALUE@15..17
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "c"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          WHITESPACE@17..18 "\n"
        error 6..6: expected IDENT
    "#]],
    );
}

/// A header goes on to the next line where it requires more, but a line that does not
/// start what it requires is not the rest of it: the header ends above it, missing what
/// it requires, and the line is a statement of its own.
#[test]
fn an_unfinished_header_leaves_the_line_after_it_a_statement() {
    for source in [
        "@mixin\n\n\n",
        "@mixin\n",
        "=\n\n",
        "@use \"x\" as\n\n",
        "@function f()\n  @return\n\n",
        "@include\n\n",
        "@if\n\n",
        "@each $v in\n\n",
    ] {
        let source = format!("{source}.z\n  y: x\n");
        let tree = render(parse_sass, &source);
        assert_eq!(error_count(&source), 1, "{source:?}:\n{tree}");
        assert!(ends_with_rule_z(&source), "{source:?}:\n{tree}");
    }
    // `@for` reports each part it is missing, but the rule below is still its own.
    let source = "@for $i from\n\n.z\n  y: x\n";
    let tree = render(parse_sass, source);
    assert!(error_count(source) <= 3, "{source:?}:\n{tree}");
    assert!(ends_with_rule_z(source), "{source:?}:\n{tree}");
}

/// A deeper line right below a header missing its name goes on with the header.
#[test]
fn a_mixin_missing_its_name_takes_the_parameters_below_it() {
    check(
        "@mixin\n  ($a)\n  b: $a\n",
        expect![[r#"
        SOURCE_FILE@0..22
          MIXIN_RULE@0..21
            AT@0..1 "@"
            IDENT@1..6 "mixin"
            PARAM_LIST@6..13
              WHITESPACE@6..9 "\n  "
              LPAREN@9..10 "("
              PARAM@10..12
                DOLLAR@10..11 "$"
                IDENT@11..12 "a"
              RPAREN@12..13 ")"
            BLOCK@13..21
              LBRACE@13..13 ""
              DECLARATION@13..21
                PROPERTY@13..17
                  WHITESPACE@13..16 "\n  "
                  IDENT@16..17 "b"
                COLON@17..18 ":"
                VALUE@18..21
                  VARIABLE_REF@18..21
                    WHITESPACE@18..19 " "
                    DOLLAR@19..20 "$"
                    IDENT@20..21 "a"
                SEMICOLON@21..21 ""
              RBRACE@21..21 ""
          WHITESPACE@21..22 "\n"
        error 9..10: expected IDENT
    "#]],
    );
}

#[test]
fn a_use_rule_missing_its_namespace_ends_before_the_next_rule() {
    check(
        "@use \"x\" as\n\n.z\n  y: x\n",
        expect![[r#"
        SOURCE_FILE@0..23
          USE_RULE@0..11
            AT@0..1 "@"
            IDENT@1..4 "use"
            WHITESPACE@4..5 " "
            QUOTED_STRING@5..8 "\"x\""
            WHITESPACE@8..9 " "
            IDENT@9..11 "as"
            SEMICOLON@11..11 ""
          RULE_SET@11..22
            SELECTOR_LIST@11..15
              SELECTOR@11..15
                SIMPLE_SELECTOR@11..15
                  WHITESPACE@11..13 "\n\n"
                  DOT@13..14 "."
                  IDENT@14..15 "z"
            BLOCK@15..22
              LBRACE@15..15 ""
              DECLARATION@15..22
                PROPERTY@15..19
                  WHITESPACE@15..18 "\n  "
                  IDENT@18..19 "y"
                COLON@19..20 ":"
                VALUE@20..22
                  VALUE@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "x"
                SEMICOLON@22..22 ""
              RBRACE@22..22 ""
          WHITESPACE@22..23 "\n"
        error 11..11: expected IDENT
    "#]],
    );
}

#[test]
fn a_return_missing_its_value_ends_before_the_next_rule() {
    check(
        "@function f()\n  @return\n\n.z\n  y: x\n",
        expect![[r#"
        SOURCE_FILE@0..35
          FUNCTION_RULE@0..23
            AT@0..1 "@"
            IDENT@1..9 "function"
            WHITESPACE@9..10 " "
            IDENT@10..11 "f"
            PARAM_LIST@11..13
              LPAREN@11..12 "("
              RPAREN@12..13 ")"
            BLOCK@13..23
              LBRACE@13..13 ""
              RETURN_RULE@13..23
                WHITESPACE@13..16 "\n  "
                AT@16..17 "@"
                IDENT@17..23 "return"
                SEMICOLON@23..23 ""
              RBRACE@23..23 ""
          RULE_SET@23..34
            SELECTOR_LIST@23..27
              SELECTOR@23..27
                SIMPLE_SELECTOR@23..27
                  WHITESPACE@23..25 "\n\n"
                  DOT@25..26 "."
                  IDENT@26..27 "z"
            BLOCK@27..34
              LBRACE@27..27 ""
              DECLARATION@27..34
                PROPERTY@27..31
                  WHITESPACE@27..30 "\n  "
                  IDENT@30..31 "y"
                COLON@31..32 ":"
                VALUE@32..34
                  VALUE@32..34
                    WHITESPACE@32..33 " "
                    IDENT@33..34 "x"
                SEMICOLON@34..34 ""
              RBRACE@34..34 ""
          WHITESPACE@34..35 "\n"
        error 23..23: expected expression
    "#]],
    );
}

// ── Line endings ─────────────────────────────────────────────────────

/// The tree of `source` with its ranges and token texts left out, and its errors' messages:
/// what does not depend on how long each line break is.
fn shape(source: &str) -> String {
    let tree = render(parse_sass, source);
    tree.lines()
        .map(|line| {
            let line = line.split('@').next().unwrap_or(line);
            if let Some(message) = line.trim_start().strip_prefix("error ") {
                return format!("error{}", &message[message.find(':').unwrap_or(0)..]);
            }
            line.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `\r\n`, a lone `\r` and a form feed are each one line break, as `\n` is: a blank line
/// made of them ends a statement, or does not, as one made of `\n` does, and a comment
/// reaches as far.
#[test]
fn every_line_ending_counts_as_one_line_break() {
    for source in [
        "$a: 1 +\n\n.b\n  c: d\n",
        ".x\n  b: 1 +\n\n    2\n",
        ".a,\n\n.b\n  c: d\n",
        "@mixin\n\n  foo\n  a: b\n",
        ".a\n  b: c\n\n.d\n  e: f\n",
        "// c\n\n  deeper\n.a\n  b: c\n",
        ".a\n  // c\n\n    deeper\n  b: c\n",
        ".a\n  // c\n\n  d\n  b: c\n",
        "$a: 1 +\n  // c\n\n  2\n",
        "@if $a\n  // c\n\n  b: c\n",
        ".a\n  b: c and\n\n  d: e\n",
    ] {
        let expected = shape(source);
        for ending in ["\r\n", "\r", "\x0c"] {
            let variant = source.replace('\n', ending);
            assert_eq!(shape(&variant), expected, "{variant:?}");
        }
        let mixed = source.replacen("\n\n", "\r\n\r", 1);
        assert_eq!(shape(&mixed), expected, "{mixed:?}");
    }
}

#[test]
fn a_blank_line_of_lone_carriage_returns_ends_a_continued_expression() {
    check(
        "$a: 1 +\r\r.b\r  c: d\r",
        expect![[r#"
            SOURCE_FILE@0..19
              VARIABLE_DECL@0..7
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                NUMBER_LITERAL@3..5
                  WHITESPACE@3..4 " "
                  NUMBER@4..5 "1"
                UNARY_EXPR@5..7
                  WHITESPACE@5..6 " "
                  PLUS@6..7 "+"
                SEMICOLON@7..7 ""
              RULE_SET@7..18
                SELECTOR_LIST@7..11
                  SELECTOR@7..11
                    SIMPLE_SELECTOR@7..11
                      WHITESPACE@7..9 "\r\r"
                      DOT@9..10 "."
                      IDENT@10..11 "b"
                BLOCK@11..18
                  LBRACE@11..11 ""
                  DECLARATION@11..18
                    PROPERTY@11..15
                      WHITESPACE@11..14 "\r  "
                      IDENT@14..15 "c"
                    COLON@15..16 ":"
                    VALUE@16..18
                      VALUE@16..18
                        WHITESPACE@16..17 " "
                        IDENT@17..18 "d"
                    SEMICOLON@18..18 ""
                  RBRACE@18..18 ""
              WHITESPACE@18..19 "\r"
            error 7..7: expected expression
        "#]],
    );
}

// ── Line breaks the grammar absorbs ──────────────────────────────────

#[test]
fn an_operator_at_the_end_of_a_line_continues_the_expression() {
    check(
        "$a: 1 +\n2\n",
        expect![[r#"
            SOURCE_FILE@0..10
              VARIABLE_DECL@0..9
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                BINARY_EXPR@3..9
                  NUMBER_LITERAL@3..5
                    WHITESPACE@3..4 " "
                    NUMBER@4..5 "1"
                  WHITESPACE@5..6 " "
                  PLUS@6..7 "+"
                  NUMBER_LITERAL@7..9
                    WHITESPACE@7..8 "\n"
                    NUMBER@8..9 "2"
                SEMICOLON@9..9 ""
              WHITESPACE@9..10 "\n"
        "#]],
    );
}

/// A declaration's value reads `and`, `or` and `not` as words, but Dart Sass reads them
/// as operators: after `and` or `or` with an operand before it, or after `not`, the value
/// goes on on the next line.
#[test]
fn a_word_operator_at_the_end_of_a_value_continues_it() {
    for source in [
        ".a\n  b: c and\n    d\n",
        ".a\n  b: c and\n  d\n",
        ".a\n  b: c or\n    d\n",
        ".a\n  b: 1 and\n    2\n",
        ".a\n  b: c not\n    d\n",
        ".a\n  content: not\n    d\n",
        ".a\n  b: c and\n    d e\n",
        ".a\n  b: c, d and\n  e\n",
        ".a\n  b: c and\n    d\n  e: f\n",
    ] {
        let tree = check_pinned(source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
    check(
        ".a\n  b: c and\n    d\n",
        expect![[r#"
            SOURCE_FILE@0..20
              RULE_SET@0..19
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..19
                  LBRACE@2..2 ""
                  DECLARATION@2..19
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..19
                      VALUE@7..9
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "c"
                      VALUE@9..13
                        WHITESPACE@9..10 " "
                        IDENT@10..13 "and"
                      VALUE@13..19
                        WHITESPACE@13..18 "\n    "
                        IDENT@18..19 "d"
                    SEMICOLON@19..19 ""
                  RBRACE@19..19 ""
              WHITESPACE@19..20 "\n"
        "#]],
    );
}

/// The failing cases: a line that starts like a declaration is no operand. Dart Sass
/// takes it in and reports `expected newline.` at its colon; here it is left a statement
/// of its own, and the operator that has no operand is the one error. `and` or `or` with
/// nothing before it is a plain word, which Dart accepts.
#[test]
fn a_word_operator_does_not_take_a_declaration_into_the_value() {
    for (source, errors) in [
        (".a\n  b: c and\n    d: e\n", 1),
        (".a\n  b: c or\n  d: e\n", 1),
        (".a\n  b: c and\n  d: e\n", 1),
        (".a\n  content: not\n  b: c\n", 1),
        (".a\n  content: and\n  b: c\n", 0),
    ] {
        let tree = render(parse_sass, source);
        assert_eq!(tree.matches(" PROPERTY@").count(), 2, "{source:?}:\n{tree}");
        assert_eq!(error_count(source), errors, "{source:?}:\n{tree}");
    }
}

#[test]
fn a_word_operator_before_a_declaration_is_one_error() {
    check(
        ".a\n  content: not\n  b: c\n",
        expect![[r#"
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
            error 17..17: expected expression after `not`
        "#]],
    );
}

/// Whether `source` has a declaration whose text is `text`, after its indentation.
fn has_declaration(source: &str, text: &str) -> bool {
    use sass_parser::syntax_kind::SyntaxKind;
    let root = sass_parser::syntax::SyntaxNode::new_root(parse_sass(source).0);
    root.descendants().any(|node| {
        node.kind() == SyntaxKind::DECLARATION && node.text().to_string().trim() == text
    })
}

/// The line an operator at the end of its line goes on to is the one after any blank
/// lines: one that starts like a declaration is no operand there either. Dart Sass takes
/// it in and reports `expected newline.` at its colon.
#[test]
fn an_operator_before_a_declaration_past_a_blank_line_is_one_error() {
    for source in [
        ".a\n  content: not\n\n  b: c\n",
        ".a\n  content: 1 and\n\n  b: c\n",
        ".a\n  content: 1 or\n\n  b: c\n",
        ".a\n  content: not\n\n\n  b: c\n",
        ".a\n  a: 1 +\n\n  b: c\n",
        ".a\n  a: 1 -\n\n  b: c\n",
        ".a\n  $a: 1 +\n\n  b: c\n",
        ".a\n  $a: 1 *\n\n  b: c\n",
    ] {
        let tree = render(parse_sass, source);
        assert_eq!(error_count(source), 1, "{source:?}:\n{tree}");
        assert!(has_declaration(source, "b: c"), "{source:?}:\n{tree}");
    }
}

#[test]
fn a_word_operator_before_a_declaration_past_a_blank_line_is_one_error() {
    check(
        ".a\n  content: not\n\n  b: c\n",
        expect![[r#"
            SOURCE_FILE@0..26
              RULE_SET@0..25
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..25
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
                  DECLARATION@17..25
                    PROPERTY@17..22
                      WHITESPACE@17..21 "\n\n  "
                      IDENT@21..22 "b"
                    COLON@22..23 ":"
                    VALUE@23..25
                      VALUE@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "c"
                    SEMICOLON@25..25 ""
                  RBRACE@25..25 ""
              WHITESPACE@25..26 "\n"
            error 17..17: expected expression after `not`
        "#]],
    );
}

#[test]
fn an_operator_before_a_declaration_past_a_blank_line_leaves_it_a_statement() {
    check(
        ".a\n  $a: 1 +\n\n  b: c\n",
        expect![[r#"
            SOURCE_FILE@0..21
              RULE_SET@0..20
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..20
                  LBRACE@2..2 ""
                  VARIABLE_DECL@2..12
                    WHITESPACE@2..5 "\n  "
                    DOLLAR@5..6 "$"
                    IDENT@6..7 "a"
                    COLON@7..8 ":"
                    NUMBER_LITERAL@8..10
                      WHITESPACE@8..9 " "
                      NUMBER@9..10 "1"
                    UNARY_EXPR@10..12
                      WHITESPACE@10..11 " "
                      PLUS@11..12 "+"
                    SEMICOLON@12..12 ""
                  DECLARATION@12..20
                    PROPERTY@12..17
                      WHITESPACE@12..16 "\n\n  "
                      IDENT@16..17 "b"
                    COLON@17..18 ":"
                    VALUE@18..20
                      VALUE@18..20
                        WHITESPACE@18..19 " "
                        IDENT@19..20 "c"
                    SEMICOLON@20..20 ""
                  RBRACE@20..20 ""
              WHITESPACE@20..21 "\n"
            error 12..12: expected expression
        "#]],
    );
}

#[test]
fn a_blank_line_ends_a_continued_expression() {
    check(
        "$a: 1 +\n\n.b\n  c: d\n",
        expect![[r#"
            SOURCE_FILE@0..19
              VARIABLE_DECL@0..7
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                NUMBER_LITERAL@3..5
                  WHITESPACE@3..4 " "
                  NUMBER@4..5 "1"
                UNARY_EXPR@5..7
                  WHITESPACE@5..6 " "
                  PLUS@6..7 "+"
                SEMICOLON@7..7 ""
              RULE_SET@7..18
                SELECTOR_LIST@7..11
                  SELECTOR@7..11
                    SIMPLE_SELECTOR@7..11
                      WHITESPACE@7..9 "\n\n"
                      DOT@9..10 "."
                      IDENT@10..11 "b"
                BLOCK@11..18
                  LBRACE@11..11 ""
                  DECLARATION@11..18
                    PROPERTY@11..15
                      WHITESPACE@11..14 "\n  "
                      IDENT@14..15 "c"
                    COLON@15..16 ":"
                    VALUE@16..18
                      VALUE@16..18
                        WHITESPACE@16..17 " "
                        IDENT@17..18 "d"
                    SEMICOLON@18..18 ""
                  RBRACE@18..18 ""
              WHITESPACE@18..19 "\n"
            error 7..7: expected expression
        "#]],
    );
}

#[test]
fn a_comma_continues_across_a_blank_line() {
    check_pinned(".a,\n\n.b\n  c: d\n");
}

/// Every list the grammar reads outside brackets goes on after a comma at the end of a
/// line, with or without a blank line after it.
#[test]
fn every_comma_separated_list_goes_on_after_a_comma() {
    for source in [
        ".a,\n  .b\n  c: d\n",
        ".a\n  @extend .b,\n    .c\n",
        "@each $a,\n  $b in c\n  d: e\n",
        "@each $a in b,\n  c\n  d: e\n",
        ".a\n  b: c,\n    d\n",
        "$a: 1,\n  2\n",
        "@use \"a\" show b,\n  c\n",
        "@import \"a\",\n  \"b\"\n",
        "@layer a,\n  b\n",
        "@keyframes k\n  10%,\n    20%\n    a: b\n",
        "@foo\n  10%,\n  20%\n    a: b\n",
        "@media screen,\n  print\n  .a\n    b: c\n",
        "@foo a,\n  b\n  c: d\n",
        ".a\n  --x: a,\n    b\n",
    ] {
        check_pinned(source);
        check_pinned(&source.replacen(",\n", ",\n\n", 1));
    }
}

/// The keyframe selectors of an unknown at-rule's block: the comma joins `20%` to `10%`,
/// and the block below belongs to both.
#[test]
fn a_comma_continues_a_keyframe_list_in_an_unknown_at_rule() {
    check_pinned("@foo\n  10%,\n  20%\n    a: b\n");
    check(
        "@foo\n  10%,\n  20%\n    a: b\n",
        expect![[r#"
            SOURCE_FILE@0..27
              GENERIC_AT_RULE@0..26
                AT@0..1 "@"
                IDENT@1..4 "foo"
                BLOCK@4..26
                  LBRACE@4..4 ""
                  KEYFRAME_SELECTOR@4..26
                    WHITESPACE@4..7 "\n  "
                    NUMBER@7..9 "10"
                    PERCENT@9..10 "%"
                    COMMA@10..11 ","
                    WHITESPACE@11..14 "\n  "
                    NUMBER@14..16 "20"
                    PERCENT@16..17 "%"
                    BLOCK@17..26
                      LBRACE@17..17 ""
                      DECLARATION@17..26
                        PROPERTY@17..23
                          WHITESPACE@17..22 "\n    "
                          IDENT@22..23 "a"
                        COLON@23..24 ":"
                        VALUE@24..26
                          VALUE@24..26
                            WHITESPACE@24..25 " "
                            IDENT@25..26 "b"
                        SEMICOLON@26..26 ""
                      RBRACE@26..26 ""
                  RBRACE@26..26 ""
              WHITESPACE@26..27 "\n"
        "#]],
    );
}

#[test]
fn a_selector_list_scanned_across_a_continued_line_stays_a_selector() {
    // The declaration-or-selector scan sees past the comma on the first line.
    check(
        ".x\n  a:hover,\n  b:focus\n    c: d\n",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..32
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "x"
                BLOCK@2..32
                  LBRACE@2..2 ""
                  RULE_SET@2..32
                    SELECTOR_LIST@2..23
                      SELECTOR@2..12
                        SIMPLE_SELECTOR@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "a"
                        PSEUDO_SELECTOR@6..12
                          COLON@6..7 ":"
                          IDENT@7..12 "hover"
                      COMMA@12..13 ","
                      SELECTOR@13..23
                        SIMPLE_SELECTOR@13..17
                          WHITESPACE@13..16 "\n  "
                          IDENT@16..17 "b"
                        PSEUDO_SELECTOR@17..23
                          COLON@17..18 ":"
                          IDENT@18..23 "focus"
                    BLOCK@23..32
                      LBRACE@23..23 ""
                      DECLARATION@23..32
                        PROPERTY@23..29
                          WHITESPACE@23..28 "\n    "
                          IDENT@28..29 "c"
                        COLON@29..30 ":"
                        VALUE@30..32
                          VALUE@30..32
                            WHITESPACE@30..31 " "
                            IDENT@31..32 "d"
                        SEMICOLON@32..32 ""
                      RBRACE@32..32 ""
                  RBRACE@32..32 ""
              WHITESPACE@32..33 "\n"
        "#]],
    );
}

/// A silent comment takes in the lines indented deeper than it after a blank line too, as
/// in Dart Sass; a line at its own column after the blank line is a statement.
#[test]
fn a_silent_comment_goes_on_across_a_blank_line() {
    check(
        ".a\n  // c\n\n    deeper\n  b: c\n",
        expect![[r#"
            SOURCE_FILE@0..29
              RULE_SET@0..28
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..28
                  LBRACE@2..2 ""
                  DECLARATION@2..28
                    PROPERTY@2..25
                      WHITESPACE@2..5 "\n  "
                      SINGLE_LINE_COMMENT@5..9 "// c"
                      WHITESPACE@9..15 "\n\n    "
                      SINGLE_LINE_COMMENT@15..21 "deeper"
                      WHITESPACE@21..24 "\n  "
                      IDENT@24..25 "b"
                    COLON@25..26 ":"
                    VALUE@26..28
                      VALUE@26..28
                        WHITESPACE@26..27 " "
                        IDENT@27..28 "c"
                    SEMICOLON@28..28 ""
                  RBRACE@28..28 ""
              WHITESPACE@28..29 "\n"
        "#]],
    );
}

#[test]
fn a_line_at_the_comment_column_after_a_blank_line_ends_the_comment() {
    check(
        ".a\n  // c\n\n  d\n  b: c\n",
        expect![[r#"
            SOURCE_FILE@0..22
              RULE_SET@0..21
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..21
                  LBRACE@2..2 ""
                  RULE_SET@2..14
                    SELECTOR_LIST@2..14
                      SELECTOR@2..14
                        SIMPLE_SELECTOR@2..14
                          WHITESPACE@2..5 "\n  "
                          SINGLE_LINE_COMMENT@5..9 "// c"
                          WHITESPACE@9..13 "\n\n  "
                          IDENT@13..14 "d"
                  SEMICOLON@14..14 ""
                  DECLARATION@14..21
                    PROPERTY@14..18
                      WHITESPACE@14..17 "\n  "
                      IDENT@17..18 "b"
                    COLON@18..19 ":"
                    VALUE@19..21
                      VALUE@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "c"
                    SEMICOLON@21..21 ""
                  RBRACE@21..21 ""
              WHITESPACE@21..22 "\n"
        "#]],
    );
}

#[test]
fn a_comment_on_a_continued_line_is_an_ordinary_comment() {
    // Read as starting a statement, the comment would take the deeper line below it.
    check(
        "$a: 1 +\n  // c\n    2\n.b\n  c: $a\n",
        expect![[r#"
            SOURCE_FILE@0..32
              VARIABLE_DECL@0..20
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                BINARY_EXPR@3..20
                  NUMBER_LITERAL@3..5
                    WHITESPACE@3..4 " "
                    NUMBER@4..5 "1"
                  WHITESPACE@5..6 " "
                  PLUS@6..7 "+"
                  NUMBER_LITERAL@7..20
                    WHITESPACE@7..10 "\n  "
                    SINGLE_LINE_COMMENT@10..14 "// c"
                    WHITESPACE@14..19 "\n    "
                    NUMBER@19..20 "2"
                SEMICOLON@20..20 ""
              RULE_SET@20..31
                SELECTOR_LIST@20..23
                  SELECTOR@20..23
                    SIMPLE_SELECTOR@20..23
                      WHITESPACE@20..21 "\n"
                      DOT@21..22 "."
                      IDENT@22..23 "b"
                BLOCK@23..31
                  LBRACE@23..23 ""
                  DECLARATION@23..31
                    PROPERTY@23..27
                      WHITESPACE@23..26 "\n  "
                      IDENT@26..27 "c"
                    COLON@27..28 ":"
                    VALUE@28..31
                      VARIABLE_REF@28..31
                        WHITESPACE@28..29 " "
                        DOLLAR@29..30 "$"
                        IDENT@30..31 "a"
                    SEMICOLON@31..31 ""
                  RBRACE@31..31 ""
              WHITESPACE@31..32 "\n"
        "#]],
    );
}

/// A header goes on after one of its words at the end of a line; a variable named like
/// one is no such word, and the line below is the rule's body, as in Dart Sass.
#[test]
fn a_variable_named_like_a_header_word_ends_the_header() {
    check_pinned(".x\n  @for $i from 1 through\n    3\n    a: $i\n");
    check_pinned(".x\n  @each $v in\n    1 2\n    a: $v\n");
    for (source, rule) in [
        (".x\n  @for $i from 1 through $to\n    a: $i\n", "FOR_RULE@"),
        (".x\n  @for $i from 1 to $through\n    a: $i\n", "FOR_RULE@"),
        (
            ".x\n  @for $i from $from through $to\n    a: $i\n",
            "FOR_RULE@",
        ),
        (".x\n  @each $v in $in\n    a: $v\n", "EACH_RULE@"),
        (".x\n  @if $and\n    a: b\n", "IF_RULE@"),
        (".x\n  @if true and $or\n    a: b\n", "IF_RULE@"),
        (".x\n  @while $not\n    a: b\n", "WHILE_RULE@"),
    ] {
        let tree = render(parse_sass, source);
        let body = tree
            .split_once(rule)
            .map(|(_, rest)| rest.contains("BLOCK@") && rest.contains("DECLARATION@"));
        assert_eq!(body, Some(true), "{source:?}:\n{tree}");
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
    check(
        ".x\n  @for $i from 1 through $to\n    a: $i\n",
        expect![[r#"
            SOURCE_FILE@0..42
              RULE_SET@0..41
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "x"
                BLOCK@2..41
                  LBRACE@2..2 ""
                  FOR_RULE@2..41
                    WHITESPACE@2..5 "\n  "
                    AT@5..6 "@"
                    IDENT@6..9 "for"
                    WHITESPACE@9..10 " "
                    DOLLAR@10..11 "$"
                    IDENT@11..12 "i"
                    WHITESPACE@12..13 " "
                    IDENT@13..17 "from"
                    NUMBER_LITERAL@17..19
                      WHITESPACE@17..18 " "
                      NUMBER@18..19 "1"
                    WHITESPACE@19..20 " "
                    IDENT@20..27 "through"
                    VARIABLE_REF@27..31
                      WHITESPACE@27..28 " "
                      DOLLAR@28..29 "$"
                      IDENT@29..31 "to"
                    BLOCK@31..41
                      LBRACE@31..31 ""
                      DECLARATION@31..41
                        PROPERTY@31..37
                          WHITESPACE@31..36 "\n    "
                          IDENT@36..37 "a"
                        COLON@37..38 ":"
                        VALUE@38..41
                          VARIABLE_REF@38..41
                            WHITESPACE@38..39 " "
                            DOLLAR@39..40 "$"
                            IDENT@40..41 "i"
                        SEMICOLON@41..41 ""
                      RBRACE@41..41 ""
                  RBRACE@41..41 ""
              WHITESPACE@41..42 "\n"
        "#]],
    );
}

/// An error ends a statement before its line does; an at-rule after it on the line is a
/// statement of its own, and the header of the broken one no longer decides whether the
/// deeper line below goes on: here it opens the at-rule's block.
#[test]
fn an_at_rule_after_a_broken_statement_ends_its_header() {
    check_pinned("+J@\n\t(\n");
    check_pinned("=@\n ,\n");
    // A comma went on over the line break before the at-rule.
    check_pinned("=,\x0c@\x0c ,");
    check(
        "+m@\n  (a)\n",
        expect![[r#"
            SOURCE_FILE@0..10
              INCLUDE_RULE@0..2
                AT@0..0 ""
                IDENT@0..1 "+"
                IDENT@1..2 "m"
              ERROR@2..9
                AT@2..3 "@"
                BLOCK@3..9
                  LBRACE@3..3 ""
                  ERROR@3..9
                    WHITESPACE@3..6 "\n  "
                    LPAREN@6..7 "("
                    IDENT@7..8 "a"
                    RPAREN@8..9 ")"
                  SEMICOLON@9..9 ""
                  RBRACE@9..9 ""
              WHITESPACE@9..10 "\n"
            error 2..3: expected SEMICOLON
            error 6..7: expected declaration or nested rule
        "#]],
    );
}

#[test]
fn a_header_goes_on_on_a_deeper_line_that_starts_like_its_rest() {
    check(
        "@for $i\n  from 1 through 3\n  .a\n    b: $i\n",
        expect![[r#"
            SOURCE_FILE@0..42
              FOR_RULE@0..41
                AT@0..1 "@"
                IDENT@1..4 "for"
                WHITESPACE@4..5 " "
                DOLLAR@5..6 "$"
                IDENT@6..7 "i"
                WHITESPACE@7..10 "\n  "
                IDENT@10..14 "from"
                NUMBER_LITERAL@14..16
                  WHITESPACE@14..15 " "
                  NUMBER@15..16 "1"
                WHITESPACE@16..17 " "
                IDENT@17..24 "through"
                NUMBER_LITERAL@24..26
                  WHITESPACE@24..25 " "
                  NUMBER@25..26 "3"
                BLOCK@26..41
                  LBRACE@26..26 ""
                  RULE_SET@26..41
                    SELECTOR_LIST@26..31
                      SELECTOR@26..31
                        SIMPLE_SELECTOR@26..31
                          WHITESPACE@26..29 "\n  "
                          DOT@29..30 "."
                          IDENT@30..31 "a"
                    BLOCK@31..41
                      LBRACE@31..31 ""
                      DECLARATION@31..41
                        PROPERTY@31..37
                          WHITESPACE@31..36 "\n    "
                          IDENT@36..37 "b"
                        COLON@37..38 ":"
                        VALUE@38..41
                          VARIABLE_REF@38..41
                            WHITESPACE@38..39 " "
                            DOLLAR@39..40 "$"
                            IDENT@40..41 "i"
                        SEMICOLON@41..41 ""
                      RBRACE@41..41 ""
                  RBRACE@41..41 ""
              WHITESPACE@41..42 "\n"
        "#]],
    );
}

#[test]
fn shorthands_read_as_at_rules() {
    check(
        "=m\n  b: c\n.a\n  +m\n",
        expect![[r#"
            SOURCE_FILE@0..18
              MIXIN_RULE@0..9
                AT@0..0 ""
                IDENT@0..1 "="
                IDENT@1..2 "m"
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
              RULE_SET@9..17
                SELECTOR_LIST@9..12
                  SELECTOR@9..12
                    SIMPLE_SELECTOR@9..12
                      WHITESPACE@9..10 "\n"
                      DOT@10..11 "."
                      IDENT@11..12 "a"
                BLOCK@12..17
                  LBRACE@12..12 ""
                  INCLUDE_RULE@12..17
                    WHITESPACE@12..15 "\n  "
                    AT@15..15 ""
                    IDENT@15..16 "+"
                    IDENT@16..17 "m"
                    SEMICOLON@17..17 ""
                  RBRACE@17..17 ""
              WHITESPACE@17..18 "\n"
        "#]],
    );
}

#[test]
fn a_combinator_alone_on_its_line_does_not_continue_it() {
    check_pinned(".a\n  >\n    b: c\n");
}

// ── Brackets ─────────────────────────────────────────────────────────

#[test]
fn line_breaks_inside_brackets_are_whitespace() {
    check_pinned("$m: (\n  a: 1,\n    b: 2\n)\n.x\n  c: map-get($m,\n'a')\n");
}

#[test]
fn an_unclosed_bracket_with_only_deeper_lines_after_it_ends_at_the_end_of_the_input() {
    expect![[r#"
        SOURCE_FILE@0..17
          RULE_SET@0..16
            SELECTOR_LIST@0..16
              SELECTOR@0..16
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..16
                  COLON@2..3 ":"
                  IDENT@3..6 "not"
                  LPAREN@6..7 "("
                  DOT@7..8 "."
                  IDENT@8..9 "b"
                  WHITESPACE@9..12 "\n  "
                  IDENT@12..13 "c"
                  COLON@13..14 ":"
                  WHITESPACE@14..15 " "
                  IDENT@15..16 "d"
          SEMICOLON@16..16 ""
          WHITESPACE@16..17 "\n"
        error 16..16: expected RPAREN
    "#]]
    .assert_eq(&check_pinned(".a:not(.b\n  c: d\n"));
}

#[test]
fn a_call_below_does_not_close_an_unclosed_media_query() {
    check(
        "@media (a\n  .b\n    c: d\n.e\n  f: g(h))\n",
        expect![[r#"
            SOURCE_FILE@0..38
              MEDIA_RULE@0..23
                AT@0..1 "@"
                IDENT@1..6 "media"
                MEDIA_QUERY@6..23
                  WHITESPACE@6..7 " "
                  LPAREN@7..8 "("
                  IDENT@8..9 "a"
                  WHITESPACE@9..12 "\n  "
                  DOT@12..13 "."
                  IDENT@13..14 "b"
                  WHITESPACE@14..19 "\n    "
                  IDENT@19..20 "c"
                  COLON@20..21 ":"
                  WHITESPACE@21..22 " "
                  IDENT@22..23 "d"
              SEMICOLON@23..23 ""
              RULE_SET@23..37
                SELECTOR_LIST@23..26
                  SELECTOR@23..26
                    SIMPLE_SELECTOR@23..26
                      WHITESPACE@23..24 "\n"
                      DOT@24..25 "."
                      IDENT@25..26 "e"
                BLOCK@26..37
                  LBRACE@26..26 ""
                  DECLARATION@26..37
                    PROPERTY@26..30
                      WHITESPACE@26..29 "\n  "
                      IDENT@29..30 "f"
                    COLON@30..31 ":"
                    VALUE@31..37
                      FUNCTION_CALL@31..36
                        WHITESPACE@31..32 " "
                        IDENT@32..33 "g"
                        ARG_LIST@33..36
                          LPAREN@33..34 "("
                          ARG@34..35
                            VALUE@34..35
                              IDENT@34..35 "h"
                          RPAREN@35..36 ")"
                      ERROR@36..37
                        RPAREN@36..37 ")"
                    SEMICOLON@37..37 ""
                  RBRACE@37..37 ""
              WHITESPACE@37..38 "\n"
            error 23..23: expected RPAREN
            error 36..37: expected newline
        "#]],
    );
}

#[test]
fn a_call_below_does_not_close_an_unclosed_pseudo_class() {
    check(
        ".a:not(.b\n  c: d\n.e\n  f: g\n.h\n  i: j(k))\n",
        expect![[r#"
            SOURCE_FILE@0..41
              RULE_SET@0..16
                SELECTOR_LIST@0..16
                  SELECTOR@0..16
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    PSEUDO_SELECTOR@2..16
                      COLON@2..3 ":"
                      IDENT@3..6 "not"
                      LPAREN@6..7 "("
                      DOT@7..8 "."
                      IDENT@8..9 "b"
                      WHITESPACE@9..12 "\n  "
                      IDENT@12..13 "c"
                      COLON@13..14 ":"
                      WHITESPACE@14..15 " "
                      IDENT@15..16 "d"
              SEMICOLON@16..16 ""
              RULE_SET@16..26
                SELECTOR_LIST@16..19
                  SELECTOR@16..19
                    SIMPLE_SELECTOR@16..19
                      WHITESPACE@16..17 "\n"
                      DOT@17..18 "."
                      IDENT@18..19 "e"
                BLOCK@19..26
                  LBRACE@19..19 ""
                  DECLARATION@19..26
                    PROPERTY@19..23
                      WHITESPACE@19..22 "\n  "
                      IDENT@22..23 "f"
                    COLON@23..24 ":"
                    VALUE@24..26
                      VALUE@24..26
                        WHITESPACE@24..25 " "
                        IDENT@25..26 "g"
                    SEMICOLON@26..26 ""
                  RBRACE@26..26 ""
              RULE_SET@26..40
                SELECTOR_LIST@26..29
                  SELECTOR@26..29
                    SIMPLE_SELECTOR@26..29
                      WHITESPACE@26..27 "\n"
                      DOT@27..28 "."
                      IDENT@28..29 "h"
                BLOCK@29..40
                  LBRACE@29..29 ""
                  DECLARATION@29..40
                    PROPERTY@29..33
                      WHITESPACE@29..32 "\n  "
                      IDENT@32..33 "i"
                    COLON@33..34 ":"
                    VALUE@34..40
                      FUNCTION_CALL@34..39
                        WHITESPACE@34..35 " "
                        IDENT@35..36 "j"
                        ARG_LIST@36..39
                          LPAREN@36..37 "("
                          ARG@37..38
                            VALUE@37..38
                              IDENT@37..38 "k"
                          RPAREN@38..39 ")"
                      ERROR@39..40
                        RPAREN@39..40 ")"
                    SEMICOLON@40..40 ""
                  RBRACE@40..40 ""
              WHITESPACE@40..41 "\n"
            error 16..16: expected RPAREN
            error 39..40: expected newline
        "#]],
    );
}

#[test]
fn a_closer_of_another_kind_does_not_close_a_selector_bracket() {
    check(
        ".a:not(.b\n.c]\n  d: e\n",
        expect![[r#"
            SOURCE_FILE@0..21
              RULE_SET@0..9
                SELECTOR_LIST@0..9
                  SELECTOR@0..9
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    PSEUDO_SELECTOR@2..9
                      COLON@2..3 ":"
                      IDENT@3..6 "not"
                      LPAREN@6..7 "("
                      DOT@7..8 "."
                      IDENT@8..9 "b"
              SEMICOLON@9..9 ""
              RULE_SET@9..12
                SELECTOR_LIST@9..12
                  SELECTOR@9..12
                    SIMPLE_SELECTOR@9..12
                      WHITESPACE@9..10 "\n"
                      DOT@10..11 "."
                      IDENT@11..12 "c"
              ERROR@12..20
                RBRACKET@12..13 "]"
                BLOCK@13..20
                  LBRACE@13..13 ""
                  DECLARATION@13..20
                    PROPERTY@13..17
                      WHITESPACE@13..16 "\n  "
                      IDENT@16..17 "d"
                    COLON@17..18 ":"
                    VALUE@18..20
                      VALUE@18..20
                        WHITESPACE@18..19 " "
                        IDENT@19..20 "e"
                    SEMICOLON@20..20 ""
                  RBRACE@20..20 ""
              WHITESPACE@20..21 "\n"
            error 9..9: expected RPAREN
            error 12..13: expected newline
        "#]],
    );
}

#[test]
fn a_closer_of_another_kind_ends_a_selector_bracket_at_its_line() {
    check(
        ".a:not(.b]\n  c: d\n.e\n  f: g\n",
        expect![[r#"
        SOURCE_FILE@0..28
          RULE_SET@0..17
            SELECTOR_LIST@0..10
              SELECTOR@0..10
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..10
                  COLON@2..3 ":"
                  IDENT@3..6 "not"
                  LPAREN@6..7 "("
                  DOT@7..8 "."
                  IDENT@8..9 "b"
                  RBRACKET@9..10 "]"
            BLOCK@10..17
              LBRACE@10..10 ""
              DECLARATION@10..17
                PROPERTY@10..14
                  WHITESPACE@10..13 "\n  "
                  IDENT@13..14 "c"
                COLON@14..15 ":"
                VALUE@15..17
                  VALUE@15..17
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "d"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          RULE_SET@17..27
            SELECTOR_LIST@17..20
              SELECTOR@17..20
                SIMPLE_SELECTOR@17..20
                  WHITESPACE@17..18 "\n"
                  DOT@18..19 "."
                  IDENT@19..20 "e"
            BLOCK@20..27
              LBRACE@20..20 ""
              DECLARATION@20..27
                PROPERTY@20..24
                  WHITESPACE@20..23 "\n  "
                  IDENT@23..24 "f"
                COLON@24..25 ":"
                VALUE@25..27
                  VALUE@25..27
                    WHITESPACE@25..26 " "
                    IDENT@26..27 "g"
                SEMICOLON@27..27 ""
              RBRACE@27..27 ""
          WHITESPACE@27..28 "\n"
        error 10..10: expected RPAREN
    "#]],
    );
}

#[test]
fn a_closer_of_another_kind_ends_a_media_query_at_its_line() {
    check(
        "@media (a]\n  .b\n    c: d\n",
        expect![[r#"
        SOURCE_FILE@0..25
          MEDIA_RULE@0..24
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..10
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              IDENT@8..9 "a"
              RBRACKET@9..10 "]"
            BLOCK@10..24
              LBRACE@10..10 ""
              RULE_SET@10..24
                SELECTOR_LIST@10..15
                  SELECTOR@10..15
                    SIMPLE_SELECTOR@10..15
                      WHITESPACE@10..13 "\n  "
                      DOT@13..14 "."
                      IDENT@14..15 "b"
                BLOCK@15..24
                  LBRACE@15..15 ""
                  DECLARATION@15..24
                    PROPERTY@15..21
                      WHITESPACE@15..20 "\n    "
                      IDENT@20..21 "c"
                    COLON@21..22 ":"
                    VALUE@22..24
                      VALUE@22..24
                        WHITESPACE@22..23 " "
                        IDENT@23..24 "d"
                    SEMICOLON@24..24 ""
                  RBRACE@24..24 ""
              RBRACE@24..24 ""
          WHITESPACE@24..25 "\n"
        error 10..10: expected RPAREN
    "#]],
    );
}

/// The look ahead at a `{` in a condition stops at the zero-width tokens the layout puts
/// after the `]` that the lexer took for the `(`'s closer: reading on past them, it would
/// see the same token forever.
#[test]
fn a_brace_in_a_condition_with_a_layout_token_after_it_ends() {
    check(
        "@supports (a] {b\n  .x\n    y: z\n",
        expect![[r#"
        SOURCE_FILE@0..31
          SUPPORTS_RULE@0..30
            AT@0..1 "@"
            IDENT@1..9 "supports"
            SUPPORTS_CONDITION@9..16
              WHITESPACE@9..10 " "
              LPAREN@10..11 "("
              IDENT@11..12 "a"
              RBRACKET@12..13 "]"
              WHITESPACE@13..14 " "
              LBRACE@14..15 "{"
              IDENT@15..16 "b"
            BLOCK@16..30
              LBRACE@16..16 ""
              RULE_SET@16..30
                SELECTOR_LIST@16..21
                  SELECTOR@16..21
                    SIMPLE_SELECTOR@16..21
                      WHITESPACE@16..19 "\n  "
                      DOT@19..20 "."
                      IDENT@20..21 "x"
                BLOCK@21..30
                  LBRACE@21..21 ""
                  DECLARATION@21..30
                    PROPERTY@21..27
                      WHITESPACE@21..26 "\n    "
                      IDENT@26..27 "y"
                    COLON@27..28 ":"
                    VALUE@28..30
                      VALUE@28..30
                        WHITESPACE@28..29 " "
                        IDENT@29..30 "z"
                    SEMICOLON@30..30 ""
                  RBRACE@30..30 ""
              RBRACE@30..30 ""
          WHITESPACE@30..31 "\n"
        error 16..16: expected RPAREN
    "#]],
    );
}

/// Known limitation: Dart Sass reads this selector across its lines, but `.c:not(.d)` is
/// indented no deeper than the `(` it continues and has a deeper line below it, so it
/// reads as the selector of a rule of its own, and the `(` as unclosed. Which lines
/// continue a bracket is decided from the layout alone, erring on the side of valid
/// code elsewhere.
#[test]
fn a_bracket_continued_by_a_line_with_a_deeper_one_below_is_cut() {
    check(
        ".a:not(.b\n.c:not(.d)\n  .e)\n  color: red\n",
        expect![[r#"
            SOURCE_FILE@0..40
              RULE_SET@0..9
                SELECTOR_LIST@0..9
                  SELECTOR@0..9
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    PSEUDO_SELECTOR@2..9
                      COLON@2..3 ":"
                      IDENT@3..6 "not"
                      LPAREN@6..7 "("
                      DOT@7..8 "."
                      IDENT@8..9 "b"
              SEMICOLON@9..9 ""
              RULE_SET@9..39
                SELECTOR_LIST@9..20
                  SELECTOR@9..20
                    SIMPLE_SELECTOR@9..12
                      WHITESPACE@9..10 "\n"
                      DOT@10..11 "."
                      IDENT@11..12 "c"
                    PSEUDO_SELECTOR@12..20
                      COLON@12..13 ":"
                      IDENT@13..16 "not"
                      LPAREN@16..17 "("
                      DOT@17..18 "."
                      IDENT@18..19 "d"
                      RPAREN@19..20 ")"
                BLOCK@20..39
                  LBRACE@20..20 ""
                  RULE_SET@20..25
                    SELECTOR_LIST@20..25
                      SELECTOR@20..25
                        SIMPLE_SELECTOR@20..25
                          WHITESPACE@20..23 "\n  "
                          DOT@23..24 "."
                          IDENT@24..25 "e"
                  ERROR@25..26
                    RPAREN@25..26 ")"
                  SEMICOLON@26..26 ""
                  DECLARATION@26..39
                    PROPERTY@26..34
                      WHITESPACE@26..29 "\n  "
                      IDENT@29..34 "color"
                    COLON@34..35 ":"
                    VALUE@35..39
                      VALUE@35..39
                        WHITESPACE@35..36 " "
                        IDENT@36..39 "red"
                    SEMICOLON@39..39 ""
                  RBRACE@39..39 ""
              WHITESPACE@39..40 "\n"
            error 9..9: expected RPAREN
            error 25..26: expected newline
        "#]],
    );
}

/// Known limitation: the `)` in the block below closes the `(`, so nothing is reported,
/// where Dart Sass rejects `.b` as a media feature. Only the contents tell this from a
/// selector laid out over lines, and they are not read.
#[test]
fn a_closer_in_the_block_below_closes_an_unclosed_bracket() {
    check(
        "@media (a\n  .b\n    f: g(h))\n.x\n  y: z\n",
        expect![[r#"
        SOURCE_FILE@0..38
          MEDIA_RULE@0..27
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..27
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              IDENT@8..9 "a"
              WHITESPACE@9..12 "\n  "
              DOT@12..13 "."
              IDENT@13..14 "b"
              WHITESPACE@14..19 "\n    "
              IDENT@19..20 "f"
              COLON@20..21 ":"
              WHITESPACE@21..22 " "
              IDENT@22..23 "g"
              LPAREN@23..24 "("
              IDENT@24..25 "h"
              RPAREN@25..26 ")"
              RPAREN@26..27 ")"
          SEMICOLON@27..27 ""
          RULE_SET@27..37
            SELECTOR_LIST@27..30
              SELECTOR@27..30
                SIMPLE_SELECTOR@27..30
                  WHITESPACE@27..28 "\n"
                  DOT@28..29 "."
                  IDENT@29..30 "x"
            BLOCK@30..37
              LBRACE@30..30 ""
              DECLARATION@30..37
                PROPERTY@30..34
                  WHITESPACE@30..33 "\n  "
                  IDENT@33..34 "y"
                COLON@34..35 ":"
                VALUE@35..37
                  VALUE@35..37
                    WHITESPACE@35..36 " "
                    IDENT@36..37 "z"
                SEMICOLON@37..37 ""
              RBRACE@37..37 ""
          WHITESPACE@37..38 "\n"
    "#]],
    );
}

#[test]
fn a_stray_closer_of_another_kind_below_does_not_keep_a_bracket_open() {
    check(
        ".a:not(.b\n.c\n  d: e\n.f]\n",
        expect![[r#"
            SOURCE_FILE@0..24
              RULE_SET@0..9
                SELECTOR_LIST@0..9
                  SELECTOR@0..9
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    PSEUDO_SELECTOR@2..9
                      COLON@2..3 ":"
                      IDENT@3..6 "not"
                      LPAREN@6..7 "("
                      DOT@7..8 "."
                      IDENT@8..9 "b"
              SEMICOLON@9..9 ""
              RULE_SET@9..19
                SELECTOR_LIST@9..12
                  SELECTOR@9..12
                    SIMPLE_SELECTOR@9..12
                      WHITESPACE@9..10 "\n"
                      DOT@10..11 "."
                      IDENT@11..12 "c"
                BLOCK@12..19
                  LBRACE@12..12 ""
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
              RULE_SET@19..22
                SELECTOR_LIST@19..22
                  SELECTOR@19..22
                    SIMPLE_SELECTOR@19..22
                      WHITESPACE@19..20 "\n"
                      DOT@20..21 "."
                      IDENT@21..22 "f"
              ERROR@22..23
                RBRACKET@22..23 "]"
              SEMICOLON@23..23 ""
              WHITESPACE@23..24 "\n"
            error 9..9: expected RPAREN
            error 22..23: expected newline
        "#]],
    );
}

#[test]
fn an_unclosed_media_query_nested_in_a_rule_ends_at_its_own_line() {
    check(
        ".x\n  @media (a\n  .b\n    c: d\n.e\n  f: g\n",
        expect![[r#"
        SOURCE_FILE@0..39
          RULE_SET@0..28
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "x"
            BLOCK@2..28
              LBRACE@2..2 ""
              MEDIA_RULE@2..14
                WHITESPACE@2..5 "\n  "
                AT@5..6 "@"
                IDENT@6..11 "media"
                MEDIA_QUERY@11..14
                  WHITESPACE@11..12 " "
                  LPAREN@12..13 "("
                  IDENT@13..14 "a"
              SEMICOLON@14..14 ""
              RULE_SET@14..28
                SELECTOR_LIST@14..19
                  SELECTOR@14..19
                    SIMPLE_SELECTOR@14..19
                      WHITESPACE@14..17 "\n  "
                      DOT@17..18 "."
                      IDENT@18..19 "b"
                BLOCK@19..28
                  LBRACE@19..19 ""
                  DECLARATION@19..28
                    PROPERTY@19..25
                      WHITESPACE@19..24 "\n    "
                      IDENT@24..25 "c"
                    COLON@25..26 ":"
                    VALUE@26..28
                      VALUE@26..28
                        WHITESPACE@26..27 " "
                        IDENT@27..28 "d"
                    SEMICOLON@28..28 ""
                  RBRACE@28..28 ""
              RBRACE@28..28 ""
          RULE_SET@28..38
            SELECTOR_LIST@28..31
              SELECTOR@28..31
                SIMPLE_SELECTOR@28..31
                  WHITESPACE@28..29 "\n"
                  DOT@29..30 "."
                  IDENT@30..31 "e"
            BLOCK@31..38
              LBRACE@31..31 ""
              DECLARATION@31..38
                PROPERTY@31..35
                  WHITESPACE@31..34 "\n  "
                  IDENT@34..35 "f"
                COLON@35..36 ":"
                VALUE@36..38
                  VALUE@36..38
                    WHITESPACE@36..37 " "
                    IDENT@37..38 "g"
                SEMICOLON@38..38 ""
              RBRACE@38..38 ""
          WHITESPACE@38..39 "\n"
        error 14..14: expected RPAREN
    "#]],
    );
}

#[test]
fn an_unclosed_pseudo_class_ends_at_a_line_no_deeper_than_its_own() {
    check(
        ".a:not(.b\n.c\n  d: e\n.f\n  g: h\n",
        expect![[r#"
        SOURCE_FILE@0..30
          RULE_SET@0..9
            SELECTOR_LIST@0..9
              SELECTOR@0..9
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..9
                  COLON@2..3 ":"
                  IDENT@3..6 "not"
                  LPAREN@6..7 "("
                  DOT@7..8 "."
                  IDENT@8..9 "b"
          SEMICOLON@9..9 ""
          RULE_SET@9..19
            SELECTOR_LIST@9..12
              SELECTOR@9..12
                SIMPLE_SELECTOR@9..12
                  WHITESPACE@9..10 "\n"
                  DOT@10..11 "."
                  IDENT@11..12 "c"
            BLOCK@12..19
              LBRACE@12..12 ""
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
          RULE_SET@19..29
            SELECTOR_LIST@19..22
              SELECTOR@19..22
                SIMPLE_SELECTOR@19..22
                  WHITESPACE@19..20 "\n"
                  DOT@20..21 "."
                  IDENT@21..22 "f"
            BLOCK@22..29
              LBRACE@22..22 ""
              DECLARATION@22..29
                PROPERTY@22..26
                  WHITESPACE@22..25 "\n  "
                  IDENT@25..26 "g"
                COLON@26..27 ":"
                VALUE@27..29
                  VALUE@27..29
                    WHITESPACE@27..28 " "
                    IDENT@28..29 "h"
                SEMICOLON@29..29 ""
              RBRACE@29..29 ""
          WHITESPACE@29..30 "\n"
        error 9..9: expected RPAREN
    "#]],
    );
}

#[test]
fn an_unclosed_attribute_selector_ends_at_a_line_no_deeper_than_its_own() {
    check(
        ".x\n  .a[b\n  .c\n    d: e\n",
        expect![[r#"
        SOURCE_FILE@0..24
          RULE_SET@0..23
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "x"
            BLOCK@2..23
              LBRACE@2..2 ""
              RULE_SET@2..9
                SELECTOR_LIST@2..9
                  SELECTOR@2..9
                    SIMPLE_SELECTOR@2..7
                      WHITESPACE@2..5 "\n  "
                      DOT@5..6 "."
                      IDENT@6..7 "a"
                    ATTR_SELECTOR@7..9
                      LBRACKET@7..8 "["
                      IDENT@8..9 "b"
              SEMICOLON@9..9 ""
              RULE_SET@9..23
                SELECTOR_LIST@9..14
                  SELECTOR@9..14
                    SIMPLE_SELECTOR@9..14
                      WHITESPACE@9..12 "\n  "
                      DOT@12..13 "."
                      IDENT@13..14 "c"
                BLOCK@14..23
                  LBRACE@14..14 ""
                  DECLARATION@14..23
                    PROPERTY@14..20
                      WHITESPACE@14..19 "\n    "
                      IDENT@19..20 "d"
                    COLON@20..21 ":"
                    VALUE@21..23
                      VALUE@21..23
                        WHITESPACE@21..22 " "
                        IDENT@22..23 "e"
                    SEMICOLON@23..23 ""
                  RBRACE@23..23 ""
              RBRACE@23..23 ""
          WHITESPACE@23..24 "\n"
        error 9..9: expected RBRACKET
    "#]],
    );
}

#[test]
fn an_unclosed_paren_in_an_unknown_at_rule_ends_at_a_line_no_deeper_than_its_own() {
    check(
        "@foo (a\n.c\n  d: e\n",
        expect![[r#"
        SOURCE_FILE@0..18
          GENERIC_AT_RULE@0..7
            AT@0..1 "@"
            IDENT@1..4 "foo"
            WHITESPACE@4..5 " "
            LPAREN@5..6 "("
            IDENT@6..7 "a"
            SEMICOLON@7..7 ""
          RULE_SET@7..17
            SELECTOR_LIST@7..10
              SELECTOR@7..10
                SIMPLE_SELECTOR@7..10
                  WHITESPACE@7..8 "\n"
                  DOT@8..9 "."
                  IDENT@9..10 "c"
            BLOCK@10..17
              LBRACE@10..10 ""
              DECLARATION@10..17
                PROPERTY@10..14
                  WHITESPACE@10..13 "\n  "
                  IDENT@13..14 "d"
                COLON@14..15 ":"
                VALUE@15..17
                  VALUE@15..17
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "e"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          WHITESPACE@17..18 "\n"
        error 7..7: expected RPAREN
    "#]],
    );
}

#[test]
fn an_unclosed_paren_in_a_media_query_ends_at_a_line_no_deeper_than_its_own() {
    check(
        "@media (a\n.c\n  d: e\n",
        expect![[r#"
        SOURCE_FILE@0..20
          MEDIA_RULE@0..9
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..9
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              IDENT@8..9 "a"
          SEMICOLON@9..9 ""
          RULE_SET@9..19
            SELECTOR_LIST@9..12
              SELECTOR@9..12
                SIMPLE_SELECTOR@9..12
                  WHITESPACE@9..10 "\n"
                  DOT@10..11 "."
                  IDENT@11..12 "c"
            BLOCK@12..19
              LBRACE@12..12 ""
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
        error 9..9: expected RPAREN
    "#]],
    );
}

#[test]
fn an_unclosed_paren_in_a_condition_is_one_error_and_spares_the_next_rule() {
    for source in [
        "@media screen and (a\n.c\n  d: e\n",
        "@supports (a: b\n.c\n  d: e\n",
        "@container card (width > 1px\n.c\n  d: e\n",
        "@scope (.a) to (.b\n.c\n  d: e\n",
        ".x\n  @media (a\n  .c\n    d: e\n",
    ] {
        let tree = render(parse_sass, source);
        assert_eq!(error_count(source), 1, "{source:?}:\n{tree}");
        assert!(
            tree.contains("error ") && tree.contains("expected RPAREN"),
            "{tree}"
        );
        assert_eq!(
            tree.matches("RULE_SET@").count(),
            1 + usize::from(source.starts_with('.')),
            "{tree}"
        );
        assert_eq!(tree.matches("DECLARATION@").count(), 1, "{tree}");
    }
}

#[test]
fn an_unclosed_selector_bracket_at_the_end_of_the_input_is_reported() {
    check(
        ".a:not(.b",
        expect![[r#"
        SOURCE_FILE@0..9
          RULE_SET@0..9
            SELECTOR_LIST@0..9
              SELECTOR@0..9
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..9
                  COLON@2..3 ":"
                  IDENT@3..6 "not"
                  LPAREN@6..7 "("
                  DOT@7..8 "."
                  IDENT@8..9 "b"
          SEMICOLON@9..9 ""
        error 9..9: expected RPAREN
    "#]],
    );
}

#[test]
fn each_unclosed_selector_bracket_is_one_error() {
    check(
        ".a:not(.b\n.c[d\n.e\n  f: g\n",
        expect![[r#"
        SOURCE_FILE@0..25
          RULE_SET@0..9
            SELECTOR_LIST@0..9
              SELECTOR@0..9
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..9
                  COLON@2..3 ":"
                  IDENT@3..6 "not"
                  LPAREN@6..7 "("
                  DOT@7..8 "."
                  IDENT@8..9 "b"
          SEMICOLON@9..9 ""
          RULE_SET@9..14
            SELECTOR_LIST@9..14
              SELECTOR@9..14
                SIMPLE_SELECTOR@9..12
                  WHITESPACE@9..10 "\n"
                  DOT@10..11 "."
                  IDENT@11..12 "c"
                ATTR_SELECTOR@12..14
                  LBRACKET@12..13 "["
                  IDENT@13..14 "d"
          SEMICOLON@14..14 ""
          RULE_SET@14..24
            SELECTOR_LIST@14..17
              SELECTOR@14..17
                SIMPLE_SELECTOR@14..17
                  WHITESPACE@14..15 "\n"
                  DOT@15..16 "."
                  IDENT@16..17 "e"
            BLOCK@17..24
              LBRACE@17..17 ""
              DECLARATION@17..24
                PROPERTY@17..21
                  WHITESPACE@17..20 "\n  "
                  IDENT@20..21 "f"
                COLON@21..22 ":"
                VALUE@22..24
                  VALUE@22..24
                    WHITESPACE@22..23 " "
                    IDENT@23..24 "g"
                SEMICOLON@24..24 ""
              RBRACE@24..24 ""
          WHITESPACE@24..25 "\n"
        error 9..9: expected RPAREN
        error 14..14: expected RBRACKET
    "#]],
    );
}

#[test]
fn a_bracket_closed_on_a_later_line_is_not_cut() {
    for source in [
        ".a:not(.b,\n.c)\n  d: e\n",
        ".a:not(\n  .b\n)\n  c: d\n",
        "@foo (a,\nb)\n",
        "@media screen and (\n  min-width: 1px)\n  .a\n    b: c\n",
        "@media (min-width:\n1px)\n  .a\n    b: c\n",
        "@supports (a:\nb)\n  .c\n    d: e\n",
        ".a:not(.b\n.c:not(.d)\n.e)\n  f: g\n",
        ".a:is(.b,\n.c,\n.d)\n  e: f\n",
        ".a:not(.b,\n.c,\n  .d)\n  e: f\n",
        "@foo (a,\nb,\nc)\n  d: e\n",
    ] {
        let tree = render(parse_sass, source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

/// A comment on a continued line is lexed again once the parser knows the line goes on,
/// after the first bracket already asked which brackets are unclosed: the answer follows
/// the new tokens, whether the comment hid an unclosed bracket or a closed one.
#[test]
fn unclosed_brackets_are_known_after_a_comment_is_lexed_again() {
    let rest = ".c\n  d: e\n.f:not(.g\n.h\n  i: j\n";
    for (lines, errors) in [
        // Lexed again up to the line after the comment, then as before.
        ("    2\n", 1),
        ("    (2)\n", 1),
        // Lexed again to the end of the input.
        ("    (2\n", 3),
    ] {
        let source = format!(".a:not(.b)\n  x: y\n$a: 1 +\n  // c\n{lines}{rest}");
        let tree = render(parse_sass, &source);
        assert_eq!(error_count(&source), errors, "{source:?}:\n{tree}");
        assert_eq!(tree.matches("RULE_SET@").count(), 4, "{tree}");
        assert_eq!(tree.matches("DECLARATION@").count(), 3, "{tree}");
    }
}

#[test]
fn many_unclosed_brackets_stay_proportional() {
    let one_line = ".a:not(".repeat(5_000);
    let many_lines = ".a:not(.b\n".repeat(5_000);
    let nested = format!("{}\n.c\n  d: e\n", "@media (a ".repeat(5_000));
    assert_eq!(error_count(&one_line), 1);
    assert_eq!(error_count(&many_lines), 5_000);
    assert_eq!(error_count(&nested), 1);
}

#[test]
fn an_error_inside_a_bracket_that_closes_changes_nothing() {
    // The error makes the bracket stop at a line no deeper than the one it opened on, but
    // it closes before any such line.
    check_pinned(".a\n  b: fn(1, !\n    2)\n  c: d\n");
}

#[test]
fn unclosed_brackets_inside_a_closed_one_unwind_with_it() {
    check(
        ".a\n  b: f(g(1 ]\n.c\n  d: e\n",
        expect![[r#"
            SOURCE_FILE@0..26
              RULE_SET@0..15
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..15
                  LBRACE@2..2 ""
                  DECLARATION@2..15
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..15
                      FUNCTION_CALL@7..13
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "f"
                        ARG_LIST@9..13
                          LPAREN@9..10 "("
                          ARG@10..13
                            FUNCTION_CALL@10..13
                              IDENT@10..11 "g"
                              ARG_LIST@11..13
                                LPAREN@11..12 "("
                                ARG@12..13
                                  NUMBER_LITERAL@12..13
                                    NUMBER@12..13 "1"
                      ERROR@13..15
                        WHITESPACE@13..14 " "
                        RBRACKET@14..15 "]"
                    SEMICOLON@15..15 ""
                  RBRACE@15..15 ""
              RULE_SET@15..25
                SELECTOR_LIST@15..18
                  SELECTOR@15..18
                    SIMPLE_SELECTOR@15..18
                      WHITESPACE@15..16 "\n"
                      DOT@16..17 "."
                      IDENT@17..18 "c"
                BLOCK@18..25
                  LBRACE@18..18 ""
                  DECLARATION@18..25
                    PROPERTY@18..22
                      WHITESPACE@18..21 "\n  "
                      IDENT@21..22 "d"
                    COLON@22..23 ":"
                    VALUE@23..25
                      VALUE@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "e"
                    SEMICOLON@25..25 ""
                  RBRACE@25..25 ""
              WHITESPACE@25..26 "\n"
            error 14..15: expected RPAREN
            error 14..15: expected RPAREN
        "#]],
    );
}

#[test]
fn a_document_prelude_ends_with_its_line() {
    check(
        "@-moz-document url-prefix(http://x/)\n  a\n    b: domain($x)\n",
        expect![[r#"
            SOURCE_FILE@0..59
              GENERIC_AT_RULE@0..58
                AT@0..1 "@"
                IDENT@1..14 "-moz-document"
                WHITESPACE@14..15 " "
                IDENT@15..25 "url-prefix"
                LPAREN@25..26 "("
                URL_CONTENTS@26..35 "http://x/"
                RPAREN@35..36 ")"
                BLOCK@36..58
                  LBRACE@36..36 ""
                  RULE_SET@36..58
                    SELECTOR_LIST@36..40
                      SELECTOR@36..40
                        SIMPLE_SELECTOR@36..40
                          WHITESPACE@36..39 "\n  "
                          IDENT@39..40 "a"
                    BLOCK@40..58
                      LBRACE@40..40 ""
                      DECLARATION@40..58
                        PROPERTY@40..46
                          WHITESPACE@40..45 "\n    "
                          IDENT@45..46 "b"
                        COLON@46..47 ":"
                        VALUE@47..58
                          FUNCTION_CALL@47..58
                            WHITESPACE@47..48 " "
                            IDENT@48..54 "domain"
                            ARG_LIST@54..58
                              LPAREN@54..55 "("
                              ARG@55..57
                                VARIABLE_REF@55..57
                                  DOLLAR@55..56 "$"
                                  IDENT@56..57 "x"
                              RPAREN@57..58 ")"
                        SEMICOLON@58..58 ""
                      RBRACE@58..58 ""
                  RBRACE@58..58 ""
              WHITESPACE@58..59 "\n"
        "#]],
    );
}
