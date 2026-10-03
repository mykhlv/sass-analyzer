//! Cross-check of `Input`'s indented layout facts and comment extents against the legacy
//! indented lexer.
//!
//! Included with `#[path]` by the test binaries that run it, rather than through
//! `common/mod.rs`, so that binaries which do not are not left with dead code.

use std::collections::HashMap;

use sass_parser::input::{Input, LineBreak};
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::{TextRange, TextSize};

/// Every newline that the legacy lexer measures indentation at, checked against the
/// facts of `Input::from_source_indented`: the line break at that whitespace must exist
/// and have the same column, and when it is the last break before a token (or before
/// EOF), `line_start` there must report that column too. Its comments must be exactly
/// those of the legacy lexer.
///
/// Returns one line per disagreement; empty means the facts reproduce legacy here.
pub fn line_fact_mismatches(source: &str) -> Vec<String> {
    let input = Input::from_source_indented(source);
    let mut mismatches = comment_extent_mismatches(source, &input);

    // Whitespace offset → (column of its break, column `line_start` gives the token it
    // precedes, whether it is that token's last break).
    let mut facts: HashMap<TextSize, (u32, Option<u32>, bool)> = HashMap::new();
    for pos in 0..=input.len() {
        let trivia = if pos < input.len() {
            input.trivia_before(pos)
        } else {
            input.trailing_trivia()
        };
        let newline_ws: Vec<TextSize> = trivia
            .iter()
            .filter(|(kind, range)| {
                *kind == SyntaxKind::WHITESPACE && source[*range].contains(['\n', '\r', '\x0c'])
            })
            .map(|(_, range)| range.start())
            .collect();
        let breaks = input.line_breaks_before(pos);
        if newline_ws.len() != breaks.len() {
            mismatches.push(format!(
                "token {pos}: {} newline-bearing whitespace tokens but {} line breaks",
                newline_ws.len(),
                breaks.len(),
            ));
            continue;
        }
        let line_start = input.line_start(pos).map(LineBreak::column);
        for (i, (offset, line_break)) in newline_ws.into_iter().zip(breaks).enumerate() {
            let is_last = i + 1 == breaks.len();
            facts.insert(offset, (line_break.column(), line_start, is_last));
        }
    }

    for (offset, legacy) in sass_parser::sass_lexer::newline_indents(source) {
        match facts.get(&offset) {
            None => mismatches.push(format!(
                "@{offset:?}: no line break, legacy measured {legacy}"
            )),
            Some(&(column, _, _)) if column != legacy => mismatches.push(format!(
                "@{offset:?}: column {column}, legacy measured {legacy}"
            )),
            Some(&(_, line_start, true)) if line_start != Some(legacy) => {
                mismatches.push(format!(
                    "@{offset:?}: line_start {line_start:?}, legacy measured {legacy}"
                ));
            }
            Some(_) => {}
        }
    }
    mismatches
}

/// The comments of the legacy lexer's token stream against those of `input`. They can
/// differ only where a comment is the first token on a line that continues the one
/// above (`.a,` / `// c` / `  .b`): legacy knows that the comment does not start a
/// statement, the layout alone does not.
fn comment_extent_mismatches(source: &str, input: &Input) -> Vec<String> {
    let is_comment = |kind: SyntaxKind, text: &str| {
        matches!(
            kind,
            SyntaxKind::SINGLE_LINE_COMMENT | SyntaxKind::MULTI_LINE_COMMENT
        ) || (kind == SyntaxKind::ERROR && text.starts_with("/*"))
    };
    let mut layout = Vec::new();
    for pos in 0..=input.len() {
        let trivia = if pos < input.len() {
            input.trivia_before(pos)
        } else {
            input.trailing_trivia()
        };
        for &(kind, range) in trivia {
            if is_comment(kind, &source[range]) {
                layout.push(range);
            }
        }
        if pos < input.len() && is_comment(input.kind(pos), &source[input.range(pos)]) {
            layout.push(input.range(pos));
        }
    }
    let mut legacy = Vec::new();
    let mut offset = 0u32;
    for (kind, text) in sass_parser::sass_lexer::sass_tokenize(source) {
        #[allow(clippy::cast_possible_truncation)]
        let len = text.len() as u32;
        if is_comment(kind, text) {
            legacy.push(TextRange::new(
                TextSize::from(offset),
                TextSize::from(offset + len),
            ));
        }
        offset += len;
    }
    let mut mismatches = Vec::new();
    for range in &legacy {
        if !layout.contains(range) {
            mismatches.push(format!("legacy comment {range:?} {:?}", &source[*range]));
        }
    }
    for range in &layout {
        if !legacy.contains(range) {
            mismatches.push(format!("layout comment {range:?} {:?}", &source[*range]));
        }
    }
    mismatches
}
