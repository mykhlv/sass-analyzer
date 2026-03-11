use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use tower_lsp_server::ls_types::{
    MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, SignatureInformation,
};

use crate::ast_helpers::{ident_text_range_of, nth_ident_text_range_of};
use crate::symbols;

// ── Signature help ──────────────────────────────────────────────────

pub(crate) struct CallInfo {
    pub(crate) namespace: Option<String>,
    pub(crate) name: String,
    pub(crate) kind: symbols::SymbolKind,
    pub(crate) arg_list_start: sass_parser::text_range::TextSize,
}

pub(crate) fn find_call_at_offset(
    root: &SyntaxNode,
    offset: sass_parser::text_range::TextSize,
) -> Option<CallInfo> {
    let token = root.token_at_offset(offset).left_biased()?;

    for node in token.parent()?.ancestors() {
        match node.kind() {
            SyntaxKind::FUNCTION_CALL => {
                let arg_list = node.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)?;
                if !arg_list.text_range().contains(offset) {
                    continue;
                }

                // Check if inside a NAMESPACE_REF parent
                if let Some(ns_ref) = node
                    .parent()
                    .filter(|p| p.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    let ns_name = ns_ref
                        .children_with_tokens()
                        .filter_map(rowan::NodeOrToken::into_token)
                        .find(|t| t.kind() == SyntaxKind::IDENT)?
                        .text()
                        .to_string();
                    let func_name = ident_text_range_of(&node)?.0;
                    return Some(CallInfo {
                        namespace: Some(ns_name),
                        name: func_name,
                        kind: symbols::SymbolKind::Function,
                        arg_list_start: arg_list.text_range().start(),
                    });
                }

                let func_name = ident_text_range_of(&node)?.0;
                return Some(CallInfo {
                    namespace: None,
                    name: func_name,
                    kind: symbols::SymbolKind::Function,
                    arg_list_start: arg_list.text_range().start(),
                });
            }
            SyntaxKind::INCLUDE_RULE => {
                let arg_list = node.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)?;
                if !arg_list.text_range().contains(offset) {
                    continue;
                }

                // Check if has a NAMESPACE_REF child
                if let Some(ns_ref) = node
                    .children()
                    .find(|c| c.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    let tokens: Vec<_> = ns_ref
                        .children_with_tokens()
                        .filter_map(rowan::NodeOrToken::into_token)
                        .collect();
                    let ns_name = tokens
                        .iter()
                        .find(|t| t.kind() == SyntaxKind::IDENT)?
                        .text()
                        .to_string();
                    let dot_pos = tokens.iter().position(|t| t.kind() == SyntaxKind::DOT)?;
                    let mixin_name = tokens[dot_pos + 1..]
                        .iter()
                        .find(|t| t.kind() == SyntaxKind::IDENT)?
                        .text()
                        .to_string();
                    return Some(CallInfo {
                        namespace: Some(ns_name),
                        name: mixin_name,
                        kind: symbols::SymbolKind::Mixin,
                        arg_list_start: arg_list.text_range().start(),
                    });
                }

                let mixin_name = nth_ident_text_range_of(&node, 1)?.0;
                return Some(CallInfo {
                    namespace: None,
                    name: mixin_name,
                    kind: symbols::SymbolKind::Mixin,
                    arg_list_start: arg_list.text_range().start(),
                });
            }
            _ => {}
        }
    }
    None
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn count_active_parameter(
    source: &str,
    call_info: &CallInfo,
    cursor: sass_parser::text_range::TextSize,
) -> u32 {
    let start = u32::from(call_info.arg_list_start) as usize;
    let cursor_pos = u32::from(cursor) as usize;
    if cursor_pos <= start {
        return 0;
    }

    let slice = &source[start..cursor_pos];

    // Count commas that are not inside nested parens/brackets
    let mut depth = 0u32;
    let mut commas = 0u32;
    for ch in slice.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 1 => commas += 1,
            _ => {}
        }
    }
    commas
}

pub(crate) fn build_signature_info(
    sym: &symbols::Symbol,
    params_text: &str,
) -> SignatureInformation {
    let label = match sym.kind {
        symbols::SymbolKind::Function => format!("@function {}{params_text}", sym.name),
        symbols::SymbolKind::Mixin => format!("@mixin {}{params_text}", sym.name),
        _ => {
            return SignatureInformation {
                label: sym.name.clone(),
                documentation: None,
                parameters: None,
                active_parameter: None,
            };
        }
    };

    let parameters = parse_param_labels(&label, params_text);

    let documentation = sym.doc.as_ref().map(|d| {
        tower_lsp_server::ls_types::Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d.clone(),
        })
    });

    SignatureInformation {
        label,
        documentation,
        parameters: Some(parameters),
        active_parameter: None,
    }
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn parse_param_labels(signature: &str, params_text: &str) -> Vec<ParameterInformation> {
    // Find the offset of params_text within the signature
    let Some(params_offset) = signature.find(params_text) else {
        return Vec::new();
    };

    // Strip outer parens
    let inner = params_text
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(params_text);

    if inner.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    // +1 for the opening paren
    let content_offset = params_offset + 1;

    // Split by commas at depth 0 (handle nested parens in defaults)
    let mut depth = 0u32;
    let mut segment_start = 0;

    for (i, ch) in inner.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let param = inner[segment_start..i].trim();
                if !param.is_empty() {
                    let abs_start = content_offset + segment_start;
                    let abs_end = content_offset + segment_start + param.len();
                    result.push(ParameterInformation {
                        label: ParameterLabel::LabelOffsets([abs_start as u32, abs_end as u32]),
                        documentation: None,
                    });
                }
                segment_start = i + 1;
                // Skip whitespace after comma
                for (j, c) in inner[segment_start..].char_indices() {
                    if c != ' ' {
                        segment_start += j;
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    // Last segment
    let param = inner[segment_start..].trim();
    if !param.is_empty() {
        let abs_start = content_offset + segment_start;
        let abs_end = content_offset + segment_start + param.len();
        result.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([abs_start as u32, abs_end as u32]),
            documentation: None,
        });
    }

    result
}
