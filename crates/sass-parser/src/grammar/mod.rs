pub(crate) mod at_rules;
mod declarations;
pub(crate) mod expressions;
mod lines;
pub(crate) mod selectors;

use crate::parser::{Joined, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// Parsing context — drives disambiguation for `/`, `%`, `min()`/`max()`, etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseContext {
    /// Inside variable declarations, function bodies, `@if` conditions, mixin args.
    /// `/` is division, `%` is modulo.
    SassScript,
    /// Inside CSS property values.
    /// `/` is a separator (unless heuristics say otherwise), `%` can be a unit.
    CssValue,
    /// Inside `calc()`, `min()`, `max()`, `clamp()`, etc.
    /// `/` is always division, Sass variables allowed, `%` modulo NOT allowed.
    Calculation,
    /// Inside `url()`, `element()`, `progid:...()`.
    /// Content is mostly opaque.
    SpecialFunction,
}

/// Parse `SourceFile` — top-level sequence of items with error recovery.
pub fn source_file(p: &mut Parser<'_>) {
    let m = p.start();
    while !p.at_end() {
        top_level_item(p);
    }
    let _ = m.complete(p, SOURCE_FILE);
}

/// Parse a single top-level item with error recovery.
fn top_level_item(p: &mut Parser<'_>) {
    if p.at(AT) || p.at_shorthand() {
        at_rules::at_rule(p);
    } else if p.at(DOLLAR) {
        expressions::variable_declaration(p);
    } else if at_namespace_var_assignment(p) {
        namespace_variable_assignment(p);
    } else if p.at_ts(selectors::SELECTOR_START) || p.at_ts(selectors::COMBINATOR_TOKEN) {
        rule_set(p);
    } else if p.at(SEMICOLON) {
        p.bump();
    } else {
        let m = p.start();
        p.error("expected rule");
        lines::bump_in_run(p);
        while !p.at_end()
            && !p.at(AT)
            && !p.at_shorthand()
            && !p.at_ts(selectors::SELECTOR_START)
            && !p.at_ts(selectors::COMBINATOR_TOKEN)
            && !p.at(SEMICOLON)
            && !p.at(DOLLAR)
        {
            lines::bump_in_run(p);
        }
        let _ = m.complete(p, SyntaxKind::ERROR);
    }
}

/// Reparse a sequence of block-body items (for incremental reparsing).
/// Wraps results in a `BLOCK` node (without `{` / `}`).
pub fn reparse_block_body(p: &mut Parser<'_>) {
    let m = p.start();
    while !p.at_end() {
        if p.at(SEMICOLON) {
            p.bump();
        } else {
            block_item(p);
        }
    }
    let _ = m.complete(p, BLOCK);
}

/// Check for `ns.$var: value` — namespace variable assignment.
fn at_namespace_var_assignment(p: &Parser<'_>) -> bool {
    p.at(IDENT)
        && p.nth(1) == DOT
        && !p.nth_has_whitespace_before(1)
        && p.nth(2) == DOLLAR
        && !p.nth_has_whitespace_before(2)
        && p.nth(3) == IDENT
        && p.nth(4) == COLON
}

/// Parse `ns.$var: value;` namespace variable assignment.
fn namespace_variable_assignment(p: &mut Parser<'_>) {
    let m = p.start();
    // Only a declaration that starts a statement goes on across lines.
    let statement = p.at_statement_start();
    if statement {
        p.continue_header(&lines::VARIABLE);
    }
    p.bump(); // IDENT (namespace)
    p.bump(); // DOT
    p.bump(); // DOLLAR
    p.expect(IDENT); // variable name
    if p.expect(COLON) && statement {
        p.continue_line();
    }
    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        expressions::sass_value_list(p, ParseContext::SassScript);
    }
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, VARIABLE_DECL);
}

/// Parse a single item inside a block.
/// Disambiguates between rule sets and declarations.
fn block_item(p: &mut Parser<'_>) {
    if p.at(AT) || p.at_shorthand() {
        at_rules::at_rule(p);
    } else if p.at(DOLLAR) {
        expressions::variable_declaration(p);
    } else if at_namespace_var_assignment(p) {
        namespace_variable_assignment(p);
    } else if looks_like_declaration(p) {
        declarations::declaration(p);
    } else if p.at_ts(selectors::SELECTOR_START) || p.at_ts(selectors::COMBINATOR_TOKEN) {
        rule_set(p);
    } else {
        lines::err_and_bump(p, "expected declaration or nested rule");
    }
}

