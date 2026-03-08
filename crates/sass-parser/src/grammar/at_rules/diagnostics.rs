use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@error expr;`
pub fn at_error_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // error
    super::expressions::sass_value_list(p, super::ParseContext::SassScript);
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, ERROR_RULE);
}

/// `@warn expr;`
pub fn warn_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // warn
    super::expressions::sass_value_list(p, super::ParseContext::SassScript);
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, WARN_RULE);
}

/// `@debug expr;`
pub fn debug_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // debug
    super::expressions::sass_value_list(p, super::ParseContext::SassScript);
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, DEBUG_RULE);
}
