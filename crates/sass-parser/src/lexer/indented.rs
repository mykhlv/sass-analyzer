//! Comment extent in the indented syntax: the one lexical fact that depends on layout.
//!
//! A comment that starts a statement — the first token on its line, outside brackets —
//! reaches over the more deeply indented lines below it, as in Dart Sass:
//!
//! - `//` continues over each following line that is indented deeper than the comment's
//!   own line, with no blank line in between. Every SCSS token on a continuation line is
//!   retagged `SINGLE_LINE_COMMENT`; a token that would run past the end of that line is
//!   cut there, so a stray quote in a comment cannot swallow the rest of the file. With
//!   [`IndentedLexer::crossing_blank_lines`], blank lines do not stop it, as in Dart Sass.
//! - `/*` ends at `*/`, or at the end of the last line before the first non-blank line
//!   indented no deeper than the comment's own line, whichever comes first. When nothing
//!   but spaces follows the `/*`, the next non-blank line belongs to the comment whatever
//!   its indentation. No closing `*/` is needed.
//!
//! A comment anywhere else follows the SCSS rules: `//` to the end of the line, `/*` to
//! `*/` or the end of the file.

use super::Lexer;
use crate::syntax_kind::SyntaxKind;

/// The SCSS lexer, plus layout-dependent comment extent for indented Sass.
pub struct IndentedLexer<'src> {
    lexer: Lexer<'src>,
    /// Depth of `(`, `[` and `#{`; a newline inside them is not a line break of the layout,
    /// so a comment there never starts a statement.
    depth: u32,
    /// A significant token has been seen since the last line break at depth 0.
    line_has_content: bool,
    /// Indentation of the current line, measured at the last line break at depth 0.
    column: u32,
    /// Column of the `//` comment whose continuation lines are being lexed.
    silent_comment: Option<u32>,
    /// The current line continues that comment, so its tokens are retagged.
    in_continuation_line: bool,
    /// A `//` comment reaches across blank lines onto the deeper lines after them.
    across_blank_lines: bool,
}

impl<'src> IndentedLexer<'src> {
    pub fn new(input: &'src str) -> Self {
        Self {
            lexer: Lexer::new(input),
            depth: 0,
            line_has_content: false,
            column: 0,
            silent_comment: None,
            in_continuation_line: false,
            across_blank_lines: false,
        }
    }

    /// Like [`IndentedLexer::new`], except that a `//` comment that starts a statement
    /// also takes the deeper lines after a blank line, as Dart Sass reads it: blank lines
    /// do not end a silent comment, only a line indented no deeper than its own.
    pub fn crossing_blank_lines(input: &'src str) -> Self {
        Self {
            across_blank_lines: true,
            ..Self::new(input)
        }
    }

    /// [`IndentedLexer::crossing_blank_lines`] from byte `pos`, a line break outside
    /// brackets: as at the start of a line, outside any string, interpolation or comment.
    pub(crate) fn resume(input: &'src str, pos: usize) -> Self {
        let mut lexer = Self::crossing_blank_lines(input);
        lexer.lexer.pos = pos;
        lexer
    }

    /// Whether lexing from here on goes as from a fresh line break outside brackets: no
    /// bracket, string or interpolation is open and no comment reaches onto the next line.
    pub(crate) fn is_clean(&self) -> bool {
        self.depth == 0
            && self.silent_comment.is_none()
            && !self.in_continuation_line
            && self.lexer.context_stack.is_empty()
    }

    pub fn next_token(&mut self) -> (SyntaxKind, &'src str) {
        self.next_token_where(true)
    }

    /// Like [`Self::next_token`], but a comment starts a statement only where
    /// `statement_position` agrees. The layout alone cannot see that a line continues the
    /// one above it (`.a,` / `// c` / `  .b`): there the comment is an ordinary one, and only
    /// a caller that tracks continuations knows. `true` defers to the layout.
    pub fn next_token_where(&mut self, statement_position: bool) -> (SyntaxKind, &'src str) {
        let (kind, text) = self.lexer.next_token();
        if self.in_continuation_line {
            return self.continuation_token(kind, text);
        }
        match kind {
            SyntaxKind::WHITESPACE => {
                if contains_newline(text) {
                    self.line_break(text);
                }
            }
            SyntaxKind::SINGLE_LINE_COMMENT
            | SyntaxKind::MULTI_LINE_COMMENT
            | SyntaxKind::ERROR => return self.comment(kind, text, statement_position),
            SyntaxKind::EOF => {}
            _ => {
                self.track_depth(kind);
                self.line_has_content = true;
            }
        }
        (kind, text)
    }

    /// A comment, or an `ERROR` that may be a loud comment never closed.
    fn comment(
        &mut self,
        kind: SyntaxKind,
        text: &'src str,
        statement_position: bool,
    ) -> (SyntaxKind, &'src str) {
        let starts_statement = statement_position && self.depth == 0 && !self.line_has_content;
        match kind {
            SyntaxKind::SINGLE_LINE_COMMENT if starts_statement => {
                self.silent_comment = Some(self.column);
                (kind, text)
            }
            SyntaxKind::MULTI_LINE_COMMENT | SyntaxKind::ERROR
                if starts_statement && text.starts_with("/*") =>
            {
                (SyntaxKind::MULTI_LINE_COMMENT, self.loud_comment(text))
            }
            SyntaxKind::ERROR => {
                self.line_has_content = true;
                (kind, text)
            }
            _ => (kind, text),
        }
    }