/// Lookahead scan to decide if current position starts a declaration.
///
/// Recognizes:
/// - `IDENT COLON ...` — plain property, scan for `{`/`;`/`}`
/// - `IDENT COLON LBRACE` — nested property
/// - `HASH_LBRACE ... RBRACE [fragments] COLON` — interpolated property name
/// - `IDENT("--") HASH_LBRACE ... RBRACE [fragments] COLON` — custom property with interpolation
/// - any of these behind a property hack: `*width: 1px`, `.width: 1px`, `#width`, `:width`
///
/// The scan reads on across the line breaks the grammar will absorb (after a comma or an
/// operator), so that it sees the whole statement in the indented syntax too.
fn looks_like_declaration(p: &Parser<'_>) -> bool {
    let j = p.joined(lines::LINE_JOINS);
    let hack = matches!(p.current(), STAR | DOT | HASH | COLON)
        && matches!(p.nth_joined(j, 1), IDENT | HASH_LBRACE);
    let name = usize::from(hack);
    let colon = match p.nth_joined(j, name) {
        HASH_LBRACE => scan_past_interpolated_name(p, j, name),
        // `-#{"foo"}-bar: b` — property name starting with hyphen then interpolation
        MINUS if p.nth_joined(j, name + 1) == HASH_LBRACE => {
            scan_past_interpolated_name(p, j, name + 1)
        }
        // `margin-#{$side}: val`, `--#{$prefix}name: val`
        IDENT if p.nth_joined(j, name + 1) == HASH_LBRACE => {
            scan_past_interpolated_name(p, j, name + 1)
        }
        IDENT => Some(name + 1),
        _ => None,
    };
    let Some(colon) = colon.filter(|&colon| p.nth_joined(j, colon) == COLON) else {
        return false;
    };
    // 2.10: `name: {` → nested property
    if p.nth_joined(j, colon + 1) == LBRACE {
        return true;
    }
    // Dart reads `.a:b` as a selector in the indented syntax, a hack declaration in SCSS.
    if hack && !p.braced() && could_start_pseudo_class(p, j, colon + 1) {
        return false;
    }
    scan_for_declaration_end(p, j, colon + 1)
}

