use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@mixin name(params) { body }` or `@mixin name { body }`
pub fn mixin_rule(p: &mut Parser<'_>) {
    let m = p.start();
    if bump_name(p) {
        p.continue_header(&super::lines::MIXIN);
    }
    p.expect(IDENT); // name
    if p.at(LPAREN) {
        super::param_list(p);
    }
    if p.at(LBRACE) {
        super::block(p);
    }
    let _ = m.complete(p, MIXIN_RULE);
}

/// `@include name(args) { content }` or `@include name;`
/// Also supports `@include name(args) using ($arg) { content }`
pub fn include_rule(p: &mut Parser<'_>) {
    let m = p.start();
    if bump_name(p) {
        p.continue_header(&super::lines::INCLUDE);
    }
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
    if p.at(IDENT) && p.current_text().eq_ignore_ascii_case("using") {
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
    p.continue_header(&super::lines::CONTENT);
    if p.at(LPAREN) {
        super::expressions::arg_list(p, super::ParseContext::SassScript);
    }
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, CONTENT_RULE);
}

/// The `@` and the name of `@mixin` or `@include`, or the `=` or `+` that the indented
/// syntax writes for them, which stands for both and reads as the name. Returns whether
/// the rule has its name: `@=` and `@+` are not names but stray operators.
fn bump_name(p: &mut Parser<'_>) -> bool {
    if p.at_shorthand() {
        p.bump_shorthand_at();
        p.bump_remap(IDENT);
        return true;
    }
    p.bump(); // @
    if p.at(IDENT) {
        p.bump(); // mixin / include
        true
    } else {
        super::lines::bump_in_run(p);
        false
    }
}
