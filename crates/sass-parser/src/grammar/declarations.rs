use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

use super::ParseContext;
use super::expressions;
use super::lines::WordOperator;
use super::selectors;

/// 2.7: Parse declaration or nested property.
///
/// Dispatches to custom property, nested property, or regular declaration.
pub(crate) fn declaration(p: &mut Parser<'_>) {
    // 2.9: Detect custom properties (--var)
    if p.at(IDENT) && p.current_text().starts_with("--") {
        custom_property_declaration(p);
        return;
    }

    let m = p.start();
    property(p);
    p.expect(COLON);

    // 2.10: Check for nested property patterns
    if p.at(LBRACE) {
        // IDENT COLON LBRACE → nested property with no value: `font: { weight: bold; }`
        super::property_block(p);
        let _ = m.complete(p, NESTED_PROPERTY);
        return;
    }

    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() && !p.at(LBRACE) {
        value(p);
    }

    // Check for !important after the value
    if p.at(BANG) {
        let joined = p.joined(super::lines::LINE_JOINS);
        let flag_text = if p.nth_joined(joined, 1) == IDENT {
            p.nth_text_joined(joined, 1)
        } else {
            ""
        };
        if flag_text.eq_ignore_ascii_case("important") {
            let fm = p.start();
            p.bump(); // !
            p.continue_line_across_blank();
            p.bump(); // important
            let _ = fm.complete(p, IMPORTANT);
        }
    }

    // 2.10(c): Value-and-block: `margin: 10px { top: 20px; }`
    if p.at(LBRACE) {
        super::property_block(p);
        let _ = m.complete(p, NESTED_PROPERTY);
        return;
    }

    // A string left unclosed took the rest of its line, the `;` with it, and the value
    // ends at the declaration on the line after.
    let swallowed = p.after_unclosed_string() && at_next_declaration(p);
    if !p.at(RBRACE) && !p.at_end() && !swallowed {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, DECLARATION);
}

/// 2.9: Parse CSS custom property declaration.
///
/// `--var: value;` where value is raw tokens (not Sass expressions).
/// Depth-aware: tracks `()`, `[]`, `{}` nesting.
fn custom_property_declaration(p: &mut Parser<'_>) {
    let m = p.start();
    let errors = p.error_count();
    property(p);
    p.expect(COLON);
    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        custom_property_value(p, false);
    }
    super::lines::end_without_block(p, errors, "a custom property");
    let _ = m.complete(p, CUSTOM_PROPERTY_DECL);
}

/// Parse a property name, including interpolation.
///
/// Handles: `color`, `--custom`, `#{$prop}`, `border-#{$side}`,
/// `#{$a}-#{$b}`, `#{$ns}-#{$prop}-color`, and the hacks `*color`, `.color`, `#color`,
/// `:color`.
fn property(p: &mut Parser<'_>) {
    let m = p.start();
    if p.at(DOT) || p.at(HASH) || p.at(COLON) || p.at(STAR) {
        p.bump();
    }
    if !p.at(IDENT) && !p.at(HASH_LBRACE) && !p.at(MINUS) && !p.at(STAR) {
        p.error("expected property name");
        let _ = m.complete(p, PROPERTY);
        return;
    }
    // Consume interleaved IDENT/MINUS/HASH_LBRACE fragments (no whitespace between)
    loop {
        match p.current() {
            IDENT | MINUS | STAR => p.bump(),
            HASH_LBRACE => {
                let _ = expressions::interpolation(p);
            }
            _ => break,
        }
        if p.has_whitespace_before() {
            break;
        }
    }
    let _ = m.complete(p, PROPERTY);
}

