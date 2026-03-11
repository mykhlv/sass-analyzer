//! Hand-written, lossless SCSS parser following rust-analyzer's architecture.
//!
//! Produces a lossless concrete syntax tree (CST) via [rowan], where every byte
//! of the input is preserved. The tree is built in two phases: an events-based
//! parser emits `Event`s, and a bridge converts them into a rowan green tree.
//!
//! # Quick start
//!
//! ```
//! let source = "$color: #fff;\n.btn { color: $color; }";
//! let (green, errors) = sass_parser::parse(source);
//!
//! // The tree round-trips losslessly
//! let root = sass_parser::syntax::SyntaxNode::new_root(green);
//! assert_eq!(root.text().to_string(), source);
//! assert!(errors.is_empty());
//! ```
//!
//! # Key types
//!
//! - [`parse()`] — entry point, returns a green tree + diagnostics
//! - [`syntax_kind::SyntaxKind`] — all node and token kinds
//! - [`syntax::SyntaxNode`] / [`syntax::SyntaxToken`] — typed tree accessors
//! - [`line_index::LineIndex`] — byte offset ↔ line/column mapping
//! - [`resolver::ModuleResolver`] — `@use` / `@forward` / `@import` resolution

#![warn(missing_docs)]

// ── Public API ──────────────────────────────────────────────────────

/// All node and token kinds in the SCSS grammar.
pub mod syntax_kind;
/// Re-exports of rowan's `TextRange` / `TextSize` for span manipulation.
pub mod text_range;

/// Typed AST wrappers generated from `sass.ungram`.
pub mod ast;
/// Import statement extraction from the CST.
pub mod imports;
/// Byte offset ↔ line/column mapping.
pub mod line_index;
/// Incremental reparsing support.
pub mod reparse;
/// Module resolution for `@use` / `@forward` / `@import`.
pub mod resolver;
/// Rowan-backed concrete syntax tree types (`SyntaxNode`, `SyntaxToken`).
pub mod syntax;
/// Virtual filesystem abstraction for resolver tests and embedding.
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
