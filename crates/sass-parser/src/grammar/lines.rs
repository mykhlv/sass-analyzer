//! Where a statement goes on across a line break in the indented syntax.
//!
//! Each grammar position that continues a statement onto the next line calls
//! [`Parser::continue_line`] (or its comma form) itself. The tables here are the other
//! half: how each at-rule's header, and a variable's value, may go on on a line indented
//! beneath it, which the at-rule function declares with [`Parser::continue_header`]
//! when it has read the rule's name.
//!
//! Forms marked `lenient:` are accepted here although Dart Sass rejects them; they are
//! kept, for now, so that the indented frontend does not change what it accepts while it
//! is being rebuilt.

use crate::parser::{HeaderLines, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

/// The tokens that start an expression on the next line of a header that takes one.
#[rustfmt::skip]
const EXPR_STARTS: TokenSet = TokenSet::new(&[
    LPAREN, COMMA, QUOTED_STRING, STRING_START, NUMBER,
]);

/// Right after the name, a variable is the expression: `@if` / `  $a`.
#[rustfmt::skip]
const EXPR_BARE_STARTS: TokenSet = TokenSet::new(&[
    LPAREN, COMMA, QUOTED_STRING, STRING_START, NUMBER, DOLLAR,
]);

#[rustfmt::skip]
const EXPR_WORDS: &[&str] = &[
    // lenient: Dart rejects `@if $x` / `  and $y` — "expected selector."
    "and", "or", "not",
    "if",
];

pub(crate) const IF: HeaderLines = HeaderLines {
    starts: EXPR_STARTS,
    words: EXPR_WORDS,
    bare_starts: EXPR_BARE_STARTS,
    bare_words: EXPR_WORDS,
    end_words: EXPR_WORDS,
    name_continues: false,
};

/// `@while`, `@return`, `@debug`, `@warn` and `@error`: a bare name leaves the header
/// unfinished.
pub(crate) const EXPRESSION: HeaderLines = HeaderLines {
    name_continues: true,
    ..IF
};

#[rustfmt::skip]
const FOR_WORDS: &[&str] = &["from", "through", "to", "and", "or", "not", "if"];

pub(crate) const FOR: HeaderLines = HeaderLines {
    words: FOR_WORDS,
    bare_words: FOR_WORDS,
    end_words: FOR_WORDS,
    ..EXPRESSION
};

const EACH_WORDS: &[&str] = &["in", "and", "or", "not", "if"];

pub(crate) const EACH: HeaderLines = HeaderLines {
    words: EACH_WORDS,
    bare_words: EACH_WORDS,
    end_words: EACH_WORDS,
    ..EXPRESSION
};

/// A bare `@else` is a complete header: what is indented beneath it is its body, except
/// for the `if` of `@else if`.
pub(crate) const ELSE: HeaderLines = HeaderLines {
    bare_starts: TokenSet::EMPTY,
    // lenient: Dart rejects `@else` / `  if $b` — "expected selector."
    bare_words: &["if"],
    ..IF
};

const PARAMS: TokenSet = TokenSet::new(&[LPAREN, COMMA]);
const PARENS: TokenSet = TokenSet::new(&[LPAREN]);

pub(crate) const MIXIN: HeaderLines = HeaderLines {
    starts: PARAMS,
    words: &[],
    bare_starts: PARAMS,
    bare_words: &[],
    end_words: &[],
    name_continues: true,
};

pub(crate) const FUNCTION: HeaderLines = MIXIN;

pub(crate) const INCLUDE: HeaderLines = HeaderLines {
    // lenient: Dart rejects `@include foo` / `  ($x)` — "Mixin doesn't accept a content
    // block."
    starts: PARAMS,
    words: &["using"],
    bare_starts: PARAMS,
    bare_words: &["using"],
    end_words: &["using"],
    ..MIXIN
};

pub(crate) const EXTEND: HeaderLines = HeaderLines {
    starts: TokenSet::new(&[COMMA]),
    bare_starts: TokenSet::new(&[COMMA]),
    ..MIXIN
};

pub(crate) const CONTENT: HeaderLines = HeaderLines {
    starts: PARENS,
    bare_starts: PARENS,
    name_continues: false,
    ..MIXIN
};

const QUERY_WORDS: &[&str] = &["and", "or", "not"];

/// `@media` and `@supports`.
pub(crate) const QUERY: HeaderLines = HeaderLines {
    starts: PARAMS,
    words: QUERY_WORDS,
    bare_starts: PARAMS,
    bare_words: QUERY_WORDS,
    // lenient: Dart rejects `@media screen and` / `  (min-width: 1px)` — "expected media
    // condition in parentheses."
    end_words: QUERY_WORDS,
    name_continues: false,
};

pub(crate) const CONTAINER: HeaderLines = HeaderLines {
    // lenient: Dart rejects `@container card` / `  (width > 400px)`, and `@container card
    // (width > 400px) and` / `  (height > 100px)` — "expected selector."
    starts: PARENS,
    bare_starts: PARENS,
    ..QUERY
};

pub(crate) const SCOPE: HeaderLines = HeaderLines {
    starts: PARENS,
    words: &["to"],
    bare_starts: PARENS,
    bare_words: &["to"],
    // lenient: Dart rejects `@scope (.a) to` / `  (.b)` — "expected selector."
    end_words: &["to"],
    name_continues: false,
};

pub(crate) const AT_ROOT: HeaderLines = HeaderLines {
    starts: PARENS,
    words: &[],
    // lenient: Dart rejects `@at-root` / `  (without: media)` — "expected selector."
    bare_starts: PARENS,
    bare_words: &[],
    end_words: &[],
    name_continues: false,
};

#[rustfmt::skip]
const MODULE_STARTS: TokenSet = TokenSet::new(&[
    LPAREN, COMMA, QUOTED_STRING, STRING_START,
]);

const MODULE_WORDS: &[&str] = &["as", "with", "show", "hide"];

/// `@use`, `@forward` and `@import`.
pub(crate) const MODULE: HeaderLines = HeaderLines {
    starts: MODULE_STARTS,
    // lenient: Dart rejects `@use "x"` / `  as y` — "Nothing may be indented beneath a
    // @use rule."
    words: MODULE_WORDS,
    bare_starts: MODULE_STARTS,
    bare_words: MODULE_WORDS,
    end_words: MODULE_WORDS,
    name_continues: false,
};

pub(crate) const CHARSET: HeaderLines = HeaderLines {
    starts: TokenSet::new(&[QUOTED_STRING, STRING_START]),
    words: &[],
    bare_starts: TokenSet::new(&[QUOTED_STRING, STRING_START]),
    bare_words: &[],
    end_words: &[],
    name_continues: false,
};

#[rustfmt::skip]
const VALUE_STARTS: TokenSet = TokenSet::new(&[
    DOLLAR, LPAREN, QUOTED_STRING, STRING_START, COMMA, COLON, NUMBER,
]);

#[rustfmt::skip]
const VALUE_WORDS: &[&str] = &[
    "from", "through", "to", "in", "and", "or", "not", "as", "with", "show", "hide",
    "using", "if",
];

/// A variable declaration's value goes on below it on a line that starts like a value.
pub(crate) const VARIABLE: HeaderLines = HeaderLines {
    starts: VALUE_STARTS,
    words: VALUE_WORDS,
    bare_starts: VALUE_STARTS,
    bare_words: VALUE_WORDS,
    end_words: &[],
    name_continues: false,
};

/// Operators that leave a run of tokens unfinished at the end of a line.
#[rustfmt::skip]
const UNFINISHED: TokenSet = TokenSet::new(&[
    BANG, PLUS, MINUS, SLASH, PERCENT, EQ_EQ, BANG_EQ, GT_EQ, LT_EQ, GT, LT, TILDE,
]);

/// The tokens after which the grammar continues a statement at the end of its line,
/// whatever the statement: lookahead that decides what a statement is reads across them
/// (see [`Parser::joined`]).
#[rustfmt::skip]
pub(crate) const LINE_JOINS: TokenSet = TokenSet::new(&[
    COMMA, BANG, PLUS, MINUS, SLASH, PERCENT, EQ_EQ, BANG_EQ, GT_EQ, LT_EQ, GT, LT, TILDE,
]);

/// Bump a token of a run the grammar does not structure (a media query, an unknown
/// at-rule's prelude): a comma or an operator at the end of a line continues the run on
/// the next one.
pub(crate) fn bump_in_run(p: &mut Parser<'_>) {
    let kind = p.current();
    if kind == COMMA {
        p.bump();
        p.continue_line_across_blank();
    } else if UNFINISHED.contains(kind) {
        bump_operator(p);
    } else {
        p.bump();
    }
}

/// Bump an operator, which continues its statement on the next line if it ends this
/// one — except a `%` right after a number, which reads as the number's unit there.
pub(crate) fn bump_operator(p: &mut Parser<'_>) {
    let unit = p.at(PERCENT) && p.prev_kind() == NUMBER;
    p.bump();
    if !unit {
        p.continue_line();
    }
}

/// [`Parser::err_and_bump`], with the token bumped as in [`bump_in_run`].
pub(crate) fn err_and_bump(p: &mut Parser<'_>, message: &str) {
    let m = p.start();
    p.error(message);
    bump_in_run(p);
    let _ = m.complete(p, ERROR);
}
