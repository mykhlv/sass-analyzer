use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

/// `@if condition { } @else if condition { } @else { }`
pub fn if_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // if
    p.continue_header(&super::lines::EXPRESSION);
    super::expressions::expr(p, super::ParseContext::SassScript);
    if p.at(LBRACE) {
        super::block(p);
    }

    // Consume @else / @else if / @elseif clauses.
    // In indented Sass, a virtual SEMICOLON may separate the @if body from @else.
    loop {
        if p.at(AT) && (p.nth_text(1) == "else" || p.nth_text(1) == "elseif") {
            else_clause(p);
        } else if p.at(SEMICOLON)
            && p.nth_past_virtual(1) == AT
            && (p.nth_text_past_virtual(2) == "else" || p.nth_text_past_virtual(2) == "elseif")
        {
            p.bump(); // skip virtual SEMICOLON
            else_clause(p);
        } else {
            break;
        }
    }

    let _ = m.complete(p, IF_RULE);
}

/// `@else if condition { }`, `@elseif condition { }`, or `@else { }`
fn else_clause(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @

    // `@elseif` (deprecated no-space form) — treat as `@else if`
    if p.current_text() == "elseif" {
        p.bump(); // elseif
        super::expressions::expr(p, super::ParseContext::SassScript);
    } else {
        p.bump(); // else
        p.continue_header(&super::lines::ELSE);

        // `@else if` — has a condition
        if p.at(IDENT) && p.current_text() == "if" {
            p.bump(); // if
            super::expressions::expr(p, super::ParseContext::SassScript);
        }
    }

    if p.at(LBRACE) {
        super::block(p);
    }
    let _ = m.complete(p, ELSE_CLAUSE);
}

/// `@for $var from expr through/to expr { }`
pub fn for_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // for
    p.continue_header(&super::lines::FOR);
    let errors = p.error_count();
    p.expect(DOLLAR);
    p.expect(IDENT); // variable name
    // Across blank lines only to the `from` it requires: `@for $i` / blank / `.z` is
    // missing it, before a rule.
    p.continue_line_to(TokenSet::new(&[IDENT]));

    // Once a part is missing where the statement ends, the parts after it are missing
    // with it, as one error: `@for $i from` / `.z`.
    let ended = |p: &Parser<'_>| {
        p.error_count() > errors && (p.at(SEMICOLON) || p.at(LBRACE) || p.at(RBRACE) || p.at_end())
    };

    // Expect `from`
    // Missing, `from` ends the header unless an expression follows, as in `@for $i 1 to 3`.
    let mut from = p.at(IDENT) && p.current_text() == "from";
    if from {
        p.bump();
    } else {
        p.error("expected `from`");
        from = p.at_ts(super::expressions::EXPR_START);
    }

    if from && !ended(p) {
        super::expressions::expr(p, super::ParseContext::SassScript);
        p.continue_line_across_blank();
    }

    // Expect `through` or `to`
    if !from || ended(p) {
    } else if p.at(IDENT) && (p.current_text() == "through" || p.current_text() == "to") {
        p.bump();
    } else {
        p.error("expected `through` or `to`");
    }

    if from && !ended(p) {
        super::expressions::expr(p, super::ParseContext::SassScript);
    }

    if p.at(LBRACE) {
        super::block(p);
    }
    let _ = m.complete(p, FOR_RULE);
}

/// `@each $var [, $var2] in expr { }`
pub fn each_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // each
    p.continue_header(&super::lines::EACH);
    p.expect(DOLLAR);
    p.expect(IDENT);
    p.continue_line_across_blank();

    // Additional destructured variables: `$key, $value`
    while p.eat(COMMA) {
        p.continue_line_across_blank();
        p.expect(DOLLAR);
        p.expect(IDENT);
        p.continue_line_across_blank();
    }

    // Expect `in`
    if p.at(IDENT) && p.current_text() == "in" {
        p.bump();
    } else {
        p.error("expected `in`");
    }

    // Parse the list: space-separated (`1px 2px 3px 4px`) or comma-separated (`foo, bar`).
    // Consume expressions until `{`.
    each_list(p);

    if p.at(LBRACE) {
        super::block(p);
    }
    let _ = m.complete(p, EACH_RULE);
}

/// Parse the expression list after `@each ... in`: space-separated or comma-separated.
/// Consumes until `{`, `}`, or EOF.
fn each_list(p: &mut Parser<'_>) {
    if p.at(LBRACE) || p.at(RBRACE) || p.at_end() || p.at(SEMICOLON) {
        p.error("expected expression");
        return;
    }
    super::expressions::expr(p, super::ParseContext::SassScript);
    loop {
        if p.at(LBRACE) || p.at(RBRACE) || p.at_end() || p.at(SEMICOLON) {
            break;
        }
        if p.at(COMMA) {
            p.bump();
        }
        if p.at(LBRACE) || p.at(RBRACE) || p.at_end() || p.at(SEMICOLON) {
            break;
        }
        if super::expressions::expr(p, super::ParseContext::SassScript).is_none() {
            break;
        }
    }
}

/// `@while condition { }`
pub fn while_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // while
    p.continue_header(&super::lines::EXPRESSION);
    super::expressions::expr(p, super::ParseContext::SassScript);
    if p.at(LBRACE) {
        super::block(p);
    }
    let _ = m.complete(p, WHILE_RULE);
}
