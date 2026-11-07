//! Folding and selection ranges over indented-Sass (`.sass`) documents.
//!
//! Indented trees carry zero-width `{`/`}`/`;` tokens instead of real ones, so a
//! change in where those tokens land moves node ranges without any text changing.
//! These snapshots pin the ranges the editor sees, so such a move shows up here.

use std::fmt::Write;

use expect_test::{Expect, expect};

use super::folding_ranges::get_folds;
use super::selection_ranges::{flatten_selection_range, get_selection_ranges};
use super::*;

const NESTED_RULE: &str = "\
.nav
  color: red
  .item
    margin: 0
    &:hover
      color: blue
.footer
  padding: 0
";

const CONTINUED_HEADER: &str = "\
@each $name,
  $glyph in (a b)
  .a
    color: red
@include button($bg,
  $border)
";

const COMMENT_ONLY_LINES: &str = "\
// leading comment
// second line
.a
  // only a comment
  color: red
@mixin empty
  // comment-only body
.b
  /* loud */
  margin: 0
";

// No trailing newline, so `.eof` and `.deep` close together at EOF.
const FOLD_EDGES: &str = "\
.trailing-comment
  color: red
  // last body line is a comment

.blank-lines-after
  margin: 0


@if $a
  b: 1
@else
  c: 2
