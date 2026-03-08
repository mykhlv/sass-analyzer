use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@if condition { } @else if condition { } @else { }`
pub fn if_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // if
    crate::grammar::expressions::expr(p, crate::grammar::ParseContext::SassScript);
    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }

    // Consume @else / @else if clauses
    while p.at(AT) && p.nth_text(1) == "else" {
        else_clause(p);
    }

    let _ = m.complete(p, IF_RULE);
}

/// `@else if condition { }` or `@else { }`
fn else_clause(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // else

    // `@else if` — has a condition
    if p.at(IDENT) && p.current_text() == "if" {
        p.bump(); // if
        crate::grammar::expressions::expr(p, crate::grammar::ParseContext::SassScript);
    }

    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, ELSE_CLAUSE);
}

/// `@for $var from expr through/to expr { }`
pub fn for_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // for
    p.expect(DOLLAR);
    p.expect(IDENT); // variable name

    // Expect `from`
    if p.at(IDENT) && p.current_text() == "from" {
        p.bump();
    } else {
        p.error("expected `from`");
    }

    crate::grammar::expressions::expr(p, crate::grammar::ParseContext::SassScript);

    // Expect `through` or `to`
    if p.at(IDENT) && (p.current_text() == "through" || p.current_text() == "to") {
        p.bump();
    } else {
        p.error("expected `through` or `to`");
    }

    crate::grammar::expressions::expr(p, crate::grammar::ParseContext::SassScript);

    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, FOR_RULE);
}

/// `@each $var [, $var2] in expr { }`
pub fn each_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // each
    p.expect(DOLLAR);
    p.expect(IDENT);

    // Additional destructured variables: `$key, $value`
    while p.eat(COMMA) {
        p.expect(DOLLAR);
        p.expect(IDENT);
    }

    // Expect `in`
    if p.at(IDENT) && p.current_text() == "in" {
        p.bump();
    } else {
        p.error("expected `in`");
    }

    crate::grammar::expressions::expr_list(p, crate::grammar::ParseContext::SassScript);

    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, EACH_RULE);
}

/// `@while condition { }`
pub fn while_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // while
    crate::grammar::expressions::expr(p, crate::grammar::ParseContext::SassScript);
    if p.at(LBRACE) {
        super::super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, WHILE_RULE);
}
