//! Where a statement goes on across a line break in the indented syntax.
//!
//! As in Dart Sass, a statement goes on across a line break only where it cannot end:
//! right after a token that leaves it unfinished (an operator, a comma in a list that
//! needs another item), and at each position of a header that requires more. The grammar continues the line there itself, with
//! [`Parser::continue_line`] or its comma form. The tables here are the part of a header
//! that is decided ahead of the cursor: whether it goes on after its name, or after one of
//! its words, at the end of a line, which the at-rule function declares with
//! [`Parser::continue_header`] when it has read the rule's name. A line indented beneath
//! a header that may end is the rule's block, not more of its header.

use crate::parser::{HeaderLines, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

const OPERATOR_WORDS: &[&str] = &["and", "or", "not"];

/// `@if`, `@while`, `@return`, `@debug`, `@warn` and `@error`: the expression is required.
pub(crate) const EXPRESSION: HeaderLines = HeaderLines {
    end_words: OPERATOR_WORDS,
    name_continues: true,
};

pub(crate) const FOR: HeaderLines = HeaderLines {
    end_words: &["from", "through", "to", "and", "or", "not"],
    name_continues: true,
};

pub(crate) const EACH: HeaderLines = HeaderLines {
    end_words: &["in", "and", "or", "not"],
    name_continues: true,
};

/// A bare `@else` is a complete header: what is indented beneath it is its body.
pub(crate) const ELSE: HeaderLines = HeaderLines {
    end_words: &["if", "and", "or", "not"],
    name_continues: false,
};

/// `@mixin`, `@function` and `@extend`: the name, or the selector, is required.
pub(crate) const NAMED: HeaderLines = HeaderLines {
    end_words: &[],
    name_continues: true,
};

pub(crate) const INCLUDE: HeaderLines = HeaderLines {
    end_words: &["using"],
    name_continues: true,
};

/// `@use` and `@forward`.
pub(crate) const MODULE: HeaderLines = HeaderLines {
    end_words: &["as", "with", "show", "hide"],
    name_continues: true,
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
        let unit = p.at(PERCENT) && p.prev_kind() == NUMBER;
        p.bump();
        if !unit {
            p.continue_line();
        }
    } else {
        p.bump();
    }
}

/// Bump an operator of an expression, which continues its statement on the next line if
/// it ends this one, across blank lines to a line that starts an expression, as in Dart
/// Sass — except a `%` right after a number, which reads as the number's unit there.
pub(crate) fn bump_operator(p: &mut Parser<'_>) {
    let unit = p.at(PERCENT) && p.prev_kind() == NUMBER;
    p.bump();
    if !unit {
        p.continue_line_to(super::expressions::EXPR_START);
    }
}

/// How a value goes on after the token at the cursor, when it is a word that a CSS value
/// reads as a plain identifier but Dart Sass as an operator: `and` or `or` after an
/// operand, or `not`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordOperator {
    /// Not such an operator, or not at the end of its line.
    No,
    /// At the end of its line: Dart takes the next line as its operand.
    GoesOn,
    /// At the end of its line, before a line that starts like a declaration (`name:`),
    /// blank lines between or not, which cannot be its operand — Dart rejects it. The
    /// line is left a statement of its own, and the operator is missing its operand.
    NoOperand,
}

pub(crate) fn word_operator(p: &Parser<'_>, operand_before: bool) -> WordOperator {
    if !p.at(IDENT) {
        return WordOperator::No;
    }
    let operator = match p.current_text() {
        "and" | "or" => operand_before,
        "not" => true,
        _ => false,
    };
    if !operator || !matches!(p.nth(1), SEMICOLON | LBRACE | RBRACE | EOF) {
        return WordOperator::No;
    }
    if p.next_line_starts_declaration() {
        WordOperator::NoOperand
    } else {
        WordOperator::GoesOn
    }
}

/// [`Parser::err_and_bump`], with the token bumped as in [`bump_in_run`].
pub(crate) fn err_and_bump(p: &mut Parser<'_>, message: &str) {
    let m = p.start();
    p.error(message);
    bump_in_run(p);
    let _ = m.complete(p, ERROR);
}

/// Where the tokens of a line end in the indented layout, at the line's zero-width token
/// or a written one.
const LINE_END: TokenSet = TokenSet::new(&[SEMICOLON, LBRACE, RBRACE]);

/// A line that starts no statement, at the cursor: one error, which takes the rest of
/// the line, and below it the lines indented beneath it, which still parse as a block.
/// Dart Sass reads such a line as a style rule whose selector it rejects.
pub(crate) fn err_line(p: &mut Parser<'_>, message: &str, report: bool) {
    let m = p.start();
    if report {
        p.error(message);
    }
    skip_line(p);
    if p.at(LBRACE) {
        super::block(p);
    }
    let _ = m.complete(p, ERROR);
}

