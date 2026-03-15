use sass_parser::line_index::LineIndex;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextSize;
use tower_lsp_server::ls_types::SemanticToken;

use crate::ast_helpers::{dollar_ident_range, first_ident_token, nth_ident_token};
use crate::convert::{byte_to_lsp_pos, utf16_len};

// Semantic token type indices (must match legend order in initialize)
pub(crate) const TOK_VARIABLE: u32 = 0;
pub(crate) const TOK_FUNCTION: u32 = 1;
pub(crate) const TOK_MIXIN: u32 = 2;
pub(crate) const TOK_PARAMETER: u32 = 3;
pub(crate) const TOK_PROPERTY: u32 = 4;
pub(crate) const TOK_TYPE: u32 = 5;

pub(crate) const MOD_DECLARATION: u32 = 1 << 0;

pub(crate) struct RawSemanticToken {
    pub(crate) start: u32,
    pub(crate) len: u32,
    pub(crate) token_type: u32,
    pub(crate) modifiers: u32,
}

pub(crate) fn collect_semantic_tokens(root: &SyntaxNode) -> Vec<RawSemanticToken> {
    let mut tokens = Vec::new();

    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::VARIABLE_DECL => {
                if let Some((range, len)) = dollar_ident_range(&node) {
                    tokens.push(RawSemanticToken {
                        start: range.start().into(),
                        len,
                        token_type: TOK_VARIABLE,
                        modifiers: MOD_DECLARATION,
                    });
                }
            }
            SyntaxKind::VARIABLE_REF => {
                if let Some((range, len)) = dollar_ident_range(&node) {
                    tokens.push(RawSemanticToken {
                        start: range.start().into(),
                        len,
                        token_type: TOK_VARIABLE,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::FUNCTION_CALL => {
                if let Some(ident) = first_ident_token(&node) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_FUNCTION,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::FUNCTION_RULE => {
                // Skip first IDENT ("function"), take second (the name)
                if let Some(ident) = nth_ident_token(&node, 1) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_FUNCTION,
                        modifiers: MOD_DECLARATION,
                    });
                }
            }
            SyntaxKind::MIXIN_RULE => {
                if let Some(ident) = nth_ident_token(&node, 1) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_MIXIN,
                        modifiers: MOD_DECLARATION,
                    });
                }
            }
            SyntaxKind::INCLUDE_RULE => {
                if let Some(ident) = nth_ident_token(&node, 1) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_MIXIN,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::PARAM => {
                if let Some((range, len)) = dollar_ident_range(&node) {
                    tokens.push(RawSemanticToken {
                        start: range.start().into(),
                        len,
                        token_type: TOK_PARAMETER,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::PROPERTY => {
                if let Some(ident) = first_ident_token(&node) {
                    tokens.push(RawSemanticToken {
                        start: ident.text_range().start().into(),
                        len: utf16_len(ident.text()),
                        token_type: TOK_PROPERTY,
                        modifiers: 0,
                    });
                }
            }
            SyntaxKind::SIMPLE_SELECTOR => {
                // %placeholder → TYPE
                let mut has_percent = false;
                let mut pct_start = None;
                let mut ident_text = None;
                for element in node.children_with_tokens() {
                    if let Some(token) = element.into_token() {
                        match token.kind() {
                            SyntaxKind::PERCENT => {
                                has_percent = true;
                                pct_start = Some(token.text_range().start());
                            }
                            SyntaxKind::IDENT if has_percent => {
                                ident_text = Some(token.text().to_string());
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                if let (Some(start), Some(text)) = (pct_start, ident_text) {
                    tokens.push(RawSemanticToken {
                        start: start.into(),
                        len: 1 + utf16_len(&text), // % + name
                        token_type: TOK_TYPE,
                        modifiers: 0,
                    });
                }
            }
            _ => {}
        }
    }

    tokens.sort_by_key(|t| t.start);
    tokens
}

pub(crate) fn delta_encode(
    raw: &[RawSemanticToken],
    source: &str,
    line_index: &LineIndex,
) -> Vec<SemanticToken> {
    let mut result = Vec::with_capacity(raw.len());
    let mut prev_line: u32 = 0;
    let mut prev_col: u32 = 0;

    for tok in raw {
        let (line, col) = byte_to_lsp_pos(source, line_index, TextSize::from(tok.start));

        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 { col - prev_col } else { col };

        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.len,
            token_type: tok.token_type,
            token_modifiers_bitset: tok.modifiers,
        });

        prev_line = line;
        prev_col = col;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sass_parser::line_index::LineIndex;
    use sass_parser::syntax::SyntaxNode;

    fn parse_tokens(input: &str) -> Vec<RawSemanticToken> {
        let (green, _errors) = sass_parser::parse_scss(input);
        let root = SyntaxNode::new_root(green);
        collect_semantic_tokens(&root)
    }

    #[test]
    fn function_call() {
        let tokens = parse_tokens("$x: my-fn(1);");
        let tok = tokens
            .iter()
            .find(|t| t.token_type == TOK_FUNCTION)
            .unwrap();
        assert_eq!(tok.len, 5); // "my-fn"
        assert_eq!(tok.modifiers, 0);
    }

    #[test]
    fn function_rule() {
        let tokens = parse_tokens("@function add($a, $b) { @return $a + $b; }");
        let tok = tokens
            .iter()
            .find(|t| t.token_type == TOK_FUNCTION && t.modifiers == MOD_DECLARATION)
            .unwrap();
        assert_eq!(tok.len, 3); // "add"
    }

    #[test]
    fn mixin_rule() {
        let tokens = parse_tokens("@mixin flex { display: flex; }");
        let tok = tokens
            .iter()
            .find(|t| t.token_type == TOK_MIXIN && t.modifiers == MOD_DECLARATION)
            .unwrap();
        assert_eq!(tok.len, 4); // "flex"
    }

    #[test]
    fn include_rule() {
        let tokens = parse_tokens("@include flex;");
        let tok = tokens.iter().find(|t| t.token_type == TOK_MIXIN).unwrap();
        assert_eq!(tok.len, 4); // "flex"
        assert_eq!(tok.modifiers, 0);
    }

    #[test]
    fn param() {
        let tokens = parse_tokens("@mixin m($size) {}");
        let tok = tokens
            .iter()
            .find(|t| t.token_type == TOK_PARAMETER)
            .unwrap();
        assert_eq!(tok.len, 5); // "$size"
        assert_eq!(tok.modifiers, 0);
    }

    #[test]
    fn placeholder_selector() {
        let tokens = parse_tokens("%base { color: red; }");
        let tok = tokens.iter().find(|t| t.token_type == TOK_TYPE).unwrap();
        assert_eq!(tok.start, 0);
        assert_eq!(tok.len, 5); // "%base"
        assert_eq!(tok.modifiers, 0);
    }

    #[test]
    fn delta_encode_multiline() {
        let input = "$a: 1;\n$b: 2;";
        let tokens = parse_tokens(input);
        let line_index = LineIndex::new(input);
        let encoded = delta_encode(&tokens, input, &line_index);

        // $a on line 0, $b on line 1
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].delta_line, 0);
        assert_eq!(encoded[0].delta_start, 0);
        assert_eq!(encoded[1].delta_line, 1);
        assert_eq!(encoded[1].delta_start, 0); // column resets on new line
    }

    #[test]
    fn delta_encode_same_line() {
        let input = "$a: $b;";
        let tokens = parse_tokens(input);
        let line_index = LineIndex::new(input);
        let encoded = delta_encode(&tokens, input, &line_index);

        // $a (decl) and $b (ref) on the same line
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].delta_line, 0);
        assert_eq!(encoded[1].delta_line, 0);
        assert!(encoded[1].delta_start > 0); // relative offset
    }
}
