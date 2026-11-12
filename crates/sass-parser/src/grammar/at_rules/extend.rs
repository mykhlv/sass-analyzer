use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@extend selector !optional;` or `@extend sel1, sel2 !optional;`
pub fn extend_rule(p: &mut Parser<'_>) {
    let m = p.start();
    let errors = p.error_count();
    p.bump(); // @
    p.bump(); // extend
    p.continue_header(&super::lines::NAMED);

    // Parse selector(s) — comma-separated simple/compound selectors
    // @extend only allows simple/compound selectors (no descendant combinators)
    if p.at(SEMICOLON) || p.at(RBRACE) || p.at_end() {
        p.error("expected selector");
    }
    let mut has_selector_token = false;
    let mut has_combinator = false;
    let mut has_interpolation = false;
    while !p.at(SEMICOLON) && !p.at(BANG) && !p.at(RBRACE) && !p.at_end() && !p.at_lines_beneath() {
        if p.at(COMMA) {
            // Comma separates multiple selectors: `@extend .a, .b;`
            has_selector_token = false;
            p.bump();
            continue;
        }
        if p.at(HASH_LBRACE) {
            has_selector_token = true;
            has_interpolation = true;
            let _ = super::interpolation(p);
        } else {
            // Whitespace between selector-start tokens = descendant combinator
            if has_selector_token && p.has_whitespace_before() {
                let kind = p.current();
                if kind == IDENT
                    || kind == DOT
                    || kind == HASH
                    || kind == PERCENT
                    || kind == COLON
                    || kind == COLON_COLON
                    || kind == LBRACKET
                {
                    has_combinator = true;
                }
            }
            has_selector_token = true;
            super::lines::bump_in_run(p);
        }
    }

    // Suppress combinator error when any interpolation is present — interpolation may produce
    // content that changes the selector structure (e.g., `@extend .foo #{","} .bar`).
    // This is intentionally broad (matches Dart Sass behavior): we can't statically know
    // what the interpolation will produce, so we allow it.
    if has_combinator && !has_interpolation {
        p.error("`@extend` does not support descendant combinators");
    }

    // Optional !optional flag, joined to its `!` as Dart Sass requires: `! optional`, or
    // `!` and `optional` on lines of their own, is one error right after the `!`.
    if p.at(BANG) {
        let fm = p.start();
        p.bump(); // !
        if p.at(IDENT) && p.current_text() == "optional" && !p.has_whitespace_before() {
            p.bump();
            let _ = fm.complete(p, SASS_FLAG);
        } else if p.at(IDENT) && p.current_text() == "optional" {
            p.error_after_prev("expected `optional`");
            p.bump();
            let _ = fm.complete(p, ERROR);
        } else {
            p.error("expected `optional`");
            fm.abandon(p);
        }
    }

    super::lines::end_without_block(p, errors, "`@extend`");
    let _ = m.complete(p, EXTEND_RULE);
}
