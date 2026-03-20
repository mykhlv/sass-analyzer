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
    // Combined nesting depth for (), [], and #{}
    let mut nesting: u32 = 0;
    // Pending virtual LBRACE: Some(indent) when we've deferred a block-open decision.
    let mut pending_lbrace: Option<u32> = None;
    // True when AT has been seen on the current statement (reset on virtual SEMICOLON/LBRACE).
    let mut after_at = false;
    // True when DOLLAR has been seen at statement start (for `$var\n  : value` continuation).
    let mut after_dollar = false;
    // True when the last significant token is the directive keyword name (first IDENT after AT).
    // Used for end-of-line continuation: `@mixin\n name` but NOT `@extend .error\n body`.
    let mut at_name_is_last = false;

    let mut i = 0;
    while i < raw.len() {
        let (kind, text) = raw[i];

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
            if (after_at || after_dollar) && is_start_of_line_continuation(kind, text) {
                // Cancel the pending LBRACE — this line continues the header.
                pending_lbrace = None;
            } else {
                // Emit the deferred LBRACE.
                result.push((SyntaxKind::LBRACE, ""));
                indent_stack.push(pending_indent);
                pending_lbrace = None;
                after_at = false;
                after_dollar = false;
            }
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
                process_newline(
                    text,
                    &mut result,
                    &mut indent_stack,
                    &mut saw_content,
                    last_sig_kind,
                    last_sig_text,
                    &mut pending_lbrace,
                    &mut after_at,
                    &mut after_dollar,
                    at_name_is_last,
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
            }
            SyntaxKind::DOLLAR if nesting == 0 && !saw_content => {
                result.push((kind, text));
                saw_content = true;
                last_sig_kind = kind;
                last_sig_text = text;
                after_dollar = true;
            }
            _ if kind.is_trivia() => {
                result.push((kind, text));
            }
            _ => {
                result.push((kind, text));
                saw_content = true;
                // Track when this IDENT is the directive keyword name (first IDENT after AT).
                at_name_is_last = kind == SyntaxKind::IDENT && last_sig_kind == SyntaxKind::AT;
                last_sig_kind = kind;
                last_sig_text = text;
            }
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
fn is_end_of_line_continuation(kind: SyntaxKind, text: &str, at_name_is_last: bool) -> bool {
    #[rustfmt::skip]
    let by_kind = matches!(
        kind,
        SyntaxKind::COMMA   | SyntaxKind::COLON   | SyntaxKind::BANG
        | SyntaxKind::PLUS  | SyntaxKind::MINUS   | SyntaxKind::STAR
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

#[allow(clippy::too_many_arguments)]
fn process_newline<'src>(
    ws_text: &'src str,
    result: &mut Vec<(SyntaxKind, &'src str)>,
    indent_stack: &mut Vec<u32>,
    saw_content: &mut bool,
    last_sig_kind: SyntaxKind,
    last_sig_text: &str,
    pending_lbrace: &mut Option<u32>,
    after_at: &mut bool,
    after_dollar: &mut bool,
    at_name_is_last: bool,
) {
    let new_indent = measure_indent_after_last_newline(ws_text);
    let current_indent = *indent_stack.last().unwrap_or(&0);

    // Continuation token at end of line → statement continues on the next line.
    // Don't emit any virtual tokens; don't change indent stack.
    if is_end_of_line_continuation(last_sig_kind, last_sig_text, at_name_is_last) {
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
            }
        } else {
            // Same or less indentation: end of statement
            result.push((SyntaxKind::SEMICOLON, ""));
            *after_at = false;
            *after_dollar = false;
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
