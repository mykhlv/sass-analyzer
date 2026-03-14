use dashmap::DashMap;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use tower_lsp_server::ls_types::{
    MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, SignatureHelp,
    SignatureHelpParams, SignatureInformation, Uri,
};

use crate::DocumentState;
use crate::ast_helpers::{ident_text_range_of, nth_ident_text_range_of};
use crate::convert::lsp_position_to_offset;
use crate::sassdoc;
use crate::symbols;
use crate::workspace::ModuleGraph;

// ── Handler ─────────────────────────────────────────────────────────

pub(crate) fn handle(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: SignatureHelpParams,
) -> Option<SignatureHelp> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let (green, text, offset) = {
        let doc = documents.get(&uri)?;
        let offset = lsp_position_to_offset(&doc.text, &doc.line_index, position)?;
        (doc.green.clone(), doc.text.clone(), offset)
    };

    let root = SyntaxNode::new_root(green);

    let call_info = find_call_at_offset(&root, offset)?;

    let resolved = module_graph.resolve_reference(
        &uri,
        call_info.namespace.as_deref(),
        &call_info.name,
        call_info.kind,
    );

    let (_target_uri, symbol) = resolved?;

    let params_text = symbol.params.as_ref()?;

    let active_param = count_active_parameter(&text, &call_info, offset);

    let sig_info = build_signature_info(&symbol, params_text);

    Some(SignatureHelp {
        signatures: vec![sig_info],
        active_signature: Some(0),
        active_parameter: Some(active_param),
    })
}

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

    let parsed_doc = sym
        .doc
        .as_ref()
        .filter(|d| sassdoc::has_annotations(d))
        .map(|d| sassdoc::parse(d));

    let mut parameters = parse_param_labels(&label, params_text);

    // Attach @param descriptions to parameter info
    if let Some(ref sassdoc) = parsed_doc {
        for param_info in &mut parameters {
            if let ParameterLabel::LabelOffsets([start, end]) = param_info.label {
                let param_text = utf16_slice(&label, start, end);
                // Extract bare param name (strip $, default values, ...)
                let bare_name = param_text
                    .strip_prefix('$')
                    .unwrap_or(&param_text)
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .split("...")
                    .next()
                    .unwrap_or("")
                    .trim();
                if let Some(pdoc) = sassdoc.params.iter().find(|p| p.name == bare_name) {
                    let mut doc_parts = Vec::new();
                    if let Some(ty) = &pdoc.type_annotation {
                        doc_parts.push(format!("`{{{ty}}}`"));
                    }
                    if let Some(desc) = &pdoc.description {
                        doc_parts.push(desc.clone());
                    }
                    if !doc_parts.is_empty() {
                        param_info.documentation =
                            Some(tower_lsp_server::ls_types::Documentation::MarkupContent(
                                MarkupContent {
                                    kind: MarkupKind::Markdown,
                                    value: doc_parts.join(" — "),
                                },
                            ));
                    }
                }
            }
        }
    }

    let documentation = sym.doc.as_ref().map(|d| {
        let value = if let Some(ref sassdoc) = parsed_doc {
            sassdoc::format_markdown(sassdoc)
        } else {
            d.clone()
        };
        tower_lsp_server::ls_types::Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
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
fn utf16_slice(s: &str, start: u32, end: u32) -> String {
    let start = start as usize;
    let end = end as usize;
    let utf16: Vec<u16> = s.encode_utf16().collect();
    if start >= utf16.len() || end > utf16.len() {
        return String::new();
    }
    String::from_utf16_lossy(&utf16[start..end])
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
                    let utf16_start = byte_offset_to_utf16(&signature[..abs_start]);
                    let utf16_end = byte_offset_to_utf16(&signature[..abs_end]);
                    result.push(ParameterInformation {
                        label: ParameterLabel::LabelOffsets([utf16_start, utf16_end]),
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
        let utf16_start = byte_offset_to_utf16(&signature[..abs_start]);
        let utf16_end = byte_offset_to_utf16(&signature[..abs_end]);
        result.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([utf16_start, utf16_end]),
            documentation: None,
        });
    }

    result
}

#[allow(clippy::cast_possible_truncation)]
fn byte_offset_to_utf16(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}