/// Parse declaration value using expression-based parsing.
///
/// Parses space-separated expressions as direct children of the DECLARATION.
/// Wraps in a VALUE node (acts as the expression-group container).
/// Comma-separated values and `/` separators are consumed within.
fn value(p: &mut Parser<'_>) {
    let m = p.start();
    let ctx = ParseContext::CssValue;

    let mut has_content = false;
    // The last item was an operand, which `and` or `or` after it would combine.
    let mut operand_before = false;
    loop {
        // A string left unclosed ends the declaration at its line break: it most likely
        // took the `;`, and if not, the next line is still more likely a new statement.
        if p.at(SEMICOLON)
            || p.at(RBRACE)
            || p.at(LBRACE)
            || p.at_end()
            || p.at_unclosed_string_break()
        {
            break;
        }
        // Break for `!important` only when it's at the END of the value
        if p.at(BANG) {
            let joined = p.joined(super::lines::LINE_JOINS);
            let important = p.nth_joined(joined, 1) == IDENT
                && p.nth_text_joined(joined, 1)
                    .eq_ignore_ascii_case("important");
            if important {
                let after = p.nth_joined(joined, 2);
                if after == SEMICOLON || after == RBRACE || after == LBRACE || after == EOF {
                    break;
                }
            } else if expressions::at_important_expr(p) {
                // `! d`: Dart Sass reads `!important` there, missing its word.
                expressions::important(p);
                has_content = true;
                operand_before = false;
                continue;
            }
            // Not end-of-value !important — consume as value content
            p.bump();
            p.continue_line();
            has_content = true;
            operand_before = false;
            continue;
        }
        // In CssValue context, `/` is a separator — just consume it
        if p.at(SLASH) {
            p.bump();
            p.continue_line_to(expressions::EXPR_START);
            has_content = true;
            operand_before = false;
            continue;
        }
        // A comma that ends its line is a trailing one: the value ends there.
        if p.at(COMMA) {
            p.bump();
            has_content = true;
            operand_before = false;
            continue;
        }
        if has_content && p.braced() && at_next_declaration(p) {
            break;
        }
        if matches!(p.current(), COLON | RPAREN | RBRACKET) {
            let message = if !has_content {
                "expected expression"
            } else if p.braced() {
                "expected SEMICOLON"
            } else {
                "expected newline"
            };
            stray_in_value(p, message);
            has_content = true;
            break;
        }
        if p.at(DOT) {
            // A `.` that starts no number. Joined to a word, it names a module's
            // function, `math.div(...)`, whose `(` is missing.
            let message = if p.prev_kind() == IDENT && !p.has_whitespace_before() {
                "expected `(`"
            } else {
                "expected digit"
            };
            stray_in_value(p, message);
            has_content = true;
            break;
        }
        if !p.at_ts(expressions::EXPR_START) {
            // Unknown token in value position — bump for progress
            super::lines::bump_in_run(p);
            has_content = true;
            operand_before = false;
            continue;
        }
        let operator = super::lines::word_operator(p, operand_before);
        if expressions::expr(p, ctx).is_some() {
            has_content = true;
            operand_before = true;
            match operator {
                WordOperator::GoesOn => {
                    p.continue_line_to(expressions::EXPR_START);
                }
                WordOperator::NoOperand => {
                    p.error_after_prev(format!("expected expression after `{}`", p.prev_text()));
                }
                WordOperator::No => {}
            }
        } else {
            break;
        }
    }

    if !has_content && !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at(LBRACE) && !p.at_end() {
        super::lines::err_and_bump(p, "expected value");
    }

    let _ = m.complete(p, VALUE);
}

/// Whether the value goes on at the cursor with `name:`, which no value holds: the next
/// declaration, missing the `;` before it; braced only, as a line holds one statement.
/// `progid:` starts an IE filter function.
fn at_next_declaration(p: &Parser<'_>) -> bool {
    p.at(IDENT)
        && p.nth(1) == COLON
        && p.has_whitespace_before()
        && !p.nth_has_whitespace_before(1)
        && !p.current_text().eq_ignore_ascii_case("progid")
}

/// At a token no value holds where it stands: a `:`, `)` or `]`, before which Dart Sass
/// expects the declaration to end, or reads no value at all if it comes first, or a `.`
/// that starts no number. That is one error, `message`, which takes the rest of the value,
/// up to where the declaration ends.
fn stray_in_value(p: &mut Parser<'_>, message: &str) {
    let m = p.start();
    // A bracket that stopped at the token has reported it already.
    if !p.reported_here() {
        p.error(message);
    }
    while !matches!(p.current(), SEMICOLON | RBRACE | LBRACE | EOF)
        && !p.at_end()
        && !p.at_unclosed_string_break()
    {
        if matches!(p.current(), LPAREN | LBRACKET | HASH_LBRACE) {
            super::lines::skip_bracket(p);
        } else {
            p.bump();
        }
    }
    let _ = m.complete(p, ERROR);
}

/// Parse a declaration inside a CSS `@function --name()` body.
///
/// Like a regular declaration, but values are raw CSS tokens (like custom properties).
pub(super) fn css_function_body_declaration(p: &mut Parser<'_>) {
    let m = p.start();
    property(p);
    p.expect(COLON);
    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        custom_property_value(p, true);
    }
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, DECLARATION);
}

/// 2.9: Parse custom property value — fully depth-aware raw token scan.
///
/// Tracks `()`, `[]`, `{}` nesting. Only terminates at `;` or `}` at depth 0.
/// Interpolation `#{...}` is still processed. In the indented layout, the lines indented
/// beneath it are taken in as a brace group only if `takes_block`: a custom property
/// takes none.
fn custom_property_value(p: &mut Parser<'_>, takes_block: bool) {
    let m = p.start();
    let mut paren_depth: u32 = 0;
    let mut bracket_depth: u32 = 0;
    let mut brace_depth: u32 = 0;
    loop {
        let at_depth_zero = paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
        // At the line break after a string left unclosed, as `value` does, inside brackets
        // too: the string most likely took their ends with the `;`, `--a: f("x);`.
        if p.at_unclosed_string_break() {
            break;
        }
        match p.current() {
            EOF => break,
            SEMICOLON if at_depth_zero => break,
            RBRACE if at_depth_zero => break,
            LBRACE if at_depth_zero && !takes_block && p.at_lines_beneath() => break,
            LPAREN => {
                paren_depth += 1;
                p.bump();
            }
            RPAREN => {
                paren_depth = paren_depth.saturating_sub(1);
                p.bump();
            }
            LBRACKET => {
                bracket_depth += 1;
                p.bump();
            }
            RBRACKET => {
                bracket_depth = bracket_depth.saturating_sub(1);
                p.bump();
            }
            LBRACE => {
                brace_depth += 1;
                p.bump();
            }
            RBRACE => {
                brace_depth = brace_depth.saturating_sub(1);
                p.bump();
            }
            HASH_LBRACE => {
                let _ = selectors::interpolation(p);
            }
            _ => p.bump(),
        }
    }
    let _ = m.complete(p, VALUE);
}