    /// A token on a line that continues a `//` comment, which ends with that line.
    fn continuation_token(&mut self, kind: SyntaxKind, text: &'src str) -> (SyntaxKind, &'src str) {
        match kind {
            SyntaxKind::EOF => (kind, text),
            SyntaxKind::WHITESPACE if contains_newline(text) => {
                self.line_break(text);
                (kind, text)
            }
            _ => match text.bytes().position(is_newline) {
                Some(len) => (SyntaxKind::SINGLE_LINE_COMMENT, self.truncate(text, len)),
                None => (SyntaxKind::SINGLE_LINE_COMMENT, text),
            },
        }
    }

    fn line_break(&mut self, whitespace: &str) {
        if self.in_continuation_line {
            // Whatever the comment's tokens opened — a string, an interpolation, `url(` —
            // ends with its line.
            self.lexer.reset_context();
        }
        self.in_continuation_line = self.silent_comment.is_some_and(|column| {
            continues_silent_comment(whitespace, column, self.across_blank_lines)
        });
        if !self.in_continuation_line {
            self.silent_comment = None;
            if self.depth == 0 {
                self.line_has_content = false;
                self.column = indent_after_last_newline(whitespace);
            }
        }
    }

    fn loud_comment(&mut self, text: &'src str) -> &'src str {
        let start = self.lexer.pos - text.len();
        let len = loud_comment_len(&self.lexer.input[start..], self.column);
        if len < text.len() {
            self.truncate(text, len)
        } else {
            text
        }
    }

    /// Shorten the token just lexed to `len` bytes and resume lexing after them, outside
    /// any string, interpolation or `url(`.
    fn truncate(&mut self, text: &'src str, len: usize) -> &'src str {
        self.lexer.pos -= text.len() - len;
        self.lexer.reset_context();
        &text[..len]
    }

    /// `{` and `}` count only inside a bracket, where they belong to an interpolation or a
    /// map; an unmatched closer is ignored.
    fn track_depth(&mut self, kind: SyntaxKind) {
        match kind {
            SyntaxKind::LPAREN | SyntaxKind::LBRACKET | SyntaxKind::HASH_LBRACE => self.depth += 1,
            SyntaxKind::LBRACE if self.depth > 0 => self.depth += 1,
            SyntaxKind::RPAREN | SyntaxKind::RBRACKET | SyntaxKind::RBRACE => {
                self.depth = self.depth.saturating_sub(1);
            }
            _ => {}
        }
    }
}

#[inline]
pub fn contains_newline(text: &str) -> bool {
    text.bytes().any(is_newline)
}

/// The indentation after the last newline in `whitespace`: its leading space and tab
/// bytes, each tab counting as one column, as in Dart Sass.
pub fn indent_after_last_newline(whitespace: &str) -> u32 {
    let bytes = whitespace.as_bytes();
    let line_start = bytes
        .iter()
        .rposition(|&b| is_newline(b))
        .map_or(0, |pos| pos + 1);
    let indent = spaces(&bytes[line_start..]);
    #[allow(clippy::cast_possible_truncation)]
    {
        indent as u32
    }
}

/// Whether a line break continues a `//` comment begun at `column`: the next line is
/// indented deeper, and it is a single line break unless `across_blank_lines`.
fn continues_silent_comment(whitespace: &str, column: u32, across_blank_lines: bool) -> bool {
    (across_blank_lines || line_break_count(whitespace) == 1)
        && indent_after_last_newline(whitespace) > column
}

/// `\r\n` is one line break, as is a lone `\n`, `\r` or `\x0c`.
fn line_break_count(text: &str) -> usize {
    let bytes = text.as_bytes();
    bytes
        .iter()
        .enumerate()
        .filter(|&(i, &b)| match b {
            b'\n' => i == 0 || bytes[i - 1] != b'\r',
            b'\r' | b'\x0c' => true,
            _ => false,
        })
        .count()
}

#[inline]
fn is_newline(b: u8) -> bool {
    matches!(b, b'\n' | b'\r' | b'\x0c')
}

/// Length of a loud comment that starts a statement on a line indented `column` deep;
/// `text` starts with its `/*` and runs to the end of the file.
fn loud_comment_len(text: &str, column: u32) -> usize {
    let bytes = text.as_bytes();
    let mut pos = 2;
    // With nothing after the `/*`, the next line is the comment's first, however indented.
    let rest_of_line = pos + spaces(&bytes[pos..]);
    if bytes.get(rest_of_line).copied().is_some_and(is_newline) {
        match next_line(bytes, rest_of_line) {
            Some((_, start)) => pos = start,
            None => return rest_of_line,
        }
    }
    loop {
        let line_end = match bytes[pos..].iter().position(|&b| is_newline(b)) {
            Some(offset) => pos + offset,
            None => bytes.len(),
        };
        if let Some(offset) = bytes[pos..line_end].windows(2).position(|w| w == b"*/") {
            return pos + offset + 2;
        }
        match next_line(bytes, line_end) {
            Some((indent, start)) if indent > column => pos = start,
            _ => return line_end,
        }
    }
}

/// The next non-blank line after the line break at `pos`: its indentation and where its
/// content starts. `None` at the end of the file.
fn next_line(bytes: &[u8], mut pos: usize) -> Option<(u32, usize)> {
    let mut indent = 0;
    while let Some(&b) = bytes.get(pos) {
        if is_newline(b) {
            indent = 0;
        } else if matches!(b, b' ' | b'\t') {
            indent += 1;
        } else {
            return Some((indent, pos));
        }
        pos += 1;
    }
    None
}

fn spaces(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count()
}
