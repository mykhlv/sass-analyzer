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

    // ── Nodes: expressions (Phase 3) ─────────────────────────────
    VARIABLE_DECL,          // $var: expr;
    VARIABLE_REF,           // $var
    BINARY_EXPR,            // a + b, a * b, etc.
    UNARY_EXPR,             // -a, +a, not a
    PAREN_EXPR,             // (expr)
    NUMBER_LITERAL,         // 42, 3.14
    DIMENSION,              // 10px, 2em, 100%
    STRING_LITERAL,         // "hello" or 'hello' (non-interpolated)
    INTERPOLATED_STRING,    // "hello #{$name}!"
    COLOR_LITERAL,          // #fff, #aabbcc
    BOOL_LITERAL,           // true, false
    NULL_LITERAL,           // null
    LIST_EXPR,              // comma-separated list
    BRACKETED_LIST,         // [a, b, c]
    MAP_EXPR,               // (key: value, ...)
    MAP_ENTRY,              // key: value (inside map)
    FUNCTION_CALL,          // name(args)
    ARG_LIST,               // (a, $b: c, $rest...)
    ARG,                    // single argument (positional or keyword)
    CALCULATION,            // calc(), min(), max(), clamp(), etc.
    CALC_SUM,               // a + b or a - b inside calculation
    CALC_PRODUCT,           // a * b or a / b inside calculation
    CALC_VALUE,             // single value inside calculation
    SPECIAL_FUNCTION_CALL,  // url(), element(), progid:...()
    STANDALONE_PERCENT,     // standalone % atom
    IMPORTANT,              // !important
    SASS_FLAG,              // !default, !global, !optional

    // ── Nodes: at-rules (Phase 4) ────────────────────────────────
    MIXIN_RULE,             // @mixin name(params) { }
    INCLUDE_RULE,           // @include name(args) { content }
    CONTENT_RULE,           // @content or @content(args)
    FUNCTION_RULE,          // @function name(params) { }
    RETURN_RULE,            // @return expr;
    IF_RULE,                // @if cond { } @else if { } @else { }
    ELSE_CLAUSE,            // @else if ... { } or @else { }
    FOR_RULE,               // @for $var from expr through/to expr { }
    EACH_RULE,              // @each $var in expr { }
    WHILE_RULE,             // @while cond { }
    EXTEND_RULE,            // @extend selector !optional;
    ERROR_RULE,             // @error expr;
    WARN_RULE,              // @warn expr;
    DEBUG_RULE,             // @debug expr;
    AT_ROOT_RULE,           // @at-root { } / @at-root (query) { }
    AT_ROOT_QUERY,          // (with: media) or (without: ...)
    MEDIA_RULE,             // @media query { }
    MEDIA_QUERY,            // individual media query
    SUPPORTS_RULE,          // @supports condition { }
    SUPPORTS_CONDITION,     // not/and/or (prop: value)
    KEYFRAMES_RULE,         // @keyframes name { }
    KEYFRAME_SELECTOR,      // from, to, 50%, etc.
    LAYER_RULE,             // @layer name { } or @layer name;
    CONTAINER_RULE,         // @container name (query) { }
    SCOPE_RULE,             // @scope (.card) to (.content) { }
    PROPERTY_RULE,          // @property --name { }
    NAMESPACE_RULE,         // @namespace prefix url();
    CHARSET_RULE,           // @charset "UTF-8";
    PAGE_RULE,              // @page :first { }
    FONT_FACE_RULE,         // @font-face { }
    USE_RULE,               // @use "path" as name with (...)
    FORWARD_RULE,           // @forward "path" ...
    IMPORT_RULE,            // @import "path"
    NAMESPACE_REF,          // ns.$var, ns.func()
    GENERIC_AT_RULE,        // unknown @foo
    PARAM_LIST,             // ($name, $name: default, $rest...)
    PARAM,                  // single parameter
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
            // Phase 3: expressions
            66 => VARIABLE_DECL,
            67 => VARIABLE_REF,
            68 => BINARY_EXPR,
            69 => UNARY_EXPR,
            70 => PAREN_EXPR,
            71 => NUMBER_LITERAL,
            72 => DIMENSION,
            73 => STRING_LITERAL,
            74 => INTERPOLATED_STRING,
            75 => COLOR_LITERAL,
            76 => BOOL_LITERAL,
            77 => NULL_LITERAL,
            78 => LIST_EXPR,
            79 => BRACKETED_LIST,
            80 => MAP_EXPR,
            81 => MAP_ENTRY,
            82 => FUNCTION_CALL,
            83 => ARG_LIST,
            84 => ARG,
            85 => CALCULATION,
            86 => CALC_SUM,
            87 => CALC_PRODUCT,
            88 => CALC_VALUE,
            89 => SPECIAL_FUNCTION_CALL,
            90 => STANDALONE_PERCENT,
            91 => IMPORTANT,
            92 => SASS_FLAG,
            // Phase 4: at-rules
            93  => MIXIN_RULE,
            94  => INCLUDE_RULE,
            95  => CONTENT_RULE,
            96  => FUNCTION_RULE,
            97  => RETURN_RULE,
            98  => IF_RULE,
            99  => ELSE_CLAUSE,
            100 => FOR_RULE,
            101 => EACH_RULE,
            102 => WHILE_RULE,
            103 => EXTEND_RULE,
            104 => ERROR_RULE,
            105 => WARN_RULE,
            106 => DEBUG_RULE,
            107 => AT_ROOT_RULE,
            108 => AT_ROOT_QUERY,
            109 => MEDIA_RULE,
            110 => MEDIA_QUERY,
            111 => SUPPORTS_RULE,
            112 => SUPPORTS_CONDITION,
            113 => KEYFRAMES_RULE,
            114 => KEYFRAME_SELECTOR,
            115 => LAYER_RULE,
            116 => CONTAINER_RULE,
            117 => SCOPE_RULE,
            118 => PROPERTY_RULE,
            119 => NAMESPACE_RULE,
            120 => CHARSET_RULE,
            121 => PAGE_RULE,
            122 => FONT_FACE_RULE,
            123 => USE_RULE,
            124 => FORWARD_RULE,
            125 => IMPORT_RULE,
            126 => NAMESPACE_REF,
            127 => GENERIC_AT_RULE,
            128 => PARAM_LIST,
            129 => PARAM,
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
