/// All syntax elements — both tokens (leaf) and nodes (composite).
///
/// Single enum for the entire grammar. Tokens occupy the lower range,
/// nodes the upper range. `is_token()` / `is_node()` discriminate.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(non_camel_case_types)]
#[repr(u16)]
#[rustfmt::skip]
pub enum SyntaxKind {
    // ── Tokens: punctuation ──────────────────────────────────────────
    SEMICOLON = 0,  // ;
    COLON,          // :
    COMMA,          // ,
    DOT,            // .
    LPAREN,         // (
    RPAREN,         // )
    LBRACE,         // {
    RBRACE,         // }
    LBRACKET,       // [
    RBRACKET,       // ]
    PLUS,           // +
    MINUS,          // -
    STAR,           // *
    SLASH,          // /
    PERCENT,        // %
    EQ,             // =
    GT,             // >
    LT,             // <
    BANG,           // !
    AMP,            // &
    TILDE,          // ~
    PIPE,           // |
    AT,             // @
    DOLLAR,         // $
    HASH,           // #

    // ── Tokens: literals & identifiers ───────────────────────────────
    IDENT,
    NUMBER,
    QUOTED_STRING,

    // ── Tokens: trivia ───────────────────────────────────────────────
    WHITESPACE,
    SINGLE_LINE_COMMENT,
    MULTI_LINE_COMMENT,

    // ── Tokens: special ──────────────────────────────────────────────
    ERROR,
    EOF,

    // ── Sentinel: end of token range ─────────────────────────────────
    #[doc(hidden)]
    __LAST_TOKEN,

    // ── Nodes ────────────────────────────────────────────────────────
    SOURCE_FILE,
    RULE_SET,
    SELECTOR_LIST,
    DECLARATION,
    BLOCK,
    PROPERTY,
}

impl SyntaxKind {
    pub fn is_token(self) -> bool {
        (self as u16) < __LAST_TOKEN as u16
    }

    pub fn is_node(self) -> bool {
        (self as u16) > __LAST_TOKEN as u16
    }

    pub fn is_trivia(self) -> bool {
        matches!(self, WHITESPACE | SINGLE_LINE_COMMENT | MULTI_LINE_COMMENT)
    }
}

impl From<u16> for SyntaxKind {
    #[rustfmt::skip]
    fn from(raw: u16) -> Self {
        match raw {
            0  => SEMICOLON,
            1  => COLON,
            2  => COMMA,
            3  => DOT,
            4  => LPAREN,
            5  => RPAREN,
            6  => LBRACE,
            7  => RBRACE,
            8  => LBRACKET,
            9  => RBRACKET,
            10 => PLUS,
            11 => MINUS,
            12 => STAR,
            13 => SLASH,
            14 => PERCENT,
            15 => EQ,
            16 => GT,
            17 => LT,
            18 => BANG,
            19 => AMP,
            20 => TILDE,
            21 => PIPE,
            22 => AT,
            23 => DOLLAR,
            24 => HASH,
            25 => IDENT,
            26 => NUMBER,
            27 => QUOTED_STRING,
            28 => WHITESPACE,
            29 => SINGLE_LINE_COMMENT,
            30 => MULTI_LINE_COMMENT,
            31 => ERROR,
            32 => EOF,
            33 => __LAST_TOKEN,
            34 => SOURCE_FILE,
            35 => RULE_SET,
            36 => SELECTOR_LIST,
            37 => DECLARATION,
            38 => BLOCK,
            39 => PROPERTY,
            _ => panic!("invalid SyntaxKind: {raw}"),
        }
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> Self {
        kind as u16
    }
}

pub use SyntaxKind::*;