/* loud comment
   closed by the dedent
.eof
  .deep
    x: 1";

/// Byte offset of an LSP position. The inputs are ASCII, so a UTF-16 column is a
/// byte column.
fn offset(source: &str, pos: &Value) -> usize {
    let line = usize::try_from(pos["line"].as_u64().unwrap()).unwrap();
    let character = usize::try_from(pos["character"].as_u64().unwrap()).unwrap();
    let line_start: usize = source.split_inclusive('\n').take(line).map(str::len).sum();
    line_start + character
}

fn line_text<'a>(source: &'a str, line: &Value) -> &'a str {
    let line = usize::try_from(line.as_u64().unwrap()).unwrap();
    source.lines().nth(line).unwrap_or("")
}

fn render_folds(source: &str, folds: &[Value]) -> String {
    let mut out = String::new();
    for fold in folds {
        let kind = fold["kind"].as_str().unwrap_or("block");
        writeln!(
            out,
            "{}..={} {kind}: {:?} .. {:?}",
            fold["startLine"],
            fold["endLine"],
            line_text(source, &fold["startLine"]),
            line_text(source, &fold["endLine"]),
        )
        .unwrap();
    }
    out
}

fn render_selection(source: &str, result: &Value) -> String {
    let mut out = String::new();
    for range in flatten_selection_range(result) {
        let (start, end) = (&range["start"], &range["end"]);
        let text = &source[offset(source, start)..offset(source, end)];
        let shown = if text.len() <= 40 {
            format!("{text:?}")
        } else {
            format!("<{} bytes>", text.len())
        };
        writeln!(
            out,
            "{}:{}-{}:{} {shown}",
            start["line"], start["character"], end["line"], end["character"],
        )
        .unwrap();
    }
    out
}

async fn check_folds(uri: &str, source: &str, id: u64, expect: &Expect) {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;
    let folds = get_folds(&mut reader, &mut writer, uri, source, id).await;
    expect.assert_eq(&render_folds(source, &folds));
}

async fn check_selection(
    uri: &str,
    source: &str,
    id: u64,
    positions: Vec<(u32, u32)>,
    expect: &Expect,
) {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;
    let results =
        get_selection_ranges(&mut reader, &mut writer, uri, source, id, positions.clone()).await;
    assert_eq!(results.len(), positions.len(), "one result per position");
    let mut out = String::new();
    for ((line, character), result) in positions.iter().zip(&results) {
        writeln!(out, "@ {line}:{character}").unwrap();
        out.push_str(&render_selection(source, result));
    }
    expect.assert_eq(&out);
}

#[tokio::test]
async fn sass_folding_nested_rule() {
    check_folds(
        "file:///fold_nested.sass",
        NESTED_RULE,
        700,
        &expect![[r#"
            0..=5 block: ".nav" .. "      color: blue"
            2..=5 block: "  .item" .. "      color: blue"
            4..=5 block: "    &:hover" .. "      color: blue"
            6..=7 block: ".footer" .. "  padding: 0"
        "#]],
    )
    .await;
}

#[tokio::test]
async fn sass_folding_continued_header() {
    check_folds(
        "file:///fold_header.sass",
        CONTINUED_HEADER,
        701,
        &expect![[r#"
            0..=3 block: "@each $name," .. "    color: red"
            2..=3 block: "  .a" .. "    color: red"
        "#]],
    )
    .await;
}

#[tokio::test]
async fn sass_folding_comment_only_lines() {
    check_folds(
        "file:///fold_comments.sass",
        COMMENT_ONLY_LINES,
        702,
        &expect![[r#"
            2..=4 block: ".a" .. "  color: red"
            5..=6 block: "@mixin empty" .. "  // comment-only body"
            7..=9 block: ".b" .. "  margin: 0"
            0..=1 comment: "// leading comment" .. "// second line"
        "#]],
    )
    .await;
}

#[tokio::test]
async fn sass_folding_edges() {
    check_folds(
        "file:///fold_edges.sass",
        FOLD_EDGES,
        703,
        &expect![[r#"
            0..=2 block: ".trailing-comment" .. "  // last body line is a comment"
            4..=5 block: ".blank-lines-after" .. "  margin: 0"
            8..=11 block: "@if $a" .. "  c: 2"
            10..=11 block: "@else" .. "  c: 2"
            14..=16 block: ".eof" .. "    x: 1"
            12..=13 comment: "/* loud comment" .. "   closed by the dedent"
            15..=16 block: "  .deep" .. "    x: 1"
        "#]],
    )
    .await;
}

#[tokio::test]
async fn sass_selection_nested_rule() {
    check_selection(
        "file:///sel_nested.sass",
        NESTED_RULE,
        710,
        // "blue" in the innermost rule, then the `.footer` selector.
        vec![(5, 13), (6, 2)],
        &expect![[r#"
            @ 5:13
            5:13-5:17 "blue"
            5:12-5:17 " blue"
            4:11-5:17 "\n      color: blue"
            3:13-5:17 "\n    &:hover\n      color: blue"
            2:7-5:17 <44 bytes>
            1:12-5:17 <52 bytes>
            0:4-5:17 <65 bytes>
            0:0-5:17 <69 bytes>
            0:0-8:0 <91 bytes>
            @ 6:2
            6:1-6:7 "footer"
            5:17-6:7 "\n.footer"
            5:17-7:12 "\n.footer\n  padding: 0"
            0:0-8:0 <91 bytes>
        "#]],
    )
    .await;
}

#[tokio::test]
async fn sass_selection_continued_header() {
    check_selection(
        "file:///sel_header.sass",
        CONTINUED_HEADER,
        711,
        // `glyph` on the header's second line, `color` in the body, `$border` in
        // the continued argument list.
        vec![(1, 3), (3, 5), (5, 4)],
        &expect![[r#"
            @ 1:3
            1:3-1:8 "glyph"
            0:0-3:14 <50 bytes>
            0:0-6:0 <83 bytes>
            @ 3:5
            3:4-3:9 "color"
            2:4-3:9 "\n    color"
            2:4-3:14 "\n    color: red"
            1:17-3:14 "\n  .a\n    color: red"
            0:0-3:14 <50 bytes>
            0:0-6:0 <83 bytes>
            @ 5:4
            5:3-5:9 "border"
            4:20-5:9 "\n  $border"
            4:15-5:10 "($bg,\n  $border)"
            3:14-5:10 "\n@include button($bg,\n  $border)"
            0:0-6:0 <83 bytes>
        "#]],
    )
    .await;
}

#[tokio::test]
async fn sass_selection_comment_only_lines() {
    check_selection(
        "file:///sel_comments.sass",
        COMMENT_ONLY_LINES,
        712,
        // The comment line before a declaration, the declaration after it, and
        // the comment that is the mixin's whole body.
        vec![(3, 6), (4, 3), (6, 6)],
        &expect![[r#"
            @ 3:6
            3:2-3:19 "// only a comment"
            2:2-4:7 "\n  // only a comment\n  color"
            2:2-4:12 "\n  // only a comment\n  color: red"
            0:0-4:12 <69 bytes>
            0:0-10:0 <134 bytes>
            @ 4:3
            4:2-4:7 "color"
            2:2-4:7 "\n  // only a comment\n  color"
            2:2-4:12 "\n  // only a comment\n  color: red"
            0:0-4:12 <69 bytes>
            0:0-10:0 <134 bytes>
            @ 6:6
            6:2-6:22 "// comment-only body"
            5:12-6:22 "\n  // comment-only body"
            4:12-6:22 "\n@mixin empty\n  // comment-only body"
            0:0-10:0 <134 bytes>
        "#]],
    )
    .await;
}
