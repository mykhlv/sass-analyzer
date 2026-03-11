use crate::parser::Parser;
use crate::syntax_kind::SyntaxKind;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

fn diagnostic_rule(p: &mut Parser<'_>, kind: SyntaxKind) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // error/warn/debug
    super::expressions::sass_value_list(p, super::ParseContext::SassScript);
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, kind);
}

/// `@error expr;`
pub fn at_error_rule(p: &mut Parser<'_>) {
    diagnostic_rule(p, ERROR_RULE);
}

/// `@warn expr;`
pub fn at_warn_rule(p: &mut Parser<'_>) {
    diagnostic_rule(p, WARN_RULE);
}

/// `@debug expr;`
pub fn at_debug_rule(p: &mut Parser<'_>) {
    diagnostic_rule(p, DEBUG_RULE);
}