/// After a statement, in the indented layout: what is left of its line is one error,
/// `expected newline`, unless the statement has reported one since `errors`, with the
/// lines indented beneath it, which still parse as a block.
pub(crate) fn rest_of_line(p: &mut Parser<'_>, errors: usize) {
    if p.braced() || p.at_ts(LINE_END) || p.at_end() || p.at_statement_start() {
        return;
    }
    let m = p.start();
    if p.error_count() == errors {
        p.error(if after_written_semicolon(p) {
            MULTIPLE_STATEMENTS
        } else {
            "expected newline"
        });
    }
    skip_line(p);
    if p.at(LBRACE) {
        super::block(p);
    }
    let _ = m.complete(p, ERROR);
}

const MULTIPLE_STATEMENTS: &str =
    "multiple statements on one line are not supported in the indented syntax";

/// Whether the token before the cursor is a `;` written on its line, not the zero-width
/// one that ends a line.
fn after_written_semicolon(p: &Parser<'_>) -> bool {
    p.prev_kind() == SEMICOLON && !p.prev_text().is_empty()
}

/// A `;` where a statement may start: braced, or the zero-width one that ends a line, an
/// empty statement. A written one in the indented layout is one error, as in Dart Sass:
/// after another `;` on its line, as `b: c;;`, a second statement on the line; on a line
/// of its own, no statement at all.
pub(crate) fn stray_semicolon(p: &mut Parser<'_>) {
    debug_assert!(p.at(SEMICOLON));
    if p.braced() || p.current_text().is_empty() {
        p.bump();
        return;
    }
    let m = p.start();
    if after_written_semicolon(p) && !p.at_line_break() {
        p.error(MULTIPLE_STATEMENTS);
    } else {
        p.error("expected end of rule");
    }
    p.bump();
    let _ = m.complete(p, ERROR);
}

/// After a selector in the indented layout, at a written `;`, which ends no rule there:
/// one error, `expected newline` as Dart Sass reports it, with the rest of the line, and
/// below it the lines indented beneath it, which are still the rule's block.
pub(crate) fn semicolon_after_selector(p: &mut Parser<'_>) -> bool {
    if p.braced() || !p.at(SEMICOLON) || p.current_text().is_empty() {
        return false;
    }
    let m = p.start();
    p.error("expected newline");
    p.bump();
    skip_line(p);
    let _ = m.complete(p, ERROR);
    true
}

/// The end of a statement that takes no block. In the indented layout, the lines
/// indented beneath it are one error, unless the statement has reported one since
/// `errors`, and are skipped as a whole, as Dart Sass stops at them.
pub(crate) fn end_without_block(p: &mut Parser<'_>, errors: usize, statement: &str) {
    if !p.at_lines_beneath() {
        if !p.at(RBRACE) && !p.at_end() {
            p.expect(SEMICOLON);
        }
        return;
    }
    let m = p.start();
    if p.error_count() == errors {
        p.error(format!("nothing may be indented beneath {statement}"));
    }
    p.bump(); // {
    let mut depth = 1u32;
    while !p.at_end() {
        match p.current() {
            LBRACE => depth += 1,
            RBRACE => {
                depth -= 1;
                if depth == 0 {
                    p.bump();
                    break;
                }
            }
            LPAREN | LBRACKET | HASH_LBRACE => {
                skip_bracket(p);
                continue;
            }
            _ => {}
        }
        p.bump();
    }
    let _ = m.complete(p, ERROR);
}

pub(crate) fn skip_line(p: &mut Parser<'_>) {
    while !p.at_ts(LINE_END) && !p.at_end() {
        if matches!(p.current(), LPAREN | LBRACKET | HASH_LBRACE) {
            skip_bracket(p);
        } else {
            p.bump();
        }
    }
}

/// A bracket in tokens being skipped, through its closer, so that the line breaks
/// inside it stay trivia and one it leaves unclosed ends as the grammar's do, unreported.
pub(crate) fn skip_bracket(p: &mut Parser<'_>) {
    match p.current() {
        LPAREN => super::eat_balanced_with(p, LPAREN, RPAREN, false),
        LBRACKET => super::eat_balanced_with(p, LBRACKET, RBRACKET, false),
        _ => {
            let _ = super::expressions::interpolation(p);
        }
    }
}

/// After a condition read as it comes in the indented layout — a media query, a
/// `@supports`, `@container` or `@scope` prelude: whether its line leaves it unfinished,
/// `empty` or ending with a comma or one of `words`, which Dart Sass reports rather than
/// read on below it. Reports it, right after the condition.
pub(crate) fn unfinished_condition(
    p: &mut Parser<'_>,
    empty: bool,
    words: &[&str],
    condition: &str,
) {
    if p.braced() || !(p.at_virtual() || p.at_end()) {
        return;
    }
    if empty {
        p.error_after_prev(format!("expected {condition}"));
    } else if p.prev_kind() == COMMA
        || (p.prev_kind() == IDENT
            && words
                .iter()
                .any(|word| p.prev_text().eq_ignore_ascii_case(word)))
    {
        p.error_after_prev(format!("expected {condition} after `{}`", p.prev_text()));
    }
}
