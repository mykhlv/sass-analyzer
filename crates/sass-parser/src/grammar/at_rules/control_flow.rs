use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@if condition { } @else if condition { } @else { }`
pub fn if_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // if
    super::expressions::expr(p, super::ParseContext::SassScript);
    if p.at(LBRACE) {
        super::block(p);
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
        super::expressions::expr(p, super::ParseContext::SassScript);
    }

    if p.at(LBRACE) {
        super::block(p);
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

    super::expressions::expr(p, super::ParseContext::SassScript);

    // Expect `through` or `to`
    if p.at(IDENT) && (p.current_text() == "through" || p.current_text() == "to") {
        p.bump();
    } else {
        p.error("expected `through` or `to`");
    }

    super::expressions::expr(p, super::ParseContext::SassScript);

    if p.at(LBRACE) {
        super::block(p);
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

    super::expressions::expr_list(p, super::ParseContext::SassScript);

    if p.at(LBRACE) {
        super::block(p);
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
    super::expressions::expr(p, super::ParseContext::SassScript);
    if p.at(LBRACE) {
        super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, WHILE_RULE);
}
