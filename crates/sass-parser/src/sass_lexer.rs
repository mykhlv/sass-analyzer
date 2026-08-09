use crate::lexer;
use crate::syntax_kind::SyntaxKind;

/// What the newline logic knows about the last significant (non-trivia) token.
///
/// Every field here is overwritten on each significant token. That shared lifetime is
/// what separates them from `StmtCtx`, whose facts outlive individual tokens and are
/// cleared only when the statement ends — a distinction worth keeping visible, since
/// answering a statement-scoped question with a line-scoped fact is how `> *` came to be
/// read as multiplication in the first place.
struct LastSig<'src> {
    kind: SyntaxKind,
    text: &'src str,
    /// Kind of the significant token before this one.
    prev_kind: SyntaxKind,
    /// This token is a directive keyword name (the first IDENT after an AT). Drives
    /// end-of-line continuation: `@mixin\n name` continues, `@extend .error\n body` does not.
    is_at_name: bool,
    /// Its line already had content before it, so a trailing `*` is `2 *` rather than a
    /// sole `*` selector.
    had_prior_content: bool,
}

impl<'src> LastSig<'src> {
    fn new() -> Self {
        Self {
            kind: SyntaxKind::EOF,
            text: "",
            prev_kind: SyntaxKind::EOF,
            is_at_name: false,
            had_prior_content: false,
        }
    }

    /// Record a significant token. `is_at_name` clears — the three sites that need it set
    /// assign it straight after.
    fn set(&mut self, kind: SyntaxKind, text: &'src str) {
        self.kind = kind;
        self.text = text;
        self.is_at_name = false;
    }
}

/// Facts about the statement currently being lexed.
///
/// These are independent rather than exclusive — `$a: 1` sets `after_dollar` and
/// `saw_colon` at once, `@page :first` sets `after_at` and `saw_colon` — so this is a set
/// of flags, not a state machine. What they share is a lifetime: all of them are cleared
/// together, by `end`, at every point where a statement finishes or a block opens.
#[derive(Default)]
struct StmtCtx<'src> {
    /// AT has been seen on this statement.
    after_at: bool,
    /// DOLLAR has been seen at statement start (for `$var\n  : value` continuation).
    after_dollar: bool,
    /// A top-level COLON has been seen. An approximation of "we are in a value", used to
    /// decide whether a trailing `*` is arithmetic or a universal selector. Deliberately
    /// over-approximates: a pseudo-class colon sets it too, which is the `.a:hover > *`
    /// limitation pinned in the tests.
    saw_colon: bool,
    /// The directive keyword (first IDENT after AT). Meaningless unless `after_at`, and
    /// every read is guarded by it.
    directive: &'src str,
}

