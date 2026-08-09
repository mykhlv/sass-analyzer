use crate::lexer;
use crate::syntax_kind::SyntaxKind;

/// Transform SCSS tokens into a Sass-compatible token stream.
///
/// Wraps the SCSS lexer and inserts virtual (zero-width) `LBRACE`/`RBRACE`/`SEMICOLON`
/// tokens based on indentation and newlines. The resulting token stream can be fed
/// directly into `Input::from_tokens()` and then into the existing parser unchanged.
///
/// # Indentation rules
///
/// - Indentation increase after content → virtual `{` (block open)
/// - Same indentation after content → virtual `;` (statement end)
/// - Indentation decrease → virtual `;` + virtual `}` per closed level
/// - Inside `()`, `[]`, or `#{}` → newlines are not significant
/// - Continuation token at end of line → no virtual tokens
///
/// # Sass shorthands
///
/// - `=name` → `@mixin name` (virtual `AT` + re-tagged IDENT)
/// - `+name` → `@include name` (virtual `AT` + re-tagged IDENT, only when no space before name)
pub fn sass_tokenize(source: &str) -> Vec<(SyntaxKind, &str)> {
    let raw = lexer::tokenize(source);
    let mut result = Vec::with_capacity(raw.len() + raw.len() / 4);
    let mut indent_stack: Vec<u32> = vec![0];
    let mut saw_content = false;
    let mut last_sig_kind = SyntaxKind::EOF;
    let mut last_sig_text: &str = "";
    let mut prev_sig_kind = SyntaxKind::EOF;
    // Combined nesting depth for (), [], and #{}
    let mut nesting: u32 = 0;
    // Pending virtual LBRACE: Some(indent) when we've deferred a block-open decision.
    let mut pending_lbrace: Option<u32> = None;
    // True when AT has been seen on the current statement (reset on virtual SEMICOLON/LBRACE).
    let mut after_at = false;
    // True when DOLLAR has been seen at statement start (for `$var\n  : value` continuation).
    let mut after_dollar = false;
    // True when a top-level COLON has been seen on the current statement. Used as an
    // approximation of "we are in a value", to decide whether a trailing `*` is
    // arithmetic or a universal selector. Deliberately over-approximates: a pseudo-class
    // colon sets it too, which is the `.a:hover > *` limitation pinned in the tests.
    let mut after_colon = false;
    // True when the last significant token is the directive keyword name (first IDENT after AT).
    // Used for end-of-line continuation: `@mixin\n name` but NOT `@extend .error\n body`.
    let mut at_name_is_last = false;
    // Name of the directive being parsed (the first IDENT after AT), or "" when unknown.
    // Decides which indented lines continue the header — see `continues_header`.
    let mut at_directive: &str = "";
    // Indentation of the current line (updated on each newline-containing WHITESPACE).
    let mut current_line_indent: u32 = 0;
    // True when there was content on this line before the last significant token.
    // Used to distinguish `*` as sole selector vs `2 *` as operator continuation.
    let mut had_prior_content = false;

    let mut i = 0;
    while i < raw.len() {
        let (kind, text) = raw[i];
        let prior_content = saw_content;

        // Unterminated block comments are valid in indented Sass (close at dedent/EOF).
        let kind = if kind == SyntaxKind::ERROR && text.starts_with("/*") {
            SyntaxKind::MULTI_LINE_COMMENT
        } else {
            kind
        };

        // Resolve any pending LBRACE before processing a non-trivia token.
        if let Some(pending_indent) = pending_lbrace
            && !kind.is_trivia()
        {
            if continues_header(
                kind,
                text,
                after_at,
                after_dollar,
                at_name_is_last,
                at_directive,
            ) {
                // Cancel the pending LBRACE — this line continues the header.
                pending_lbrace = None;
            } else {
                // Emit the deferred LBRACE.
                result.push((SyntaxKind::LBRACE, ""));
                indent_stack.push(pending_indent);
                pending_lbrace = None;
                after_at = false;
                after_dollar = false;
                after_colon = false;
            }
        }

        // Save previous significant kind before the match overwrites last_sig_kind.
        if !kind.is_trivia() {
            prev_sig_kind = last_sig_kind;
        }

        match kind {
            SyntaxKind::LPAREN | SyntaxKind::LBRACKET | SyntaxKind::HASH_LBRACE => {
                nesting += 1;
                result.push((kind, text));
                saw_content = true;
                last_sig_kind = kind;
                last_sig_text = text;
                at_name_is_last = false;
            }
            SyntaxKind::RPAREN | SyntaxKind::RBRACKET => {
                nesting = nesting.saturating_sub(1);
                result.push((kind, text));
                saw_content = true;
                last_sig_kind = kind;
                last_sig_text = text;
                at_name_is_last = false;
            }
            SyntaxKind::RBRACE if nesting > 0 => {
                nesting -= 1;
                result.push((kind, text));
                saw_content = true;
                last_sig_kind = kind;
                last_sig_text = text;
                at_name_is_last = false;
            }
            SyntaxKind::LBRACE if nesting > 0 => {
                nesting += 1;
                result.push((kind, text));
                saw_content = true;
                last_sig_kind = kind;
                last_sig_text = text;
                at_name_is_last = false;
            }
            SyntaxKind::WHITESPACE if nesting == 0 && contains_newline(text) => {
                current_line_indent = measure_indent_after_last_newline(text);
                process_newline(
                    text,
                    &mut result,
                    &mut indent_stack,
                    &mut saw_content,
                    last_sig_kind,
                    last_sig_text,
                    prev_sig_kind,
                    &mut pending_lbrace,
                    &mut after_at,
                    &mut after_dollar,
                    &mut after_colon,
                    at_name_is_last,
                    at_directive,
                    had_prior_content,
                );
            }
            // Sass shorthand: `=` at statement start → @mixin
            SyntaxKind::EQ if nesting == 0 && !saw_content => {
                result.push((SyntaxKind::AT, ""));
                result.push((SyntaxKind::IDENT, text));
                saw_content = true;
                last_sig_kind = SyntaxKind::IDENT;
                last_sig_text = text;
                after_at = true;
                at_name_is_last = true;
                at_directive = "mixin";
            }
            // Sass shorthand: `+name` at statement start → @include
            SyntaxKind::PLUS if nesting == 0 && !saw_content => {
                if i + 1 < raw.len() && raw[i + 1].0 == SyntaxKind::IDENT {
                    result.push((SyntaxKind::AT, ""));
                    result.push((SyntaxKind::IDENT, text));
                    saw_content = true;
                    last_sig_kind = SyntaxKind::IDENT;
                    last_sig_text = text;
                    after_at = true;
                    at_name_is_last = true;
                    at_directive = "include";
                } else {
                    result.push((kind, text));
                    saw_content = true;
                    last_sig_kind = kind;
                    last_sig_text = text;
                }
            }
            SyntaxKind::AT if nesting == 0 => {
                result.push((kind, text));
                saw_content = true;
                last_sig_kind = kind;
                last_sig_text = text;
                after_at = true;
                at_directive = "";
            }
            SyntaxKind::DOLLAR if nesting == 0 && !saw_content => {
                result.push((kind, text));
                saw_content = true;
                last_sig_kind = kind;
                last_sig_text = text;
                after_dollar = true;
            }
            // In indented Sass, standalone // comments extend to subsequent indented lines.
            // Only applies when the comment is the sole content on its line (!saw_content).
            SyntaxKind::SINGLE_LINE_COMMENT if nesting == 0 && !saw_content => {
                result.push((kind, text));
                let base = current_line_indent;
                while i + 1 < raw.len() {
                    let (nk, nt) = raw[i + 1];
                    if nk == SyntaxKind::WHITESPACE && contains_newline(nt) {
                        // A blank line (2+ newlines) breaks comment continuation.
                        let newlines = nt
                            .bytes()
                            .filter(|&b| b == b'\n' || b == b'\r' || b == b'\x0c')
                            .count();
                        let next_indent = measure_indent_after_last_newline(nt);
                        if newlines == 1 && next_indent > base {
                            // Continuation line — emit whitespace, then retag line tokens.
                            i += 1;
                            result.push((SyntaxKind::WHITESPACE, nt));
                            while i + 1 < raw.len() {
                                let (tk, tt) = raw[i + 1];
                                if tk == SyntaxKind::WHITESPACE && contains_newline(tt) {
                                    break;
                                }
                                i += 1;
                                result.push((SyntaxKind::SINGLE_LINE_COMMENT, tt));
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            _ if kind.is_trivia() => {
                result.push((kind, text));
            }
            _ => {
                result.push((kind, text));
                saw_content = true;
                // `::` lexes as a single COLON_COLON, so pseudo-elements never land here
                // — splitting it into two COLON tokens would widen the pseudo-class
                // limitation to every `::before > *`.
                if kind == SyntaxKind::COLON && nesting == 0 {
                    after_colon = true;
                }
                // Track when this IDENT is the directive keyword name (first IDENT after AT).
                at_name_is_last = kind == SyntaxKind::IDENT && last_sig_kind == SyntaxKind::AT;
                if at_name_is_last {
                    at_directive = text;
                }
                last_sig_kind = kind;
                last_sig_text = text;
            }
        }
        // Track whether the last significant token had prior content on its line.
        if !kind.is_trivia() {
            had_prior_content = prior_content;
        }
        i += 1;
    }

    // Flush pending LBRACE at EOF
    if let Some(indent) = pending_lbrace {
        result.push((SyntaxKind::LBRACE, ""));
        indent_stack.push(indent);
    }

    // Close remaining open blocks at EOF
    if saw_content {
        result.push((SyntaxKind::SEMICOLON, ""));
    }
    while indent_stack.len() > 1 {
        indent_stack.pop();
        result.push((SyntaxKind::RBRACE, ""));
    }

    result
}

fn is_continuation_keyword(text: &str) -> bool {
    matches!(
        text,
        "from"
            | "through"
            | "to"
            | "in"
            | "and"
            | "or"
            | "not"
            | "as"
            | "with"
            | "show"
            | "hide"
            | "using"
            | "if"
    )
}

/// Check if a token at end-of-line indicates the statement continues on the next line.
fn is_end_of_line_continuation(
    kind: SyntaxKind,
    text: &str,
    at_name_is_last: bool,
    after_at: bool,
    at_directive: &str,
    after_colon: bool,
) -> bool {
    // `*` is a binary operator only where an expression can appear: after a top-level
    // colon (`$a: 1 *`, `width: 2px *`) or in a directive header that takes an
    // expression (`@return $x *`, `@if $a *`). Everywhere else it is a universal
    // selector — `> *`, `& ~ *`, `.a *` — and treating it as a continuation swallows
    // the next line into the selector. `is_sole_combinator` is not enough: it exempts
    // only a line whose entire content is `*`, so `> *` still fell through.
    // This also covers the `@use`/`@forward` wildcard (`as *`, `as prefix-*`): neither
    // takes an expression header, and their parenthesised `with` config never reaches
    // here because newlines inside `()` are not processed.
    if kind == SyntaxKind::STAR {
        return after_colon || (after_at && is_directive(at_directive, EXPR_DIRECTIVES));
    }
    #[rustfmt::skip]
    let by_kind = matches!(
        kind,
        SyntaxKind::COMMA   | SyntaxKind::COLON   | SyntaxKind::BANG
        | SyntaxKind::PLUS  | SyntaxKind::MINUS
        | SyntaxKind::SLASH | SyntaxKind::PERCENT
        | SyntaxKind::EQ_EQ | SyntaxKind::BANG_EQ
        | SyntaxKind::GT_EQ | SyntaxKind::LT_EQ
        | SyntaxKind::GT    | SyntaxKind::LT      | SyntaxKind::TILDE
    );
    if by_kind {
        return true;
    }
    if kind == SyntaxKind::IDENT {
        // Expression/module keywords — always continue.
        if is_continuation_keyword(text) {
            return true;
        }
        // Directive name keywords — continue only when the directive name is
        // at EOL. Covers `@mixin\n name`, `@debug\n expr`, `=\n name`, etc.
        if at_name_is_last {
            return matches!(
                text,
                "mixin"
                    | "include"
                    | "function"
                    | "while"
                    | "for"
                    | "each"
                    | "debug"
                    | "warn"
                    | "error"
                    | "return"
                    | "extend"
                    | "at-root"
                    | "="
            );
        }
    }
    false
}

/// Check if the first non-trivia token on a new indented line indicates
/// continuation of an at-rule header (rather than block body).
fn is_start_of_line_continuation(kind: SyntaxKind, text: &str) -> bool {
    match kind {
        // Variable references in directive headers: `@for $i`, `@each $item`
        SyntaxKind::DOLLAR => true,
        // Parenthesized params/args: `@function name\n  (params)`
        SyntaxKind::LPAREN => true,
        // Quoted URL: `@use\n  "url"`, `@forward\n  "url"`
        SyntaxKind::QUOTED_STRING | SyntaxKind::STRING_START => true,
        // Comma continuation: `@each $a\n  , $b in list`
        SyntaxKind::COMMA => true,
        // Colon continuation: `$a\n  : value`
        SyntaxKind::COLON => true,
        // Directive header keywords
        SyntaxKind::IDENT => is_continuation_keyword(text),
        // Expressions: numbers, booleans, strings in @debug/@warn/@return
        SyntaxKind::NUMBER => true,
        _ => false,
    }
}

/// Directives whose header is an expression or condition, so a bare value on the
/// next line still belongs to the header (`@for\n  $i`, `@debug\n  1 + 2`).
const EXPR_DIRECTIVES: &[&str] = &[
    "if", "else", "while", "return", "debug", "warn", "error", "for", "each",
];

/// Directives whose header can take a parenthesised parameter, argument or query
/// list (`@function a\n  ()`, `@supports\n  (display: grid)`).
const PAREN_DIRECTIVES: &[&str] = &[
    "if", "else", "while", "return", "debug", "warn", "error", "for", "each", "mixin", "include",
    "function", "content", "media", "supports", "at-root", "use", "forward", "import",
];

/// Directives whose header is a comma-separated list (`@each $a\n  , $b in ...`).
const COMMA_DIRECTIVES: &[&str] = &[
    "if", "else", "while", "return", "debug", "warn", "error", "for", "each", "mixin", "include",
    "function", "media", "supports", "use", "forward", "import", "extend",
];

/// Directives whose header can take a quoted string (`@use\n  "url"`).
const STRING_DIRECTIVES: &[&str] = &[
    "if", "else", "while", "return", "debug", "warn", "error", "for", "each", "use", "forward",
    "import", "charset",
];

/// Directives whose header contains boolean/query operators (`and`, `or`, `not`).
const OPERATOR_DIRECTIVES: &[&str] = &[
    "if", "else", "while", "return", "debug", "warn", "error", "for", "each", "media", "supports",
];

/// Directives that load a module and accept the `as`/`with`/`show`/`hide` clauses.
const MODULE_DIRECTIVES: &[&str] = &["use", "forward", "import"];

fn is_directive(directive: &str, names: &[&str]) -> bool {
    let name = strip_vendor_prefix(directive);
    names.iter().any(|n| name.eq_ignore_ascii_case(n))
}

/// `@-webkit-keyframes` behaves exactly like `@keyframes`.
fn strip_vendor_prefix(name: &str) -> &str {
    if let Some(rest) = name.strip_prefix('-')
        && let Some(pos) = rest.find('-')
    {
        return &rest[pos + 1..];
    }
    name
}

/// Whether a continuation keyword at the start of an indented line belongs to the
/// header of `directive` rather than opening its body.
fn keyword_continues_header(text: &str, directive: &str) -> bool {
    match text {
        "from" | "through" | "to" => is_directive(directive, &["for"]),
        "in" => is_directive(directive, &["each"]),
        "and" | "or" | "not" => is_directive(directive, OPERATOR_DIRECTIVES),
        "as" | "with" | "show" | "hide" => is_directive(directive, MODULE_DIRECTIVES),
        "using" => is_directive(directive, &["include"]),
        // `@else\n  if $x`, and the `if()` function inside a header expression.
        "if" => is_directive(directive, EXPR_DIRECTIVES),
        _ => false,
    }
}

/// Whether an indented line continues an at-rule/variable header instead of opening
/// its body.
///
/// Which starters continue a header depends on *which* directive is being parsed, so
/// the decision cannot be made from the token alone. `from`/`through`/`to` continue a
/// header only under `@for`, `in` only under `@each`, `and`/`or`/`not` only under the
/// condition- and query-taking directives. Under `@keyframes` those very same `from`,
/// `to` and `0%` tokens are keyframe selectors that open the body, and a leading `:`
/// under any at-rule is a pseudo-selector starting a nested rule, never a header
/// continuation.
///
/// `$` is additionally ambiguous within one directive: `@for\n  $i` continues the
/// header while `@function foo($bg)\n  $x: 1` starts the body, so it also requires the
/// directive name to be the last significant token.
///
/// Unrecognised directives continue nothing. Treating a header continuation as a body
/// opener produces a visible parse error; the reverse silently reparents the block's
/// contents into the enclosing scope, so ambiguity resolves towards opening the body.
fn continues_header(
    kind: SyntaxKind,
    text: &str,
    after_at: bool,
    after_dollar: bool,
    at_name_is_last: bool,
    directive: &str,
) -> bool {
    if !is_start_of_line_continuation(kind, text) {
        return false;
    }
    if after_dollar {
        return true;
    }
    if !after_at {
        return false;
    }
    match kind {
        SyntaxKind::COLON => false,
        SyntaxKind::DOLLAR => at_name_is_last && is_directive(directive, EXPR_DIRECTIVES),
        SyntaxKind::LPAREN => is_directive(directive, PAREN_DIRECTIVES),
        SyntaxKind::COMMA => is_directive(directive, COMMA_DIRECTIVES),
        SyntaxKind::QUOTED_STRING | SyntaxKind::STRING_START => {
            is_directive(directive, STRING_DIRECTIVES)
        }
        SyntaxKind::NUMBER => is_directive(directive, EXPR_DIRECTIVES),
        SyntaxKind::IDENT => keyword_continues_header(text, directive),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn process_newline<'src>(
    ws_text: &'src str,
    result: &mut Vec<(SyntaxKind, &'src str)>,
    indent_stack: &mut Vec<u32>,
    saw_content: &mut bool,
    last_sig_kind: SyntaxKind,
    last_sig_text: &str,
    prev_sig_kind: SyntaxKind,
    pending_lbrace: &mut Option<u32>,
    after_at: &mut bool,
    after_dollar: &mut bool,
    after_colon: &mut bool,
    at_name_is_last: bool,
    at_directive: &str,
    had_prior_content: bool,
) {
    let new_indent = measure_indent_after_last_newline(ws_text);
    let current_indent = *indent_stack.last().unwrap_or(&0);

    // Continuation token at end of line → statement continues on the next line.
    // Exception: combinator tokens (*, ~, +, >) as the sole content on a line
    // are selectors, not operators — don't treat them as continuation.
    let mut is_continuation = is_end_of_line_continuation(
        last_sig_kind,
        last_sig_text,
        at_name_is_last,
        *after_at,
        at_directive,
        *after_colon,
    );
    // `50%` is a unit, not modulo — don't treat PERCENT after NUMBER as continuation.
    if last_sig_kind == SyntaxKind::PERCENT && prev_sig_kind == SyntaxKind::NUMBER {
        is_continuation = false;
    }
    let is_sole_combinator = !had_prior_content
        && matches!(
            last_sig_kind,
            SyntaxKind::STAR | SyntaxKind::TILDE | SyntaxKind::PLUS | SyntaxKind::GT
        );
    // Count only `\n` to avoid double-counting `\r\n` as two newlines.
    let newlines = ws_text.bytes().filter(|&b| b == b'\n').count();
    // Blank lines break continuation for most tokens, but COMMA continues
    // across blank lines (matches Dart Sass behavior for selector lists).
    let blank_line_breaks = newlines > 1 && last_sig_kind != SyntaxKind::COMMA;
    if is_continuation && !is_sole_combinator && !blank_line_breaks {
        result.push((SyntaxKind::WHITESPACE, ws_text));
        return;
    }

    if *saw_content {
        if new_indent > current_indent {
            // Indentation increase: potentially opening a block.
            // Defer decision for at-rules and variable declarations.
            if *after_at || *after_dollar {
                *pending_lbrace = Some(new_indent);
            } else {
                result.push((SyntaxKind::LBRACE, ""));
                indent_stack.push(new_indent);
                *after_at = false;
                *after_dollar = false;
                *after_colon = false;
            }
        } else {
            // Same or less indentation: end of statement
            result.push((SyntaxKind::SEMICOLON, ""));
            *after_at = false;
            *after_dollar = false;
            *after_colon = false;
            // Close blocks for dedent
            while indent_stack.len() > 1 && *indent_stack.last().unwrap() > new_indent {
                indent_stack.pop();
                result.push((SyntaxKind::RBRACE, ""));
            }
        }
    } else {
        // No content on this line (blank or comment-only) — still handle dedent
        while indent_stack.len() > 1 && *indent_stack.last().unwrap() > new_indent {
            indent_stack.pop();
            result.push((SyntaxKind::RBRACE, ""));
        }
    }

    result.push((SyntaxKind::WHITESPACE, ws_text));
    *saw_content = false;
}

fn contains_newline(text: &str) -> bool {
    text.contains(['\n', '\r', '\x0c'])
}

fn measure_indent_after_last_newline(text: &str) -> u32 {
    let after = match text.rfind(['\n', '\r', '\x0c']) {
        Some(pos) => &text[pos + 1..],
        None => text,
    };
    let mut indent = 0u32;
    for b in after.bytes() {
        match b {
            // Tabs count as 1 column, matching Dart Sass behavior.
            b' ' | b'\t' => indent += 1,
            _ => break,
        }
    }
    indent
}
