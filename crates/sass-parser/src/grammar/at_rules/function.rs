use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@function name(params) { @return expr; }`
pub fn function_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // function
    p.expect(IDENT); // name
    if p.at(LPAREN) {
        super::param_list(p);
    } else {
        p.error("expected `(`");
    }
    if p.at(LBRACE) {
        super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, FUNCTION_RULE);
}

/// `@return expr;`
pub fn return_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // return
    super::expressions::sass_value_list(p, super::ParseContext::SassScript);
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, RETURN_RULE);
}
