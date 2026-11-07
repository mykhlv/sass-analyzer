mod at_root;
mod control_flow;
mod css_at_rules;
mod diagnostics;
mod extend;
mod function;
mod media;
mod mixin;
mod use_forward;

use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

// Re-exports for sub-modules — shortens `super::super::block` → `super::block`
// and `crate::grammar::expressions::*` → `super::expressions::*`.
use super::ParseContext;
use super::block;
use super::declarations;
use super::eat_balanced;
use super::expressions;
use super::expressions::interpolation;
use super::lines;
use super::selectors;

/// Check whether `@function` names a CSS function (custom property or interpolated name).
fn is_css_function_rule(p: &Parser<'_>) -> bool {
    (p.nth(2) == IDENT && p.nth_text(2).starts_with("--"))
        || p.nth(2) == HASH_LBRACE
        || (p.nth(2) == MINUS && p.nth(3) == MINUS)
}

/// Dispatch `@keyword` — called when parser is at `AT`.
pub fn at_rule(p: &mut Parser<'_>) {
    p.begin_at_rule();
    if p.at_shorthand() {
        if p.at(EQ) {
            mixin::mixin_rule(p);
        } else {
            mixin::include_rule(p);
        }
        return;
    }
    assert!(p.at(AT));

    // Interpolated at-rule name: `@#{...}`, `@foo#{...}bar`
    if p.nth(1) == HASH_LBRACE {
        generic_at_rule(p);
        return;
    }

    let name = p.nth_text(1);

    match name {
        "mixin" | "=" => mixin::mixin_rule(p),
        "include" | "+" => mixin::include_rule(p),
        "content" => mixin::content_rule(p),
        "function" => {
            if is_css_function_rule(p) {
                function::css_function_rule(p);
            } else {
                function::function_rule(p);
            }
        }
        "return" => function::return_rule(p),
        "if" => control_flow::if_rule(p),
        "for" => control_flow::for_rule(p),
        "each" => control_flow::each_rule(p),
        "while" => control_flow::while_rule(p),
        "extend" => extend::extend_rule(p),
        "error" => diagnostics::at_error_rule(p),
        "warn" => diagnostics::at_warn_rule(p),
        "debug" => diagnostics::at_debug_rule(p),
        "at-root" => at_root::at_root_rule(p),
        "media" => media::media_rule(p),
        "supports" => media::supports_rule(p),
        "keyframes" | "-webkit-keyframes" | "-moz-keyframes" | "-o-keyframes" => {
            media::keyframes_rule(p);
        }
        "layer" => css_at_rules::layer_rule(p),
        "container" => css_at_rules::container_rule(p),
        "scope" => css_at_rules::scope_rule(p),
        "property" => css_at_rules::property_rule(p),
        "namespace" => css_at_rules::namespace_rule(p),
        "charset" => css_at_rules::charset_rule(p),
        "page" => css_at_rules::page_rule(p),
        "font-face" => css_at_rules::font_face_rule(p),
        "use" => use_forward::use_rule(p),
        "forward" => use_forward::forward_rule(p),
        "import" => use_forward::import_rule(p),
        "else" | "elseif" => {
            // Orphan @else/@elseif — should have been consumed by if_rule
            p.error("`@else` without preceding `@if`");
            generic_at_rule(p);
        }
        _ => {
            // Case-insensitive CSS @function: `@FUNCTION --name() { ... }`
            if name.eq_ignore_ascii_case("function") && is_css_function_rule(p) {
                function::css_function_rule(p);
                return;
            }
            generic_at_rule(p);
        }
    }
}

/// Forward-compatible fallback: unknown `@foo ...;` or `@foo { }`.
fn generic_at_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    // Consume at-rule name: IDENT, interpolation, or mixed (e.g., `@foo#{bar}baz`)
    let mut consumed_name = false;
    while (p.at(IDENT) || p.at(MINUS) || p.at(HASH_LBRACE))
        && (!consumed_name || !p.has_whitespace_before())
    {
        if p.at(HASH_LBRACE) {
            let _ = interpolation(p);
        } else {
            lines::bump_in_run(p);
        }
        consumed_name = true;
    }
    // Consume tokens until ; or { block } at depth 0, or the line break of a string left
    // unclosed, which most likely took the `;`.
    while !p.at(SEMICOLON)
        && !p.at(LBRACE)
        && !p.at(RBRACE)
        && !p.at_end()
        && !p.at_unclosed_string_break()
    {
        if p.at(LPAREN) {
            eat_balanced(p, LPAREN, RPAREN);
        } else if p.at(HASH_LBRACE) {
            let _ = interpolation(p);
        } else {
            lines::bump_in_run(p);
        }
    }
    if p.at(LBRACE) {
        super::generic_at_rule_block(p);
    } else if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, GENERIC_AT_RULE);
}

