use crate::syntax_kind::SyntaxKind;

pub struct Lexer<'src> {
    input: &'src str,
    pos: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(input: &'src str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn next_token(&mut self) -> (SyntaxKind, &'src str) {
        if self.pos >= self.input.len() {
            return (SyntaxKind::EOF, "");
        }

        let start = self.pos;
        let b = self.bump();

        let kind = match b {
            // ── Whitespace ─────────────────────────────────────────
            b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => {
                self.eat_while(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\x0C'));
                SyntaxKind::WHITESPACE
            }

            // ── Comments & slash ───────────────────────────────────
            b'/' => match self.peek() {
                Some(b'/') => {
                    self.eat_while(|b| b != b'\n');
                    SyntaxKind::SINGLE_LINE_COMMENT
                }
                Some(b'*') => {
                    self.bump(); // consume *
                    self.lex_block_comment()
                }
                _ => SyntaxKind::SLASH,
            },

            // ── Numbers ────────────────────────────────────────────
            b'0'..=b'9' => self.lex_number(),
            b'.' if matches!(self.peek(), Some(b'0'..=b'9')) => self.lex_number(),

            // ── Identifiers ────────────────────────────────────────
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_ident(),
            b'-' if self.is_ident_start_after_hyphen() => self.lex_ident(),

            // ── Multi-char operators (must come before single-char) ──
            b'#' if self.peek() == Some(b'{') => {
                self.bump();
                SyntaxKind::HASH_LBRACE
            }
            b'.' if self.peek() == Some(b'.') && self.peek_at(1) == Some(b'.') => {
                self.bump();
                self.bump();
                SyntaxKind::DOT_DOT_DOT
            }
            b'=' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::EQ_EQ
            }
            b'!' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::BANG_EQ
            }
            b'<' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::LT_EQ
            }
            b'>' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::GT_EQ
            }
            b':' if self.peek() == Some(b':') => {
                self.bump();
                SyntaxKind::COLON_COLON
            }
            b'~' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::TILDE_EQ
            }
            b'|' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::PIPE_EQ
            }
            b'^' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::CARET_EQ
            }
            b'$' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::DOLLAR_EQ
            }
            b'*' if self.peek() == Some(b'=') => {
                self.bump();
                SyntaxKind::STAR_EQ
            }

            // ── Single-char punctuation ────────────────────────────
            b';' => SyntaxKind::SEMICOLON,
            b':' => SyntaxKind::COLON,
            b',' => SyntaxKind::COMMA,
            b'.' => SyntaxKind::DOT,
            b'(' => SyntaxKind::LPAREN,
            b')' => SyntaxKind::RPAREN,
            b'{' => SyntaxKind::LBRACE,
            b'}' => SyntaxKind::RBRACE,
            b'[' => SyntaxKind::LBRACKET,
            b']' => SyntaxKind::RBRACKET,
            b'+' => SyntaxKind::PLUS,
            b'-' => SyntaxKind::MINUS,
            b'*' => SyntaxKind::STAR,
            b'%' => SyntaxKind::PERCENT,
            b'=' => SyntaxKind::EQ,
            b'>' => SyntaxKind::GT,
            b'<' => SyntaxKind::LT,
            b'!' => SyntaxKind::BANG,
            b'&' => SyntaxKind::AMP,
            b'~' => SyntaxKind::TILDE,
            b'|' => SyntaxKind::PIPE,
            b'@' => SyntaxKind::AT,
            b'$' => SyntaxKind::DOLLAR,
            b'#' => SyntaxKind::HASH,

            // ── Non-ASCII: identifier or unknown ───────────────────
            _ if b >= 0x80 => {
                // Rewind: bump() advanced 1 byte, but we need a full char
                self.pos = start;
                let ch = self.current_char();
                self.pos += ch.len_utf8();
                if is_ident_start_char(ch) {
                    self.lex_ident()
                } else {
                    SyntaxKind::ERROR
                }
            }

            // ── Unknown ────────────────────────────────────────────
            _ => SyntaxKind::ERROR,
        };

        (kind, &self.input[start..self.pos])
    }

    // ── Token-specific helpers ────────────────────────────────────────

    fn lex_block_comment(&mut self) -> SyntaxKind {
        loop {
            match self.peek() {
                None => return SyntaxKind::ERROR, // unterminated
                Some(b'*') if self.peek_at(1) == Some(b'/') => {
                    self.bump(); // *
                    self.bump(); // /
                    return SyntaxKind::MULTI_LINE_COMMENT;
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    fn lex_number(&mut self) -> SyntaxKind {
        self.eat_while(|b| b.is_ascii_digit());
        if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b'0'..=b'9')) {
            self.bump(); // .
            self.eat_while(|b| b.is_ascii_digit());
        }
        SyntaxKind::NUMBER
    }

    fn lex_ident(&mut self) -> SyntaxKind {
        loop {
            match self.peek() {
                Some(b) if is_ascii_ident_continue(b) => {
                    self.pos += 1;
                }
                Some(b) if b >= 0x80 => {
                    let ch = self.current_char();
                    if is_ident_continue_char(ch) {
                        self.pos += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        SyntaxKind::IDENT
    }

    fn is_ident_start_after_hyphen(&self) -> bool {
        match self.peek() {
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'-') => true,
            Some(b) if b >= 0x80 => is_ident_start_char(self.current_char()),
            _ => false,
        }
    }

    // ── Low-level helpers ─────────────────────────────────────────────

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    #[inline]
    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.input
            .as_bytes()
            .get(self.pos.checked_add(offset)?)
            .copied()
    }

    #[inline]
    fn bump(&mut self) -> u8 {
        debug_assert!(
            self.pos < self.input.len(),
            "Lexer::bump called at end of input"
        );
        let b = self.input.as_bytes()[self.pos];
        self.pos += 1;
        b
    }

    fn current_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    // Predicate must only match ASCII bytes (< 0x80) to preserve UTF-8 alignment.
    fn eat_while(&mut self, pred: impl Fn(u8) -> bool) {
        while let Some(b) = self.peek() {
            if pred(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }
}

// ── Free-standing character classification ────────────────────────────

#[inline]
fn is_ascii_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

#[inline]
fn is_ident_start_char(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

#[inline]
fn is_ident_continue_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '-'
}

/// Collect all tokens from `input`, excluding the final `EOF`.
pub fn tokenize(input: &str) -> Vec<(SyntaxKind, &str)> {
    let mut lexer = Lexer::new(input);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token();
        if tok.0 == SyntaxKind::EOF {
            break;
        }
        tokens.push(tok);
    }
    tokens
}
