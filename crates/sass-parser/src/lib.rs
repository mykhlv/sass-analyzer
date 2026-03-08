pub mod syntax_kind;
pub mod text_range;
pub mod token_set;

pub mod event;
pub mod input;
pub mod lexer;
pub mod parser;
pub mod syntax;

pub mod ast;
pub mod grammar;
pub mod line_index;

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