/// Consume tokens opaquely until a token in `stop` is reached outside parentheses.
/// Handles `#{` interpolation. Used by `@media`, `@supports`, `@container`, `@scope`
/// conditions.
pub(super) fn eat_opaque_condition(p: &mut Parser<'_>, stop: TokenSet) {
    while !p.at_end() && !p.at_ts(stop) {
        match p.current() {
            LPAREN => eat_condition_parens(p),
            HASH_LBRACE => {
                let _ = interpolation(p);
            }
            _ => lines::bump_in_run(p),
        }
    }
}

/// `( ... )` in a condition, nested ones included. A `;` inside stays part of it, as does
/// a balanced brace group the parentheses go on after, as Dart Sass reads
/// `@supports (a {b})`. Any other `{`, or a `}` that closes nothing inside, ends it
/// unclosed; in the indented layout it ends as `eat_balanced` does.
fn eat_condition_parens(p: &mut Parser<'_>) {
    assert!(p.at(LPAREN));
    let p = &mut *p.transparent_guard();
    p.end_unclosed_bracket(LPAREN, RPAREN);
    p.bump();
    let mut depth: u32 = 1;
    let mut braces: u32 = 0;
    while !p.at_end() && depth > 0 && !p.at_unclosed_string_break() && !p.at_virtual() {
        match p.current() {
            LPAREN => depth += 1,
            RPAREN => depth -= 1,
            LBRACE if braces == 0 && !at_brace_group_in_parens(p, depth) => break,
            LBRACE => braces += 1,
            RBRACE if braces == 0 => break,
            RBRACE => braces -= 1,
            HASH_LBRACE => {
                let _ = interpolation(p);
                continue;
            }
            _ => {
                lines::bump_in_run(p);
                continue;
            }
        }
        p.bump();
    }
    if depth > 0 && !p.at_unclosed_string_break() {
        super::report_unclosed(p, RPAREN);
    }
}

/// At a `{` inside `depth` parentheses of a condition: whether it opens a group whose
/// brackets all close in kind, after which the parentheses close too, with no `{` or `;`
/// between the group and their `)`. Otherwise the `{` most likely opens a
/// block and the parentheses were left unclosed. The look ahead ends at that `{` or `;`,
/// so it reads each group and what follows it once.
fn at_brace_group_in_parens(p: &Parser<'_>, depth: u32) -> bool {
    debug_assert!(p.at(LBRACE));
    // The closers expected, innermost last, and whether each closes a brace group.
    let mut expected: Vec<(SyntaxKind, bool)> = vec![(RPAREN, false); depth as usize];
    expected.push((RBRACE, true));
    let mut groups = 1u32;
    let mut offset = 1;
    loop {
        let kind = p.nth(offset);
        offset += 1;
        match kind {
            LPAREN => expected.push((RPAREN, false)),
            LBRACKET => expected.push((RBRACKET, false)),
            HASH_LBRACE => expected.push((RBRACE, false)),
            LBRACE | SEMICOLON if groups == 0 => return false,
            LBRACE => {
                expected.push((RBRACE, true));
                groups += 1;
            }
            RPAREN | RBRACKET | RBRACE => match expected.pop() {
                Some((closer, group)) if closer == kind => {
                    groups -= u32::from(group);
                    if expected.is_empty() {
                        return true;
                    }
                }
                // Even a `}` closing the block the condition is in: the group before it is
                // more likely a rule's block, `.b { c: d }`, than part of the condition.
                _ => return false,
            },
            EOF => return false,
            _ => {}
        }
    }
}

/// Parse a parameter list for @mixin/@function: `($name, $name: default, $rest...)`.
pub fn param_list(p: &mut Parser<'_>) {
    assert!(p.at(LPAREN));
    let p = &mut *p.transparent_guard();
    let m = p.start();
    p.bump(); // (

    if !p.at(RPAREN) && !p.at_end() {
        param(p);
        while p.eat(COMMA) {
            if !p.at(RPAREN) && !p.at_end() {
                param(p);
            }
        }
    }

    p.expect(RPAREN);
    let _ = m.complete(p, PARAM_LIST);
}

fn param(p: &mut Parser<'_>) {
    let m = p.start();
    p.expect(DOLLAR);
    p.expect(IDENT);

    // Optional default value: $name: expr (can be space-separated, stops at `,`/`)`)
    if p.eat(COLON) {
        super::expressions::sass_value(p, super::ParseContext::SassScript);
    }

    // Rest param: $args...
    if p.at(DOT_DOT_DOT) {
        p.bump();
    }

    let _ = m.complete(p, PARAM);
}
