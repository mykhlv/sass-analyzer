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

        // Consume one full UTF-8 character so we never split a multi-byte sequence
        let ch = self.current_char();
        self.pos += ch.len_utf8();

        (SyntaxKind::ERROR, &self.input[start..self.pos])
    }

    // ── Helpers (used by tasks 1.3+) ────────────────────────────────

    #[inline]
    #[allow(dead_code)]
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    #[inline]
    #[allow(dead_code)]
    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.input
            .as_bytes()
            .get(self.pos.saturating_add(offset))
            .copied()
    }

    #[inline]
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
