use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@function name(params) { @return expr; }`
pub fn function_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // function
    p.continue_header(&super::lines::NAMED);
    if p.at(IDENT) {
        match p.current_text() {
            "url" | "and" | "or" | "not" | "expression" | "element" => {
                p.error("invalid function name");
            }
            "type" => p.error("this name is reserved for the plain-CSS function"),
            _ => {}
        }
    }
    p.expect(IDENT); // name
    p.continue_line_across_blank();
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
    p.continue_header(&super::lines::NAMED);

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
    let errors = p.error_count();
    p.bump(); // @
    p.bump(); // return
    p.continue_header(&super::lines::EXPRESSION);
    super::expressions::sass_value_list(p, super::ParseContext::SassScript);
    super::lines::end_without_block(p, errors, "`@return`");
    let _ = m.complete(p, RETURN_RULE);
}
