use dashmap::DashMap;
use sass_parser::line_index::LineIndex;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextSize;
use tower_lsp_server::ls_types::{
    InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, Position, Uri,
};

use crate::DocumentState;
use crate::ast_helpers::{ident_text_range_of, nth_ident_text_range_of};
use crate::convert::{byte_to_lsp_pos, lsp_position_to_offset};
use crate::symbols::SymbolKind;
use crate::workspace::ModuleGraph;

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn handle(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: InlayHintParams,
) -> Option<Vec<InlayHint>> {
    let uri = params.text_document.uri;

    let (green, text, line_index) = {
        let doc = documents.get(&uri)?;
        let li = doc.line_index.clone();
        (doc.green.clone(), doc.text.clone(), li)
    };

    let range_start =
        lsp_position_to_offset(&text, &line_index, params.range.start).unwrap_or(TextSize::from(0));
    let range_end = lsp_position_to_offset(&text, &line_index, params.range.end)
        .unwrap_or(TextSize::from(text.len() as u32));

    let root = SyntaxNode::new_root(green);
    let mut hints = Vec::new();

    for node in root.descendants() {
        let node_range = node.text_range();
        if node_range.end() <= range_start || node_range.start() >= range_end {
            continue;
        }

        match node.kind() {
            SyntaxKind::FUNCTION_CALL => {
                let Some(arg_list) = node.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)
                else {
                    continue;
                };

                let (namespace, name) = extract_call_name(&node);
                if let Some(param_hints) = resolve_and_build_hints(
                    &uri,
                    module_graph,
                    namespace.as_deref(),
                    &name,
                    SymbolKind::Function,
                    &arg_list,
                    &text,
                    &line_index,
                ) {
                    hints.extend(param_hints);
                }
            }
            SyntaxKind::INCLUDE_RULE => {
                let Some(arg_list) = node.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)
                else {
                    continue;
                };

                let (namespace, name) = extract_include_name(&node);
                let Some(name) = name else { continue };
                if let Some(param_hints) = resolve_and_build_hints(
                    &uri,
                    module_graph,
                    namespace.as_deref(),
                    &name,
                    SymbolKind::Mixin,
                    &arg_list,
                    &text,
                    &line_index,
                ) {
                    hints.extend(param_hints);
                }
            }
            _ => {}
        }
    }

    if hints.is_empty() { None } else { Some(hints) }
}

fn extract_call_name(func_call: &SyntaxNode) -> (Option<String>, String) {
    if let Some(ns_ref) = func_call
        .parent()
        .filter(|p| p.kind() == SyntaxKind::NAMESPACE_REF)
    {
        let ns_name = ns_ref
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .find(|t| t.kind() == SyntaxKind::IDENT)
            .map(|t| t.text().to_string());
        let func_name = ident_text_range_of(func_call)
            .map(|(n, _)| n)
            .unwrap_or_default();
        return (ns_name, func_name);
    }

    let func_name = ident_text_range_of(func_call)
        .map(|(n, _)| n)
        .unwrap_or_default();
    (None, func_name)
}

fn extract_include_name(include_rule: &SyntaxNode) -> (Option<String>, Option<String>) {
    if let Some(ns_ref) = include_rule
        .children()
        .find(|c| c.kind() == SyntaxKind::NAMESPACE_REF)
    {
        let tokens: Vec<_> = ns_ref
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .collect();
        let ns_name = tokens
            .iter()
            .find(|t| t.kind() == SyntaxKind::IDENT)
            .map(|t| t.text().to_string());
        let dot_pos = tokens.iter().position(|t| t.kind() == SyntaxKind::DOT);
        let mixin_name = dot_pos.and_then(|dp| {
            tokens[dp + 1..]
                .iter()
                .find(|t| t.kind() == SyntaxKind::IDENT)
                .map(|t| t.text().to_string())
        });
        return (ns_name, mixin_name);
    }

    let name = nth_ident_text_range_of(include_rule, 1).map(|(n, _)| n);
    (None, name)
}

