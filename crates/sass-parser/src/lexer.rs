use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexContext {
    Interpolation { brace_depth: u32 },
    String { quote: u8 },
    Url,
}

pub struct Lexer<'src> {
    input: &'src str,
    pos: usize,
    context_stack: Vec<LexContext>,
    pending_url_context: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(input: &'src str) -> Self {
        Self {
            input,
            pos: 0,
            context_stack: Vec::new(),
            pending_url_context: false,
        }
    }

    pub fn next_token(&mut self) -> (SyntaxKind, &'src str) {
        if self.pos >= self.input.len() {
            return (SyntaxKind::EOF, "");
        }

        if let Some(&LexContext::String { quote }) = self.context_stack.last() {
            return self.lex_string_content(quote);
        }

        if matches!(self.context_stack.last(), Some(LexContext::Url)) {
            return self.lex_url_content();
        }

        if self.pending_url_context {
            self.pending_url_context = false;
            let start = self.pos;
            self.pos += 1; // consume (
            self.context_stack.push(LexContext::Url);
            return (SyntaxKind::LPAREN, &self.input[start..self.pos]);
        }

        // ── BOM at start of file ─────────────────────────────────
        if self.pos == 0 && self.input.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
            self.pos = 3;
            return (SyntaxKind::WHITESPACE, &self.input[..3]);
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
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                self.lex_ident();
                if self.is_unicode_range_start(start) {
                    self.lex_unicode_range()
                } else if self.is_url_start(start) {
                    self.pending_url_context = true;
                    SyntaxKind::IDENT
                } else {
                    SyntaxKind::IDENT
                }
            }
            b'-' if self.is_ident_start_after_hyphen() => self.lex_ident(),

            // ── Strings ──────────────────────────────────────────────
            b'"' | b'\'' => return self.lex_string(start, b),

            // ── Multi-char operators (must come before single-char) ──
            b'#' if self.peek() == Some(b'{') => {
                self.bump();
                self.context_stack
                    .push(LexContext::Interpolation { brace_depth: 1 });
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
            b'{' => {
                if let Some(LexContext::Interpolation { brace_depth }) =
                    self.context_stack.last_mut()
                {
                    *brace_depth += 1;
                }
                SyntaxKind::LBRACE
            }
            b'}' => {
                let pop = matches!(
                    self.context_stack.last(),
                    Some(LexContext::Interpolation { brace_depth: 1 })
                );
                if pop {
                    self.context_stack.pop();
                } else if let Some(LexContext::Interpolation { brace_depth }) =
                    self.context_stack.last_mut()
                {
                    *brace_depth -= 1;
                }
                SyntaxKind::RBRACE
            }
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

            // ── CSS escape: \X starts an identifier ─────────────────
            b'\\' if matches!(self.peek(), Some(b) if b != b'\n' && b != b'\r' && b != b'\x0C') => {
                self.skip_escape_char();
                self.lex_ident()
            }

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

    fn lex_string(&mut self, start: usize, quote: u8) -> (SyntaxKind, &'src str) {
        loop {
            match self.peek() {
                None => return (SyntaxKind::ERROR, &self.input[start..self.pos]),
                Some(b) if b == quote => {
                    self.pos += 1;
                    return (SyntaxKind::QUOTED_STRING, &self.input[start..self.pos]);
                }
                Some(b'#') if self.peek_at(1) == Some(b'{') => {
                    self.context_stack.push(LexContext::String { quote });
                    return (SyntaxKind::STRING_START, &self.input[start..self.pos]);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    self.skip_escape_char();
                }
                _ => {
                    self.pos += 1;
                }
            }
        }
    }

    fn lex_string_content(&mut self, quote: u8) -> (SyntaxKind, &'src str) {
        if self.peek() == Some(b'#') && self.peek_at(1) == Some(b'{') {
            let start = self.pos;
            self.pos += 2;
            self.context_stack
                .push(LexContext::Interpolation { brace_depth: 1 });
            return (SyntaxKind::HASH_LBRACE, &self.input[start..self.pos]);
        }

        let start = self.pos;
        loop {
            match self.peek() {
                None => {
                    self.context_stack.pop();
                    return (SyntaxKind::ERROR, &self.input[start..self.pos]);
                }
                Some(b) if b == quote => {
                    self.pos += 1;
                    self.context_stack.pop();
                    return (SyntaxKind::STRING_END, &self.input[start..self.pos]);
                }
                Some(b'#') if self.peek_at(1) == Some(b'{') => {
                    return (SyntaxKind::STRING_MID, &self.input[start..self.pos]);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    self.skip_escape_char();
                }
                _ => {
                    self.pos += 1;
                }
            }
        }
    }

    fn lex_url_content(&mut self) -> (SyntaxKind, &'src str) {
        // Interpolation start
        if self.peek() == Some(b'#') && self.peek_at(1) == Some(b'{') {
            let start = self.pos;
            self.pos += 2;
            self.context_stack
                .push(LexContext::Interpolation { brace_depth: 1 });
            return (SyntaxKind::HASH_LBRACE, &self.input[start..self.pos]);
        }

        // Whitespace inside url()
        if matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0C')) {
            let start = self.pos;
            self.eat_while(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\x0C'));
            return (SyntaxKind::WHITESPACE, &self.input[start..self.pos]);
        }

        // Closing paren
        if self.peek() == Some(b')') {
            let start = self.pos;
            self.pos += 1;
            self.context_stack.pop();
            return (SyntaxKind::RPAREN, &self.input[start..self.pos]);
        }

        // Scan URL content until ), whitespace, #{, or EOF
        let start = self.pos;
        loop {
            match self.peek() {
                None => {
                    self.context_stack.pop();
                    return (SyntaxKind::URL_CONTENTS, &self.input[start..self.pos]);
                }
                Some(b')' | b' ' | b'\t' | b'\n' | b'\r' | b'\x0C') => {
                    return (SyntaxKind::URL_CONTENTS, &self.input[start..self.pos]);
                }
                Some(b'#') if self.peek_at(1) == Some(b'{') => {
                    return (SyntaxKind::URL_CONTENTS, &self.input[start..self.pos]);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    self.skip_escape_char();
                }
                Some(b) if b >= 0x80 => {
                    self.pos += self.current_char().len_utf8();
                }
                Some(_) => {
                    self.pos += 1;
                }
            }
        }
    }

    fn is_url_start(&self, start: usize) -> bool {
        self.pos - start == 3
            && self.input[start..self.pos].eq_ignore_ascii_case("url")
            && self.peek() == Some(b'(')
            && self.should_enter_url_context()
    }

    fn should_enter_url_context(&self) -> bool {
        let bytes = self.input.as_bytes();
        let mut p = self.pos + 1; // skip past (
        while p < bytes.len() && matches!(bytes[p], b' ' | b'\t' | b'\n' | b'\r' | b'\x0C') {
            p += 1;
        }
        !matches!(bytes.get(p), Some(b'\'' | b'"'))
    }

    fn skip_escape_char(&mut self) {
        match self.peek() {
            None | Some(b'\n' | b'\r' | b'\x0C') => {}
            Some(b) if b.is_ascii_hexdigit() => {
                // CSS hex escape: 1-6 hex digits, optional trailing whitespace.
                self.pos += 1;
                for _ in 0..5 {
                    if matches!(self.peek(), Some(b) if b.is_ascii_hexdigit()) {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                // Optional single whitespace after hex escape (consumed, not part of value).
                if matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0C')) {
                    self.pos += 1;
                }
            }
            Some(b) if b >= 0x80 => self.pos += self.current_char().len_utf8(),
            Some(_) => self.pos += 1,
        }
    }

    fn lex_number(&mut self) -> SyntaxKind {
        self.eat_while(|b| b.is_ascii_digit());
        if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b'0'..=b'9')) {
            self.bump(); // .
            self.eat_while(|b| b.is_ascii_digit());
        }
        // Scientific notation: 1e3, 1E3, 1e+3, 1e-3
        if matches!(self.peek(), Some(b'e' | b'E')) {
            match self.peek_at(1) {
                Some(b'0'..=b'9') => {
                    self.bump(); // e/E
                    self.eat_while(|b| b.is_ascii_digit());
                }
                Some(b'+' | b'-') if matches!(self.peek_at(2), Some(b'0'..=b'9')) => {
                    self.bump(); // e/E
                    self.bump(); // +/-
                    self.eat_while(|b| b.is_ascii_digit());
                }
                _ => {}
            }
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
                Some(b'\\') => {
                    // CSS escape continuation in identifier
                    self.pos += 1;
                    self.skip_escape_char();
                }
                _ => break,
            }
        }
        SyntaxKind::IDENT
    }

    fn is_unicode_range_start(&self, start: usize) -> bool {
        self.pos - start == 1
            && matches!(self.input.as_bytes()[start], b'U' | b'u')
            && self.peek() == Some(b'+')
            && matches!(self.peek_at(1), Some(b) if b.is_ascii_hexdigit() || b == b'?')
    }

    fn lex_unicode_range(&mut self) -> SyntaxKind {
        self.pos += 1; // consume +
        self.eat_while(|b| b.is_ascii_hexdigit() || b == b'?');
        if self.peek() == Some(b'-') && matches!(self.peek_at(1), Some(b) if b.is_ascii_hexdigit())
        {
            self.pos += 1; // consume -
            self.eat_while(|b| b.is_ascii_hexdigit());
        }
        SyntaxKind::UNICODE_RANGE
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
