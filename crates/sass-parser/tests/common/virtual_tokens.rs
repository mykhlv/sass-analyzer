//! Cross-check of `Event::Virtual` against the zero-width tokens of the legacy indented
//! lexer.
//!
//! Included with `#[path]` by the test binaries that run it, rather than through
//! `common/mod.rs`, so that binaries which do not are not left with dead code.

use std::fmt::Write;

use sass_parser::event::Event;
use sass_parser::input::Input;
use sass_parser::parser::{Layout, Parser};
use sass_parser::syntax::{SyntaxNode, debug_tree};

/// Parses `source` with the legacy pipeline, then rebuilds the tree from the same events
/// over the same tokens minus the zero-width ones: each of those becomes an
/// `Event::Virtual` placed where legacy put it among the trivia. The two trees and error
/// lists must be identical.
///
/// The tokens are legacy's rather than `Input::from_source_indented`'s so that this
/// checks the placement alone; whether the two lexers agree is the line-facts check.
///
/// Returns both renderings on disagreement, or `None`.
pub fn virtual_token_mismatch(source: &str) -> Option<String> {
    let legacy_tokens = sass_parser::sass_lexer::sass_tokenize(source);
    let mut parser = Parser::with_layout(
        Input::from_tokens(&legacy_tokens),
        source,
        Layout::LegacyIndented,
    );
    sass_parser::grammar::source_file(&mut parser);
    let (events, errors, legacy, _) = parser.finish();
    let expected = render(sass_parser::build_tree(
        events.clone(),
        &errors,
        &legacy,
        source,
    ));

    let real_tokens: Vec<_> = legacy_tokens
        .into_iter()
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let input = Input::from_tokens(&real_tokens);
    let mut legacy_pos = 0;
    let events: Vec<Event> = events
        .into_iter()
        .map(|event| match event {
            Event::Token { kind, range } => {
                legacy_pos += 1;
                if range.is_empty() {
                    // After the trivia legacy put before it, which ends where the trivia
                    // of the token after it starts.
                    #[allow(clippy::cast_possible_truncation)]
                    let trivia = legacy.trivia_start_index(legacy_pos) as u32;
                    Event::Virtual { kind, trivia }
                } else {
                    event
                }
            }
            other => other,
        })
        .collect();
    let actual = render(sass_parser::build_tree(events, &errors, &input, source));

    (actual != expected)
        .then(|| format!("trees differ\n--- legacy\n{expected}\n--- Event::Virtual\n{actual}"))
}

fn render(
    (green, errors): (
        rowan::GreenNode,
        Vec<(String, sass_parser::text_range::TextRange)>,
    ),
) -> String {
    let mut buf = debug_tree(&SyntaxNode::new_root(green));
    for (msg, range) in &errors {
        let _ = writeln!(buf, "error {range:?}: {msg}");
    }
    buf
}
