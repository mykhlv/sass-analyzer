use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@mixin name(params) { body }` or `@mixin name { body }`
pub fn mixin_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // mixin
    p.expect(IDENT); // name
    if p.at(LPAREN) {
        super::param_list(p);
    }
    if p.at(LBRACE) {
        super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, MIXIN_RULE);
}

/// `@include name(args) { content }` or `@include name;`
/// Also supports `@include name(args) using ($arg) { content }`
pub fn include_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // include
    // Mixin name — plain IDENT or namespace-qualified ns.name
    if p.at(IDENT)
        && p.nth(1) == DOT
        && !p.nth_has_whitespace_before(1)
        && p.nth(2) == IDENT
        && !p.nth_has_whitespace_before(2)
    {
        let nm = p.start();
        p.bump(); // IDENT (namespace)
        p.bump(); // DOT
        p.bump(); // IDENT (mixin name)
        let _ = nm.complete(p, NAMESPACE_REF);
    } else {
        p.expect(IDENT);
    }

    // Optional argument list (whitespace before `(` is allowed in @include)
    if p.at(LPAREN) {
        super::expressions::arg_list(p, super::ParseContext::SassScript);
    }

    // Optional `using ($args)` for content block arguments
    if p.at(IDENT) && p.current_text() == "using" {
        p.bump(); // using
        if p.at(LPAREN) {
            super::param_list(p);
        }
    }

    // Optional content block
    if p.at(LBRACE) {
        super::block(p);
    } else if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, INCLUDE_RULE);
}

/// `@content` or `@content(args)`
pub fn content_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // content
    if p.at(LPAREN) {
        super::expressions::arg_list(p, super::ParseContext::SassScript);
    }
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, CONTENT_RULE);
}
