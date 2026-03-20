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
    }
    let _ = m.complete(p, FUNCTION_RULE);
}

/// CSS `@function --name(...) { result: value; }` — raw CSS values in body.
///
/// Unlike Sass `@function`, CSS custom function bodies contain raw CSS declarations,
/// not Sass expressions. Handles `@function`/`@FUNCTION`, `--name`, interpolation,
/// and optional `returns` clause.
pub fn css_function_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // function/FUNCTION

    // Consume function name: `--name`, `--#{interp}`, `--foo#{bar}`, etc.
    let mut consumed_name = false;
    while (p.at(IDENT) || p.at(MINUS) || p.at(HASH_LBRACE))
        && (!consumed_name || !p.has_whitespace_before())
    {
        if p.at(HASH_LBRACE) {
            let _ = super::interpolation(p);
        } else {
            p.bump();
        }
        consumed_name = true;
    }
    if !consumed_name {
        p.error("expected function name");
    }

    // Parameter list
    if p.at(LPAREN) {
        super::eat_balanced(p, LPAREN, RPAREN);
    }

    // Optional `returns <type>` clause — consume opaquely until `{`
    while !p.at(LBRACE) && !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        p.bump();
    }

    if p.at(LBRACE) {
        crate::grammar::css_function_block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, GENERIC_AT_RULE);
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
