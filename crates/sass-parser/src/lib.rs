// ── Public API ──────────────────────────────────────────────────────
pub mod syntax_kind;
pub mod text_range;

pub mod ast;
pub mod imports;
pub mod line_index;
pub mod reparse;
pub mod resolver;
pub mod syntax;
pub mod vfs;

// ── Internal modules ────────────────────────────────────────────────
// These are implementation details. They are `pub` only for benchmarks
// and integration tests; downstream crates should not depend on them.
#[doc(hidden)]
pub mod event;
#[doc(hidden)]
pub mod grammar;
#[doc(hidden)]
pub mod input;
#[doc(hidden)]
pub mod lexer;
#[doc(hidden)]
pub mod parser;
#[doc(hidden)]
pub mod token_set;

mod bridge;
pub use bridge::build_tree;

use text_range::TextRange;

/// Parse SCSS source into a rowan green tree + diagnostics.
pub fn parse(source: &str) -> (rowan::GreenNode, Vec<(String, TextRange)>) {
    let input = input::Input::from_source(source);
    let mut parser = parser::Parser::new(input, source);
    grammar::source_file(&mut parser);
    let (events, errors, input, src) = parser.finish();
    build_tree(events, &errors, &input, src)
}
