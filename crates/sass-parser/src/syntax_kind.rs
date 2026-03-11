/// All syntax elements — both tokens (leaf) and nodes (composite).
///
/// Single enum for the entire grammar. Tokens occupy the lower range,
/// nodes the upper range. `is_token()` / `is_node()` discriminate.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(non_camel_case_types, missing_docs)]
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
    /// Returns `true` if this kind represents a leaf token (not a composite node).
    pub fn is_token(self) -> bool {
        (self as u16) < __LAST_TOKEN as u16
    }

    /// Returns `true` if this kind represents a composite node (not a leaf token).
    pub fn is_node(self) -> bool {
        (self as u16) > __LAST_TOKEN as u16
    }

    /// Returns `true` if this kind is trivia (whitespace or comments).
    pub fn is_trivia(self) -> bool {
        matches!(self, WHITESPACE | SINGLE_LINE_COMMENT | MULTI_LINE_COMMENT)
    }
}

impl From<u16> for SyntaxKind {
    fn from(raw: u16) -> Self {
        if raw > PARAM as u16 {
            debug_assert!(false, "invalid SyntaxKind: {raw} (max: {})", PARAM as u16);
            return ERROR;
        }
        // SAFETY: SyntaxKind is #[repr(u16)] with contiguous discriminants 0..=PARAM.
        // The bounds check above guarantees `raw` is in range.
        unsafe { std::mem::transmute(raw) }
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> Self {
        kind as u16
    }
}

pub(crate) use SyntaxKind::*;
