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

    // ── Tokens: multi-char operators ───────────────────────────────
    HASH_LBRACE,    // #{
    DOT_DOT_DOT,   // ...
    EQ_EQ,          // ==
    BANG_EQ,        // !=
    LT_EQ,          // <=
    GT_EQ,          // >=
    COLON_COLON,    // ::

    // ── Tokens: attribute selector operators ───────────────────────
    TILDE_EQ,       // ~=
    PIPE_EQ,        // |=
    CARET_EQ,       // ^=
    DOLLAR_EQ,      // $=
    STAR_EQ,        // *=

    // ── Tokens: literals & identifiers ─────────────────────────────
    IDENT,
    NUMBER,
    QUOTED_STRING,
    STRING_START,   // opening quote + text before first #{
    STRING_MID,     // text between }...#{
    STRING_END,     // text after last } + closing quote
    URL_CONTENTS,   // unquoted url() content segment
    UNICODE_RANGE,  // U+0025-00FF

    // ── Tokens: trivia ─────────────────────────────────────────────
    WHITESPACE,
    SINGLE_LINE_COMMENT,
    MULTI_LINE_COMMENT,

    // ── Tokens: special ────────────────────────────────────────────
    ERROR,
    EOF,

    // ── Sentinel: end of token range ───────────────────────────────
    #[doc(hidden)]
    __LAST_TOKEN,

    // ── Nodes ──────────────────────────────────────────────────────
    SOURCE_FILE,
    RULE_SET,
    SELECTOR_LIST,
    SELECTOR,
    SIMPLE_SELECTOR,
    PSEUDO_SELECTOR,
    ATTR_SELECTOR,
    COMBINATOR,
    DECLARATION,
    VALUE,
    CUSTOM_PROPERTY_DECL,
    NESTED_PROPERTY,
    BLOCK,
    PROPERTY,
    INTERPOLATION,
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
            25 => HASH_LBRACE,
            26 => DOT_DOT_DOT,
            27 => EQ_EQ,
            28 => BANG_EQ,
            29 => LT_EQ,
            30 => GT_EQ,
            31 => COLON_COLON,
            32 => TILDE_EQ,
            33 => PIPE_EQ,
            34 => CARET_EQ,
            35 => DOLLAR_EQ,
            36 => STAR_EQ,
            37 => IDENT,
            38 => NUMBER,
            39 => QUOTED_STRING,
            40 => STRING_START,
            41 => STRING_MID,
            42 => STRING_END,
            43 => URL_CONTENTS,
            44 => UNICODE_RANGE,
            45 => WHITESPACE,
            46 => SINGLE_LINE_COMMENT,
            47 => MULTI_LINE_COMMENT,
            48 => ERROR,
            49 => EOF,
            50 => __LAST_TOKEN,
            51 => SOURCE_FILE,
            52 => RULE_SET,
            53 => SELECTOR_LIST,
            54 => SELECTOR,
            55 => SIMPLE_SELECTOR,
            56 => PSEUDO_SELECTOR,
            57 => ATTR_SELECTOR,
            58 => COMBINATOR,
            59 => DECLARATION,
            60 => VALUE,
            61 => CUSTOM_PROPERTY_DECL,
            62 => NESTED_PROPERTY,
            63 => BLOCK,
            64 => PROPERTY,
            65 => INTERPOLATION,
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
