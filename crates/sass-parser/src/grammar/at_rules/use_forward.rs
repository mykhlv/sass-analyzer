use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@use "path"` / `as name` / `as *` / `with ($var: value, ...)`
pub fn use_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // use

    // Path — must be a non-interpolated string
    if p.at(STRING_START) {
        p.error("interpolation is not allowed in @use paths");
        // Consume the interpolated string anyway for recovery
        while !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
            p.bump();
        }
    } else {
        p.expect(QUOTED_STRING);
    }

    // Optional `as name` or `as *`
    if p.at(IDENT) && p.current_text() == "as" {
        p.bump(); // as
        if p.at(STAR) {
            p.bump();
        } else {
            p.expect(IDENT);
        }
    }

    // Optional `with ($var: value, ...)`
    if p.at(IDENT) && p.current_text() == "with" {
        p.bump(); // with
        with_config(p);
    }

    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, USE_RULE);
}

/// `@forward "path"` / `as prefix-*` / `hide`/`show` / `with (...)`
pub fn forward_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // forward

    // Path
    if p.at(STRING_START) {
        p.error("interpolation is not allowed in @forward paths");
        while !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
            p.bump();
        }
    } else {
        p.expect(QUOTED_STRING);
    }

    // Optional clauses: `as prefix-*`, `hide ...`, `show ...`, `with (...)`
    while p.at(IDENT) {
        match p.current_text() {
            "as" => {
                p.bump(); // as
                // `prefix-*`: IDENT + potential `-` + `*`
                if p.at(STAR) {
                    p.bump();
                } else {
                    // Consume prefix tokens: `prefix-*` is IDENT MINUS STAR or IDENT STAR
                    if p.at(IDENT) {
                        p.bump();
                    }
                    p.eat(MINUS);
                    if p.at(STAR) {
                        p.bump();
                    }
                }
            }
            "hide" | "show" => {
                p.bump(); // hide/show
                // List of names: identifiers and $variables
                if p.at(IDENT) || p.at(DOLLAR) {
                    visibility_member(p);
                    while p.eat(COMMA) {
                        visibility_member(p);
                    }
                }
            }
            "with" => {
                p.bump(); // with
                with_config(p);
                break;
            }
            _ => break,
        }
    }

    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, FORWARD_RULE);
}

/// Parse a member in `hide`/`show` list: `name` or `$variable`
fn visibility_member(p: &mut Parser<'_>) {
    if p.at(DOLLAR) {
        p.bump(); // $
        p.expect(IDENT);
    } else if p.at(IDENT) {
        p.bump();
    } else {
        p.error("expected member name");
    }
}

/// Parse `($var: value, ...)` configuration block for `@use`/`@forward` `with()`.
fn with_config(p: &mut Parser<'_>) {
    if !p.at(LPAREN) {
        p.error("expected `(`");
        return;
    }
    p.bump(); // (

    if !p.at(RPAREN) && !p.at_end() {
        with_config_entry(p);
        while p.eat(COMMA) {
            if !p.at(RPAREN) && !p.at_end() {
                with_config_entry(p);
            }
        }
    }

    p.expect(RPAREN);
}

/// Parse `$var: value` or `$var: value !default` inside `with()`.
fn with_config_entry(p: &mut Parser<'_>) {
    p.expect(DOLLAR);
    p.expect(IDENT);
    p.expect(COLON);
    super::expressions::expr(p, super::ParseContext::SassScript);

    // Optional !default flag (valid in @forward with(), not @use with())
    if p.at(BANG) && p.nth(1) == IDENT && p.nth_text(1) == "default" {
        let fm = p.start();
        p.bump(); // !
        p.bump(); // default
        let _ = fm.complete(p, SASS_FLAG);
    }
}

/// `@import "path"` (deprecated, but still valid syntax)
pub fn import_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // import

    // Parse one or more import paths (comma-separated)
    import_argument(p);
    while p.eat(COMMA) {
        import_argument(p);
    }

    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, IMPORT_RULE);
}

/// Parse a single import argument: string path or `url()`
fn import_argument(p: &mut Parser<'_>) {
    if p.at(QUOTED_STRING) {
        p.bump();
    } else if p.at(STRING_START) {
        // Interpolated string — consume the full STRING_START..STRING_END sequence
        let _ = super::expressions::interpolated_string(p, super::ParseContext::SassScript);
    } else if p.at(IDENT) && p.current_text().eq_ignore_ascii_case("url") {
        // url(...) import
        p.bump(); // url
        if p.at(LPAREN) {
            super::eat_balanced(p, LPAREN, RPAREN);
        }
    } else {
        p.error("expected import path");
    }
}
