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
/// - Comma at end of line → continuation (no virtual tokens)
pub fn sass_tokenize(source: &str) -> Vec<(SyntaxKind, &str)> {
    let raw = lexer::tokenize(source);
    let mut result = Vec::with_capacity(raw.len() + raw.len() / 4);
    let mut indent_stack: Vec<u32> = vec![0];
    let mut saw_content = false;
    let mut last_significant = SyntaxKind::EOF;
    // Combined nesting depth for (), [], and #{}
    let mut nesting: u32 = 0;

    for &(kind, text) in &raw {
        match kind {
            SyntaxKind::LPAREN | SyntaxKind::LBRACKET | SyntaxKind::HASH_LBRACE => {
                nesting += 1;
                result.push((kind, text));
                saw_content = true;
                last_significant = kind;
            }
            SyntaxKind::RPAREN | SyntaxKind::RBRACKET => {
                nesting = nesting.saturating_sub(1);
                result.push((kind, text));
                saw_content = true;
                last_significant = kind;
            }
            SyntaxKind::RBRACE if nesting > 0 => {
                nesting -= 1;
                result.push((kind, text));
                saw_content = true;
                last_significant = kind;
            }
            SyntaxKind::LBRACE if nesting > 0 => {
                nesting += 1;
                result.push((kind, text));
                saw_content = true;
                last_significant = kind;
            }
            SyntaxKind::WHITESPACE if nesting == 0 && contains_newline(text) => {
                process_newline(
                    text,
                    &mut result,
                    &mut indent_stack,
                    &mut saw_content,
                    last_significant,
                );
            }
            _ if kind.is_trivia() => {
                result.push((kind, text));
            }
            _ => {
                result.push((kind, text));
                saw_content = true;
                last_significant = kind;
            }
        }
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

fn process_newline<'src>(
    ws_text: &'src str,
    result: &mut Vec<(SyntaxKind, &'src str)>,
    indent_stack: &mut Vec<u32>,
    saw_content: &mut bool,
    last_significant: SyntaxKind,
) {
    let new_indent = measure_indent_after_last_newline(ws_text);
    let current_indent = *indent_stack.last().unwrap_or(&0);

    // Comma at end of line → continuation (selector lists, argument lists).
    // Don't emit any virtual tokens; the statement continues on the next line.
    if last_significant == SyntaxKind::COMMA {
        result.push((SyntaxKind::WHITESPACE, ws_text));
        return;
    }

    if *saw_content {
        if new_indent > current_indent {
            // Indentation increase: opening a block
            result.push((SyntaxKind::LBRACE, ""));
            indent_stack.push(new_indent);
        } else {
            // Same or less indentation: end of statement
            result.push((SyntaxKind::SEMICOLON, ""));
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
    text.as_bytes().contains(&b'\n')
}

fn measure_indent_after_last_newline(text: &str) -> u32 {
    let after = match text.rfind('\n') {
        Some(pos) => &text[pos + 1..],
        None => text,
    };
    let mut indent = 0u32;
    for b in after.bytes() {
        match b {
            b' ' | b'\t' => indent += 1,
            _ => break,
        }
    }
    indent
}
