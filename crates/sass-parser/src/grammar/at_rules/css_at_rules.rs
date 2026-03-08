use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@layer name { }` or `@layer name, name2;`
pub fn layer_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // layer

    // Optional layer name(s)
    if p.at(IDENT) {
        p.bump();
        // Dot-separated: `layer.sublayer`
        while p.eat(DOT) {
            p.expect(IDENT);
        }

        // Comma-separated multiple names → statement form
        while p.eat(COMMA) {
            if p.at(IDENT) {
                p.bump();
                while p.eat(DOT) {
                    p.expect(IDENT);
                }
            }
        }
    }

    if p.at(LBRACE) {
        super::super::block(p);
    } else if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, LAYER_RULE);
}

/// `@container name (width > 400px) { }` or `@container (width > 400px) { }`
pub fn container_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // container

    // Optional container name (ident before parenthesized condition)
    if p.at(IDENT) {
        p.bump();
    }

    // Consume condition tokens until `{`, `}`, or `;` at depth 0
    let mut depth: u32 = 0;
    while !p.at_end() {
        match p.current() {
            LBRACE | RBRACE | SEMICOLON if depth == 0 => break,
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth = depth.saturating_sub(1);
                p.bump();
            }
            HASH_LBRACE => {
                crate::grammar::expressions::interpolation(p);
            }
            _ => p.bump(),
        }
    }

    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, CONTAINER_RULE);
}

/// `@scope (.card) to (.content) { }`
pub fn scope_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // scope

    // Consume everything until `{`, `}`, or `;` at depth 0
    let mut depth: u32 = 0;
    while !p.at_end() {
        match p.current() {
            LBRACE | RBRACE | SEMICOLON if depth == 0 => break,
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth = depth.saturating_sub(1);
                p.bump();
            }
            HASH_LBRACE => {
                crate::grammar::expressions::interpolation(p);
            }
            _ => p.bump(),
        }
    }

    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, SCOPE_RULE);
}

/// `@property --name { syntax: "<color>"; inherits: false; initial-value: red; }`
pub fn property_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // property

    // Name: `--name` is lexed as a single IDENT (hyphens are valid in CSS idents)
    if p.at(IDENT) {
        if !p.current_text().starts_with("--") {
            p.error("@property name must start with `--`");
        }
        p.bump();
    } else {
        p.error("expected custom property name (e.g. `--my-prop`)");
    }

    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, PROPERTY_RULE);
}

/// `@namespace prefix url(...)` or `@namespace url(...)`
pub fn namespace_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // namespace

    // Optional prefix
    if p.at(IDENT) {
        let text = p.current_text();
        if !text.eq_ignore_ascii_case("url") {
            p.bump(); // prefix
        }
    }

    // URL: either quoted string or `url(...)`
    if p.at(QUOTED_STRING) {
        p.bump();
    } else if p.at(STRING_START) {
        let _ = crate::grammar::expressions::interpolated_string(
            p,
            crate::grammar::ParseContext::SassScript,
        );
    } else if p.at(IDENT) && p.current_text().eq_ignore_ascii_case("url") {
        p.bump(); // url
        if p.at(LPAREN) {
            super::eat_balanced(p, LPAREN, RPAREN);
        }
    }

    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, NAMESPACE_RULE);
}

/// `@charset "UTF-8";`
pub fn charset_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // charset
    p.expect(QUOTED_STRING);
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, CHARSET_RULE);
}

/// `@page :first { margin: 2cm; }`
pub fn page_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // page

    // Optional pseudo-page selector: `:first`, `:left`, `:right`, `:blank`
    while p.at(COLON) || p.at(IDENT) {
        p.bump();
    }

    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, PAGE_RULE);
}

/// `@font-face { font-family: "Noto"; src: url(...); }`
pub fn font_face_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // font-face
    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, FONT_FACE_RULE);
}
