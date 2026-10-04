//! `parse_sass_layout` against `parse_sass_legacy`: the two indented frontends must give
//! the same tree and errors, except for the known kinds of difference, where the layout
//! frontend fixes a defect of legacy. Any other difference crashes, for triage: a defect
//! of legacy that the layout frontend fixes too, or a regression of the layout frontend.
//!
//! Run it for a fixed time while the two frontends coexist, not as a standing job:
//! `cargo +nightly fuzz run fuzz_sass_layout_differential -- -max_total_time=600`.

#![no_main]

use std::fmt::Write;

use libfuzzer_sys::fuzz_target;
use sass_parser::input::Input;
use sass_parser::lexer::IndentedLexer;
use sass_parser::parser::{Layout, Parser};
use sass_parser::syntax::{GreenNode, SyntaxNode, debug_tree};
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;

type Parse = fn(&str) -> (GreenNode, Vec<(String, TextRange)>);

fn render(parse: Parse, source: &str) -> String {
    let (green, errors) = parse(source);
    let mut out = debug_tree(&SyntaxNode::new_root(green));
    for (message, range) in &errors {
        let _ = writeln!(out, "error {range:?}: {message}");
    }
    out
}

fn differs(source: &str) -> Option<(String, String)> {
    let legacy = render(sass_parser::parse_sass_legacy, source);
    let layout = render(sass_parser::parse_sass_layout, source);
    (legacy != layout).then_some((legacy, layout))
}

/// A difference the layout frontend makes on purpose.
fn known_difference(source: &str, legacy: &str, layout: &str) -> bool {
    // Indentation at the end of the input opens no block.
    let trimmed = source.trim_end_matches([' ', '\t']);
    if trimmed.len() < source.len()
        && trimmed.ends_with(['\n', '\r', '\x0c'])
        && differs(trimmed).is_none()
    {
        return true;
    }
    // Diagnostics legacy never reported.
    if [
        "unterminated comment",
        "inconsistent indentation",
        "may not be mixed",
        "expected tabs",
        "expected spaces",
    ]
    .iter()
    .any(|message| layout.contains(message))
    {
        return true;
    }
    recovers_from_bracket_errors(source)
        || silent_comment_crosses_a_blank_line(source)
        || operator_ends_a_line_legacy_ended(source, legacy)
        || stray_colon_ends_a_variable_line(source)
}

/// Legacy continues any line that starts with `$` and ends with a colon; the layout
/// frontend only the one after a variable's name, `$name:`.
fn stray_colon_ends_a_variable_line(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_matches([' ', '\t', '\r', '\x0c']);
        line.strip_prefix('$')
            .and_then(|rest| rest.strip_suffix(':'))
            .is_some_and(|name| {
                !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            })
    })
}

/// Legacy continues a line ending in `*`, `and`, `or` or `not` only in a statement it
/// reads as an expression, one with a colon not followed by an identifier; the layout
/// frontend continues it wherever the grammar reads the token as an operator. Legacy
/// then reports the missing operand at the end of the line.
fn operator_ends_a_line_legacy_ended(source: &str, legacy: &str) -> bool {
    legacy
        .lines()
        .filter(|line| line.ends_with(": expected expression"))
        .filter_map(|line| {
            let range = line.strip_prefix("error ")?.split(':').next()?;
            let (start, _) = range.split_once("..")?;
            start.parse::<usize>().ok()
        })
        .any(|offset| {
            let before = source
                .get(..offset)
                .unwrap_or("")
                .trim_end_matches([' ', '\t']);
            ["*", "and", "or", "not"]
                .iter()
                .any(|operator| before.ends_with(operator))
        })
}

/// An error inside a bracket ends it early, and the lines after an unclosed one count.
fn recovers_from_bracket_errors(source: &str) -> bool {
    let input = Input::from_source_indented(source);
    let mut parser = Parser::with_layout(input, source, Layout::Indented);
    sass_parser::grammar::source_file(&mut parser);
    let recovered = parser.recovered_from_bracket_errors();
    let _ = parser.finish();
    recovered
}

/// A silent comment reaches across a blank line onto the deeper lines after it.
fn silent_comment_crosses_a_blank_line(source: &str) -> bool {
    let mut legacy = IndentedLexer::new(source);
    let mut layout = IndentedLexer::crossing_blank_lines(source);
    loop {
        let token = legacy.next_token();
        if token != layout.next_token() {
            return true;
        }
        if token.0 == SyntaxKind::EOF {
            return false;
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    if let Some((legacy, layout)) = differs(source)
        && !known_difference(source, &legacy, &layout)
    {
        panic!(
            "parse_sass_layout differs from parse_sass_legacy on {source:?}\n\
             --- legacy\n{legacy}--- layout\n{layout}"
        );
    }
});