impl StmtCtx<'_> {
    fn end(&mut self) {
        *self = Self::default();
    }

    /// Whether a bare `*` here is a binary operator rather than a universal selector.
    fn in_expr_position(&self) -> bool {
        self.saw_colon || (self.after_at && is_directive(self.directive, EXPR_DIRECTIVES))
    }
}

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
    let mut last = LastSig::new();
    let mut ctx = StmtCtx::default();
    // Combined nesting depth for (), [], and #{}
    let mut nesting: u32 = 0;
    // Pending virtual LBRACE: Some(indent) when we've deferred a block-open decision.
    let mut pending_lbrace: Option<u32> = None;
    // Indentation of the current line (updated on each newline-containing WHITESPACE).
    let mut current_line_indent: u32 = 0;

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
            if continues_header(kind, text, &ctx, last.is_at_name) {
                // Cancel the pending LBRACE — this line continues the header.
                pending_lbrace = None;
            } else {
                // Emit the deferred LBRACE.
                result.push((SyntaxKind::LBRACE, ""));
                indent_stack.push(pending_indent);
                pending_lbrace = None;
                ctx.end();
            }
        }

        // Save previous significant kind before the match overwrites it.
        if !kind.is_trivia() {
            last.prev_kind = last.kind;
        }

        match kind {
            SyntaxKind::LPAREN | SyntaxKind::LBRACKET | SyntaxKind::HASH_LBRACE => {
                nesting += 1;
                result.push((kind, text));
                saw_content = true;
                last.set(kind, text);
            }
            SyntaxKind::RPAREN | SyntaxKind::RBRACKET => {
                nesting = nesting.saturating_sub(1);
                result.push((kind, text));
                saw_content = true;
                last.set(kind, text);
            }
            SyntaxKind::RBRACE if nesting > 0 => {
                nesting -= 1;
                result.push((kind, text));
                saw_content = true;
                last.set(kind, text);
            }
            SyntaxKind::LBRACE if nesting > 0 => {
                nesting += 1;
                result.push((kind, text));
                saw_content = true;
                last.set(kind, text);
            }
            SyntaxKind::WHITESPACE if nesting == 0 && contains_newline(text) => {
                current_line_indent = measure_indent_after_last_newline(text);
                process_newline(
                    text,
                    &mut result,
                    &mut indent_stack,
                    &mut saw_content,
                    &mut pending_lbrace,
                    &last,
                    &mut ctx,
                );
            }
            // Sass shorthand: `=` at statement start → @mixin
            SyntaxKind::EQ if nesting == 0 && !saw_content => {
                result.push((SyntaxKind::AT, ""));
                result.push((SyntaxKind::IDENT, text));
                saw_content = true;
                last.set(SyntaxKind::IDENT, text);
                last.is_at_name = true;
                ctx.after_at = true;
                ctx.directive = "mixin";
            }
            // Sass shorthand: `+name` at statement start → @include
            SyntaxKind::PLUS if nesting == 0 && !saw_content => {
                if i + 1 < raw.len() && raw[i + 1].0 == SyntaxKind::IDENT {
                    result.push((SyntaxKind::AT, ""));
                    result.push((SyntaxKind::IDENT, text));
                    saw_content = true;
                    last.set(SyntaxKind::IDENT, text);
                    last.is_at_name = true;
                    ctx.after_at = true;
                    ctx.directive = "include";
                } else {
                    result.push((kind, text));
                    saw_content = true;
                    last.set(kind, text);
                }
            }
            SyntaxKind::AT if nesting == 0 => {
                result.push((kind, text));
                saw_content = true;
                last.set(kind, text);
                ctx.after_at = true;
                ctx.directive = "";
            }
            SyntaxKind::DOLLAR if nesting == 0 && !saw_content => {
                result.push((kind, text));
                saw_content = true;
                last.set(kind, text);
                ctx.after_dollar = true;
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
                    ctx.saw_colon = true;
                }
                // Track when this IDENT is the directive keyword name (first IDENT after AT).
                let is_at_name = kind == SyntaxKind::IDENT && last.kind == SyntaxKind::AT;
                if is_at_name {
                    ctx.directive = text;
                }
                last.set(kind, text);
                last.is_at_name = is_at_name;
            }
        }
        // Track whether the last significant token had prior content on its line.
        if !kind.is_trivia() {
            last.had_prior_content = prior_content;
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
    ctx: &StmtCtx<'_>,
    at_name_is_last: bool,
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
        return ctx.in_expr_position();
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
    ctx: &StmtCtx<'_>,
    at_name_is_last: bool,
) -> bool {
    if !is_start_of_line_continuation(kind, text) {
        return false;
    }
    if ctx.after_dollar {
        return true;
    }
    if !ctx.after_at {
        return false;
    }
    let directive = ctx.directive;
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

fn process_newline<'src>(
    ws_text: &'src str,
    result: &mut Vec<(SyntaxKind, &'src str)>,
    indent_stack: &mut Vec<u32>,
    saw_content: &mut bool,
    pending_lbrace: &mut Option<u32>,
    last: &LastSig<'_>,
    ctx: &mut StmtCtx<'_>,
) {
    let new_indent = measure_indent_after_last_newline(ws_text);
    let current_indent = *indent_stack.last().unwrap_or(&0);

    // Continuation token at end of line → statement continues on the next line.
    // Exception: combinator tokens (*, ~, +, >) as the sole content on a line
    // are selectors, not operators — don't treat them as continuation.
    let mut is_continuation =
        is_end_of_line_continuation(last.kind, last.text, ctx, last.is_at_name);
    // `50%` is a unit, not modulo — don't treat PERCENT after NUMBER as continuation.
    if last.kind == SyntaxKind::PERCENT && last.prev_kind == SyntaxKind::NUMBER {
        is_continuation = false;
    }
    let is_sole_combinator = !last.had_prior_content
        && matches!(
            last.kind,
            SyntaxKind::STAR | SyntaxKind::TILDE | SyntaxKind::PLUS | SyntaxKind::GT
        );
    // Count only `\n` to avoid double-counting `\r\n` as two newlines.
    let newlines = ws_text.bytes().filter(|&b| b == b'\n').count();
    // Blank lines break continuation for most tokens, but COMMA continues
    // across blank lines (matches Dart Sass behavior for selector lists).
    let blank_line_breaks = newlines > 1 && last.kind != SyntaxKind::COMMA;
    if is_continuation && !is_sole_combinator && !blank_line_breaks {
        result.push((SyntaxKind::WHITESPACE, ws_text));
        return;
    }

    if *saw_content {
        if new_indent > current_indent {
            // Indentation increase: potentially opening a block.
            // Defer decision for at-rules and variable declarations.
            if ctx.after_at || ctx.after_dollar {
                *pending_lbrace = Some(new_indent);
            } else {
                result.push((SyntaxKind::LBRACE, ""));
                indent_stack.push(new_indent);
                ctx.end();
            }
        } else {
            // Same or less indentation: end of statement
            result.push((SyntaxKind::SEMICOLON, ""));
            ctx.end();
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