#[allow(clippy::too_many_arguments)]
fn resolve_and_build_hints(
    uri: &Uri,
    module_graph: &ModuleGraph,
    namespace: Option<&str>,
    name: &str,
    kind: SymbolKind,
    arg_list: &SyntaxNode,
    source: &str,
    line_index: &LineIndex,
) -> Option<Vec<InlayHint>> {
    let (_target_uri, symbol) = module_graph.resolve_reference(uri, namespace, name, kind)?;
    let params_text = symbol.params.as_ref()?;
    let param_names = parse_param_names(params_text);

    if param_names.len() <= 1 {
        return None;
    }

    let args: Vec<_> = arg_list
        .children()
        .filter(|c| c.kind() == SyntaxKind::ARG)
        .collect();

    let mut hints = Vec::new();
    let mut param_idx = 0;

    #[allow(clippy::explicit_counter_loop)] // param_idx skips keyword args — not a simple counter
    for arg in &args {
        if param_idx >= param_names.len() {
            break;
        }

        if is_keyword_arg(arg) || is_splat_arg(arg) {
            break;
        }

        if param_names[param_idx].ends_with("...") {
            break;
        }

        let arg_start = arg.text_range().start();
        let content_start = arg
            .children_with_tokens()
            .find(|ct| !ct.kind().is_trivia())
            .map_or(arg_start, |ct| ct.text_range().start());

        let (line, col) = byte_to_lsp_pos(source, line_index, content_start);

        hints.push(InlayHint {
            position: Position::new(line, col),
            label: InlayHintLabel::String(format!("{}:", param_names[param_idx])),
            kind: Some(InlayHintKind::PARAMETER),
            text_edits: None,
            tooltip: None,
            padding_left: Some(false),
            padding_right: Some(true),
            data: None,
        });

        param_idx += 1;
    }

    if hints.is_empty() { None } else { Some(hints) }
}

fn is_keyword_arg(arg: &SyntaxNode) -> bool {
    let mut tokens = arg
        .children_with_tokens()
        .filter(|ct| !ct.kind().is_trivia())
        .filter_map(rowan::NodeOrToken::into_token);

    let Some(first) = tokens.next() else {
        return false;
    };
    if first.kind() != SyntaxKind::DOLLAR {
        return false;
    }
    let Some(second) = tokens.next() else {
        return false;
    };
    if second.kind() != SyntaxKind::IDENT {
        return false;
    }
    let Some(third) = tokens.next() else {
        return false;
    };
    third.kind() == SyntaxKind::COLON
}

fn is_splat_arg(arg: &SyntaxNode) -> bool {
    arg.children_with_tokens()
        .any(|ct| ct.kind() == SyntaxKind::DOT_DOT_DOT)
}

fn parse_param_names(params_text: &str) -> Vec<String> {
    let inner = params_text
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(params_text);

    if inner.is_empty() {
        return Vec::new();
    }

    let mut names = Vec::new();
    let mut depth = 0u32;
    let mut segment_start = 0;

    for (i, ch) in inner.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(name) = extract_name_from_param(inner[segment_start..i].trim()) {
                    names.push(name);
                }
                segment_start = i + 1;
            }
            _ => {}
        }
    }

    if let Some(name) = extract_name_from_param(inner[segment_start..].trim()) {
        names.push(name);
    }

    names
}

fn extract_name_from_param(param: &str) -> Option<String> {
    if param.is_empty() {
        return None;
    }
    let name_part = if let Some(colon_pos) = param.find(':') {
        param[..colon_pos].trim()
    } else if let Some(dots_pos) = param.find("...") {
        return Some(param[..dots_pos + 3].trim().to_string());
    } else {
        param.trim()
    };
    Some(name_part.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_param_names_basic() {
        let names = parse_param_names("($a, $b)");
        assert_eq!(names, vec!["$a", "$b"]);
    }

    #[test]
    fn test_parse_param_names_defaults() {
        let names = parse_param_names("($a, $b: 10px, $c: red)");
        assert_eq!(names, vec!["$a", "$b", "$c"]);
    }

    #[test]
    fn test_parse_param_names_rest() {
        let names = parse_param_names("($a, $rest...)");
        assert_eq!(names, vec!["$a", "$rest..."]);
    }

    #[test]
    fn test_parse_param_names_empty() {
        let names = parse_param_names("()");
        assert!(names.is_empty());
    }

    #[test]
    fn test_parse_param_names_nested_defaults() {
        let names = parse_param_names("($a, $b: fn(1, 2), $c)");
        assert_eq!(names, vec!["$a", "$b", "$c"]);
    }
}