/// Scan past `HASH_LBRACE ... RBRACE [IDENT|MINUS|HASH_LBRACE...]` starting at `start`
/// (which must point at a `HASH_LBRACE`).
/// Returns the offset after the property name fragments, or `None` if scan exceeds budget.
fn scan_past_interpolated_name(p: &Parser<'_>, j: Joined, start: usize) -> Option<usize> {
    const MAX_SCAN: usize = 100;
    let limit = start + MAX_SCAN;
    let mut offset = start + 1; // past HASH_LBRACE
    let mut depth: u32 = 1;
    // Skip interpolation body
    loop {
        if offset >= limit {
            return None;
        }
        match p.nth_joined(j, offset) {
            EOF => return None,
            LBRACE | HASH_LBRACE => depth += 1,
            RBRACE => {
                depth -= 1;
                if depth == 0 {
                    offset += 1;
                    break;
                }
            }
            _ => {}
        }
        offset += 1;
    }
    // Skip trailing property fragments (IDENT, MINUS, more interpolations)
    loop {
        if offset >= limit {
            return None;
        }
        match p.nth_joined(j, offset) {
            IDENT | MINUS => offset += 1,
            HASH_LBRACE => {
                // Skip another interpolation
                offset += 1;
                let mut d: u32 = 1;
                loop {
                    if offset >= limit {
                        return None;
                    }
                    match p.nth_joined(j, offset) {
                        EOF => return None,
                        LBRACE | HASH_LBRACE => d += 1,
                        RBRACE => {
                            d -= 1;
                            if d == 0 {
                                offset += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    offset += 1;
                }
            }
            _ => break,
        }
    }
    Some(offset)
}

/// Decide whether `name:` followed by the tokens at `start` is a declaration.
///
/// Follows Dart Sass: the colon can begin a pseudo-class only when an identifier or
/// interpolation follows it with no whitespace (`a:hover`, `a:#{$p}`). Anything else —
/// `font: bold { … }`, `background: url(x) { … }`, `font:12px { … }` — is a declaration,
/// with nested children if a `{` follows its value.
///
/// Otherwise, scan for `{`, `;`, or `}` at depth 0: `;`/`}`/EOF first means a declaration,
/// `{` first a selector — unless an unambiguously-value token (`NUMBER`, `QUOTED_STRING`,
/// `DOLLAR`, etc.) precedes the `{`. A declaration whose value Dart cannot parse is a
/// selector after all, so a selector-only token (see `is_selector_only_token`) before
/// `;`/`}`/EOF makes it one: `div:hover .x` is a rule missing its block. Scan is bounded to
/// avoid O(n²) behavior on pathological inputs.
fn scan_for_declaration_end(p: &Parser<'_>, j: Joined, start: usize) -> bool {
    const MAX_SCAN: usize = 100;
    if !could_start_pseudo_class(p, j, start) || is_progid(p, j, start) {
        return true;
    }
    let mut depth: u32 = 0;
    let mut brace_depth: u32 = 0;
    let mut offset = start;
    let limit = start + MAX_SCAN;
    // Track whether we've seen a token that can only appear in a value, not a selector.
    // Disambiguates `p:hover { }` (selector) from `margin: 0 { top: 20px; }` (declaration).
    // Only counted at bracket depth 0 — see the match arm below.
    let mut has_value_only_token = false;
    let mut has_selector_only_token = false;
    loop {
        if offset >= limit {
            // Exceeded scan budget — guess declaration to avoid misparse as selector
            return true;
        }
        // Inside interpolation #{...} — skip until matching RBRACE
        if brace_depth > 0 {
            match p.nth_joined(j, offset) {
                EOF => return true,
                HASH_LBRACE | LBRACE => brace_depth += 1,
                RBRACE => brace_depth -= 1,
                _ => {}
            }
            offset += 1;
            continue;
        }
        match p.nth_joined(j, offset) {
            EOF => return !has_selector_only_token,
            HASH_LBRACE => brace_depth += 1,
            LBRACE if depth == 0 => return has_value_only_token,
            RBRACE | SEMICOLON if depth == 0 => return !has_selector_only_token,
            _ if is_selector_only_token(p, j, offset, depth) => has_selector_only_token = true,
            LPAREN | LBRACKET => depth += 1,
            RPAREN | RBRACKET => depth = depth.saturating_sub(1),
            // Tokens that can only appear in values, never in selectors — but only
            // at bracket depth 0. Inside `(...)` and `[...]` they occur in selectors
            // too: `[type="b"]`, `:nth-child(2)`, `:not([hidden])`.
            NUMBER | QUOTED_STRING | STRING_START | DOLLAR | UNICODE_RANGE if depth == 0 => {
                has_value_only_token = true;
            }
            _ => {}
        }
        offset += 1;
    }
}

/// Whether the token at `offset`, `depth` brackets deep, has no place in a declaration
/// value but does in a selector: a `.` that does not join two names, as in `math.div`,
/// `map.$x` or IE's `expression(this.value)`, or a `:`, `::` or `~` outside brackets.
fn is_selector_only_token(p: &Parser<'_>, j: Joined, offset: usize, depth: u32) -> bool {
    match p.nth_joined(j, offset) {
        DOT => {
            let joins_names = offset > 0
                && p.nth_joined(j, offset - 1) == IDENT
                && !p.nth_has_whitespace_before_joined(j, offset)
                && matches!(p.nth_joined(j, offset + 1), IDENT | DOLLAR)
                && !p.nth_has_whitespace_before_joined(j, offset + 1);
            !joins_names
        }
        COLON | COLON_COLON | TILDE => depth == 0,
        _ => false,
    }
}

/// Whether a value starts at `offset` with IE's `progid:`, whose `:` and dotted names Dart
/// reads as part of the value: `filter:progid:DXImageTransform.Microsoft.Alpha(…)`.
fn is_progid(p: &Parser<'_>, j: Joined, offset: usize) -> bool {
    p.nth_joined(j, offset) == IDENT
        && p.nth_text_joined(j, offset).eq_ignore_ascii_case("progid")
        && p.nth_joined(j, offset + 1) == COLON
}

/// Whether the token at `offset`, right after a colon, could be a pseudo-class name.
fn could_start_pseudo_class(p: &Parser<'_>, j: Joined, offset: usize) -> bool {
    if p.nth_has_whitespace_before_joined(j, offset) {
        return false;
    }
    match p.nth_joined(j, offset) {
        IDENT | HASH_LBRACE => true,
        MINUS => {
            matches!(p.nth_joined(j, offset + 1), IDENT | HASH_LBRACE)
                && !p.nth_has_whitespace_before_joined(j, offset + 1)
        }
        _ => false,
    }
}

/// 2.5: Parse rule set — selector list + `{` block `}`.
fn rule_set(p: &mut Parser<'_>) {
    let Ok(mut g) = p.depth_guard() else {
        // Skip past this construct to prevent infinite loops at depth limit
        skip_until_block_end(p);
        return;
    };
    let m = g.start();
    let errors = g.error_count();
    selectors::selector_list(&mut g);
    if g.at(LBRACE) {
        block(&mut g);
    } else if g.braced() {
        let selector_reported = g.error_count() > errors;
        missing_rule_block(&mut g, selector_reported);
    }
    let _ = m.complete(&mut g, RULE_SET);
}

/// Report a braced rule's missing `{`. Where the statement ends, or the next one starts,
/// the error goes right after the selector, unless the selector has reported one of its
/// own already, such as an unclosed `(`. Tokens the selector stopped at are skipped as
/// one error up to the next `{`, `;` or `}`, and a block there is still the rule's.
fn missing_rule_block(p: &mut Parser<'_>, selector_reported: bool) {
    // The selector consumes every selector-start token, unless one begins a declaration
    // (see `declaration_interrupts_selector`): a statement of its own.
    if p.at_end()
        || p.at(SEMICOLON)
        || p.at(RBRACE)
        || p.at(AT)
        || p.at(DOLLAR)
        || p.at_ts(selectors::SELECTOR_START)
    {
        if !selector_reported {
            p.error_after_prev("expected `{`");
        }
        return;
    }
    skip_to_rule_block(p, true);
}

/// Whether the cursor is at an identifier as Dart Sass's `interpolatedIdentifier` reads
/// one: a word or an interpolation, or a `-` joined to one.
pub(crate) fn at_interpolated_identifier(p: &Parser<'_>) -> bool {
    match p.current() {
        IDENT | HASH_LBRACE => true,
        MINUS => !p.nth_has_whitespace_before(1) && matches!(p.nth(1), IDENT | HASH_LBRACE),
        _ => false,
    }
}

/// An identifier as Dart Sass's `interpolatedIdentifier` reads one: words,
/// interpolations, `-` and digits with no whitespace between them, as in `-#{$a}-b`,
/// `#{$a}-` or `x#{$b}-2`. A number with a `.` or an exponent is no part of a name.
pub(crate) fn interpolated_identifier(p: &mut Parser<'_>) {
    debug_assert!(at_interpolated_identifier(p));
    loop {
        if p.at(HASH_LBRACE) {
            let _ = expressions::interpolation(p);
        } else {
            p.bump();
        }
        let digits = p.at(NUMBER) && p.current_text().bytes().all(|b| b.is_ascii_digit());
        let piece = matches!(p.current(), IDENT | HASH_LBRACE | MINUS) || digits;
        if p.has_whitespace_before() || !piece {
            break;
        }
    }
}

/// Report the tokens at the cursor, where a `{` belongs, if `report`, and skip them as one
/// error up to the next `{`, `;` or `}`, or the line break of a string left unclosed on a
/// line below; a block there is still the rule's. A string that swallowed the block's `{` on the line
/// the tokens start on is that one mistake, reported as the string: the `{` it took is
/// put back where the indentation below agrees, and opens the block.
pub(crate) fn skip_to_rule_block(p: &mut Parser<'_>, report: bool) {
    let m = p.start();
    let at = p.error_range();
    let strings = p.unclosed_strings();
    let mut first = true;
    let mut same_line = true;
    let mut interpolation: u32 = 0;
    // A string left unclosed on a line below ends at its line break the statement it
    // starts; on the first line it is more of the mistake.
    while !p.at_end() && (same_line || !p.at_unclosed_string_break()) {
        match p.current() {
            LBRACE | RBRACE | SEMICOLON if interpolation == 0 => break,
            HASH_LBRACE | LBRACE => interpolation += 1,
            RBRACE => interpolation -= 1,
            _ => {}
        }
        same_line &= first || !p.at_line_break();
        first = false;
        if p.at(STRING_START) {
            let _ = expressions::interpolated_string(p, ParseContext::CssValue);
        } else {
            p.bump();
        }
    }
    let took_block = p.unclosed_strings() > strings && same_line && p.at_virtual() && p.at(LBRACE);
    if report && !took_block {
        p.error_at_start(&m, at, "expected `{`");
    }
    let _ = m.complete(p, SyntaxKind::ERROR);
    if p.at(LBRACE) {
        block(p);
    }
}

/// In a braced selector, whether the compound at the cursor starts a declaration instead:
/// `name:` where the colon cannot begin a pseudo-class, or whose statement ends at `;` or
/// `}` before any `{` with nothing only a selector holds. Only a rule missing its `{` gets
/// here with one, as in `.a { .b  color: red; }`; ending the selector before it keeps the
/// declaration.
pub(crate) fn declaration_interrupts_selector(p: &Parser<'_>) -> bool {
    const MAX_SCAN: usize = 100;
    if !p.at(IDENT) || p.nth(1) != COLON || p.nth_has_whitespace_before(1) {
        return false;
    }
    let j = p.joined(lines::LINE_JOINS);
    if !could_start_pseudo_class(p, j, 2) || is_progid(p, j, 2) {
        return true;
    }
    // Budget and EOF read as a selector: a valid one must not be cut short.
    let mut depth: u32 = 0;
    let mut interpolation: u32 = 0;
    let mut has_selector_only_token = false;
    for offset in 2..MAX_SCAN {
        let kind = p.nth_joined(j, offset);
        if interpolation > 0 {
            match kind {
                EOF => return false,
                HASH_LBRACE | LBRACE => interpolation += 1,
                RBRACE => interpolation -= 1,
                _ => {}
            }
            continue;
        }
        match kind {
            EOF => return false,
            HASH_LBRACE => interpolation += 1,
            LBRACE if depth == 0 => return false,
            SEMICOLON | RBRACE if depth == 0 => return !has_selector_only_token,
            _ if is_selector_only_token(p, j, offset, depth) => has_selector_only_token = true,
            LPAREN | LBRACKET | LBRACE => depth += 1,
            RPAREN | RBRACKET | RBRACE => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

/// Skip tokens until `}` or EOF, consuming a balanced `{ ... }` block if present.
/// Wraps all skipped tokens in an ERROR node.
fn skip_until_block_end(p: &mut Parser<'_>) {
    let m = p.start();
    let mut depth: u32 = 0;
    let mut consumed = false;
    while !p.at_end() {
        match p.current() {
            LBRACE => {
                depth += 1;
                p.bump();
                consumed = true;
            }
            RBRACE => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                p.bump();
            }
            _ => {
                p.bump();
                consumed = true;
            }
        }
    }
    if consumed {
        let _ = m.complete(p, SyntaxKind::ERROR);
    } else {
        m.abandon(p);
    }
}

/// Consume `open ... close`, treating content as opaque. Braced, a `{` or `}` outside
/// interpolation ends a bracket left unclosed: no selector argument or at-rule prelude
/// holds one, and the rules after it stay intact. The indented layout ends it at a line
/// instead: see [`Parser::end_unclosed_bracket`].
pub(crate) fn eat_balanced(p: &mut Parser<'_>, open: SyntaxKind, close: SyntaxKind) {
    assert!(p.at(open));
    let p = &mut *p.transparent_guard();
    let braced = p.braced();
    p.end_unclosed_bracket(open, close);
    p.bump();
    let mut depth: u32 = 1;
    let mut interpolation: u32 = 0;
    // A string left unclosed took the rest of its line, the closing bracket and all. A
    // zero-width token comes where a closer of another kind took the bracket's place.
    while !p.at_end() && depth > 0 && !p.at_unclosed_string_break() && !p.at_virtual() {
        let kind = p.current();
        if braced {
            match kind {
                HASH_LBRACE => interpolation += 1,
                LBRACE | RBRACE if interpolation == 0 => break,
                LBRACE => interpolation += 1,
                RBRACE => interpolation -= 1,
                _ => {}
            }
        }
        if kind == open {
            depth += 1;
        } else if kind == close {
            depth -= 1;
        }
        p.bump();
    }
    if depth > 0 && !p.at_unclosed_string_break() {
        report_unclosed(p, close);
    }
}

/// Report a bracket left unclosed, at the end of its contents. At a `{` the rule's block
/// starts: that is where the bracket is found unclosed.
pub(crate) fn report_unclosed(p: &mut Parser<'_>, close: SyntaxKind) {
    let msg = format!("expected {close:?}");
    if p.braced() && p.at(LBRACE) {
        p.error(msg);
    } else {
        p.error_after_prev(msg);
    }
}

/// Parse a `{ ... }` block containing declarations and/or nested rules.
pub(crate) fn block(p: &mut Parser<'_>) {
    let Ok(mut g) = p.depth_guard() else {
        // Skip past the block to prevent infinite loops at depth limit
        skip_until_block_end(p);
        return;
    };
    debug_assert!(g.at(LBRACE));
    let m = g.start();
    g.bump(); // {
    while !g.at(RBRACE) && !g.at_end() {
        if g.at(SEMICOLON) {
            g.bump();
        } else if g.at(AT)
            || g.at(DOLLAR)
            || g.at(MINUS)
            || g.at_ts(selectors::SELECTOR_START)
            || g.at_ts(selectors::COMBINATOR_TOKEN)
            || g.at_shorthand()
        {
            block_item(&mut g);
        } else {
            lines::err_and_bump(&mut g, "expected declaration or nested rule");
        }
    }
    g.expect(RBRACE);
    let _ = m.complete(&mut g, BLOCK);
}

/// Parse a `{ ... }` block for unknown/interpolated at-rules.
///
/// More permissive than `block()`: also accepts `NUMBER [%] { ... }` constructs
/// (keyframe selectors) since the at-rule name may resolve to `@keyframes` at runtime.
pub(crate) fn generic_at_rule_block(p: &mut Parser<'_>) {
    let Ok(mut g) = p.depth_guard() else {
        skip_until_block_end(p);
        return;
    };
    debug_assert!(g.at(LBRACE));
    let m = g.start();
    g.bump(); // {
    while !g.at(RBRACE) && !g.at_end() {
        if g.at(SEMICOLON) {
            g.bump();
        } else if g.at(AT)
            || g.at(DOLLAR)
            || g.at(MINUS)
            || g.at_ts(selectors::SELECTOR_START)
            || g.at_ts(selectors::COMBINATOR_TOKEN)
            || g.at_shorthand()
        {
            block_item(&mut g);
        } else if g.at(NUMBER) {
            // Keyframe selector: `10% { ... }`, `from { ... }`
            generic_keyframe_block(&mut g);
        } else {
            lines::err_and_bump(&mut g, "expected declaration or nested rule");
        }
    }
    g.expect(RBRACE);
    let _ = m.complete(&mut g, BLOCK);
}

/// Parse a keyframe-like block inside a generic at-rule: `10% { ... }`, `50%, 75% { ... }`.
fn generic_keyframe_block(p: &mut Parser<'_>) {
    let m = p.start();
    // Consume keyframe selectors: NUMBER [%] [, NUMBER [%]]*
    loop {
        if p.at(NUMBER) {
            p.bump();
            if p.at(PERCENT) && !p.has_whitespace_before() {
                p.bump();
            }
        } else if p.at(IDENT) {
            p.bump();
        } else if p.at(HASH_LBRACE) {
            let _ = expressions::interpolation(p);
        } else {
            break;
        }
        if !p.eat(COMMA) {
            break;
        }
        p.continue_line_across_blank();
    }
    if p.at(LBRACE) {
        block(p);
    }
    let _ = m.complete(p, KEYFRAME_SELECTOR);
}

/// Parse a `{ ... }` block for CSS `@function --name()` bodies.
///
/// Declarations inside CSS function bodies use raw CSS values (like custom properties),
/// not Sass expression parsing. This avoids errors on raw CSS tokens like `{}#&%^*`.
pub(crate) fn css_function_block(p: &mut Parser<'_>) {
    let Ok(mut g) = p.depth_guard() else {
        skip_until_block_end(p);
        return;
    };
    debug_assert!(g.at(LBRACE));
    let m = g.start();
    g.bump(); // {
    while !g.at(RBRACE) && !g.at_end() {
        if g.at(SEMICOLON) {
            g.bump();
        } else if g.at(AT) || g.at_shorthand() {
            at_rules::at_rule(&mut g);
        } else if g.at(IDENT) || g.at(HASH_LBRACE) || g.at(MINUS) || g.at(STAR) {
            declarations::css_function_body_declaration(&mut g);
        } else {
            lines::err_and_bump(&mut g, "expected declaration");
        }
    }
    g.expect(RBRACE);
    let _ = m.complete(&mut g, BLOCK);
}
