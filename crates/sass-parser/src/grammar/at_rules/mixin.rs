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
        super::super::block(p);
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
    p.expect(IDENT); // mixin name

    // Optional argument list
    if p.at(LPAREN) && !p.has_whitespace_before() {
        crate::grammar::expressions::arg_list(p, crate::grammar::ParseContext::SassScript);
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
        super::super::block(p);
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
    if p.at(LPAREN) && !p.has_whitespace_before() {
        crate::grammar::expressions::arg_list(p, crate::grammar::ParseContext::SassScript);
    }
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, CONTENT_RULE);
}
