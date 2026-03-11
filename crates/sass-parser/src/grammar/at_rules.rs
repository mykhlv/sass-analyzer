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
use super::expressions;
use super::expressions::interpolation;
use super::selectors;

/// Dispatch `@keyword` — called when parser is at `AT`.
pub fn at_rule(p: &mut Parser<'_>) {
    assert!(p.at(AT));
    let name = p.nth_text(1);

    match name {
        "mixin" => mixin::mixin_rule(p),
        "include" => mixin::include_rule(p),
        "content" => mixin::content_rule(p),
        "function" => function::function_rule(p),
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
        "else" => {
            // Orphan @else — should have been consumed by if_rule
            p.error("`@else` without preceding `@if`");
            generic_at_rule(p);
        }
        _ => generic_at_rule(p),
    }
}

/// Forward-compatible fallback: unknown `@foo ...;` or `@foo { }`.
fn generic_at_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    if p.at(IDENT) {
        p.bump(); // keyword
    }
    // Consume tokens until ; or { block } at depth 0
    while !p.at(SEMICOLON) && !p.at(LBRACE) && !p.at(RBRACE) && !p.at_end() {
        if p.at(LPAREN) {
            eat_balanced(p, LPAREN, RPAREN);
        } else {
            p.bump();
        }
    }
    if p.at(LBRACE) {
        super::block(p);
    } else if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, GENERIC_AT_RULE);
}

/// Consume balanced delimiters (parens, brackets) including outer pair.
pub(super) fn eat_balanced(p: &mut Parser<'_>, open: SyntaxKind, close: SyntaxKind) {
    assert!(p.at(open));
    p.bump();
    let mut depth: u32 = 1;
    while !p.at_end() && depth > 0 {
        if p.current() == open {
            depth += 1;
        } else if p.current() == close {
            depth -= 1;
        }
        p.bump();
    }
}

/// Consume tokens opaquely until a token in `stop` is reached at paren-depth 0.
/// Tracks `(` / `)` nesting and handles `#{` interpolation.
/// Used by `@media`, `@supports`, `@container`, `@scope` conditions.
pub(super) fn eat_opaque_condition(p: &mut Parser<'_>, stop: TokenSet) {
    let mut depth: u32 = 0;
    while !p.at_end() {
        if depth == 0 && stop.contains(p.current()) {
            break;
        }
        match p.current() {
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth = depth.saturating_sub(1);
                p.bump();
            }
            HASH_LBRACE => {
                let _ = interpolation(p);
            }
            _ => p.bump(),
        }
    }
}

/// Parse a parameter list for @mixin/@function: `($name, $name: default, $rest...)`.
pub fn param_list(p: &mut Parser<'_>) {
    assert!(p.at(LPAREN));
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
