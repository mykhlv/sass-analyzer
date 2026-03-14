use std::collections::HashMap;

use dashmap::DashMap;
use sass_parser::line_index::LineIndex;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextSize;
use serde_json::json;
use tower_lsp_server::ls_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams, Range,
    SymbolKind, Uri,
};

use crate::DocumentState;
use crate::convert::{lsp_position_to_offset, text_range_to_lsp};
use crate::navigation::{
    extract_namespace_ref_info, find_definition_at_offset, find_reference_at_offset,
};
use crate::symbols;
use crate::workspace::ModuleGraph;

pub(crate) fn handle_prepare(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: CallHierarchyPrepareParams,
) -> Option<Vec<CallHierarchyItem>> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let (green, offset, file_symbols) = {
        let doc = documents.get(&uri)?;
        let offset = lsp_position_to_offset(&doc.text, &doc.line_index, position)?;
        (doc.green.clone(), offset, doc.symbols.clone())
    };

    let root = SyntaxNode::new_root(green);

    // Try as a reference first (cursor on a call site)
    if let Some(ref_info) = find_reference_at_offset(&root, offset) {
        if !is_callable(ref_info.kind) {
            return None;
        }
        let (target_uri, symbol) = module_graph.resolve_reference(
            &uri,
            ref_info.namespace.as_deref(),
            &ref_info.name,
            ref_info.kind,
        )?;
        let li = module_graph.line_index(&target_uri)?;
        let src = module_graph.source_text(&target_uri)?;
        return Some(vec![symbol_to_call_item(&target_uri, &symbol, &li, &src)]);
    }

    // Try as a definition (cursor on function/mixin name)
    if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
        if !is_callable(sym.kind) {
            return None;
        }
        let li = module_graph.line_index(&uri)?;
        let src = module_graph.source_text(&uri)?;
        return Some(vec![symbol_to_call_item(&uri, sym, &li, &src)]);
    }

    None
}

pub(crate) fn handle_incoming(
    module_graph: &ModuleGraph,
    params: &CallHierarchyIncomingCallsParams,
) -> Option<Vec<CallHierarchyIncomingCall>> {
    struct FileCache {
        root: Option<SyntaxNode>,
        line_index: LineIndex,
        source: String,
    }

    let item = &params.item;
    if is_file_item(item.data.as_ref()) {
        return None;
    }
    let target_kind = kind_from_data(item.data.as_ref());
    let target_uri: Uri = item.uri.clone();
    let target_name = &item.name;

    let refs = module_graph.find_all_references(&target_uri, target_name, target_kind, false);
    if refs.is_empty() {
        return None;
    }

    let mut file_cache: HashMap<Uri, Option<FileCache>> = HashMap::new();

    // Group references by their enclosing callable
    // Key: (uri, Option<(name, kind)>) — None means top-level
    #[allow(clippy::type_complexity)]
    let mut grouped: HashMap<
        (Uri, Option<(String, symbols::SymbolKind)>),
        (Option<symbols::Symbol>, Vec<Range>),
    > = HashMap::new();

    for (ref_uri, ref_range) in refs {
        let Some(cached) = file_cache
            .entry(ref_uri.clone())
            .or_insert_with(|| {
                let li = module_graph.line_index(&ref_uri)?;
                let src = module_graph.source_text(&ref_uri)?;
                let root = module_graph.get_green(&ref_uri).map(SyntaxNode::new_root);
                Some(FileCache {
                    root,
                    line_index: li,
                    source: src,
                })
            })
            .as_ref()
        else {
            continue;
        };

        let lsp_range = text_range_to_lsp(ref_range, &cached.line_index, &cached.source);

        // Find enclosing function/mixin
        let enclosing = cached.root.as_ref().and_then(|root| {
            find_enclosing_callable_in(module_graph, &ref_uri, root, ref_range.start())
        });

        let key = (
            ref_uri.clone(),
            enclosing.as_ref().map(|s| (s.name.clone(), s.kind)),
        );
        grouped
            .entry(key)
            .or_insert_with(|| (enclosing, Vec::new()))
            .1
            .push(lsp_range);
    }

    let mut results = Vec::new();
    for ((ref_uri, _), (enclosing, from_ranges)) in grouped {
        let from = if let Some(sym) = enclosing {
            // file_cache always has an entry for URIs that made it into `grouped`
            let cached = file_cache[&ref_uri].as_ref().unwrap();
            symbol_to_call_item(&ref_uri, &sym, &cached.line_index, &cached.source)
        } else {
            file_level_item(&ref_uri)
        };

        results.push(CallHierarchyIncomingCall { from, from_ranges });
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

pub(crate) fn handle_outgoing(
    module_graph: &ModuleGraph,
    params: &CallHierarchyOutgoingCallsParams,
) -> Option<Vec<CallHierarchyOutgoingCall>> {
    let item = &params.item;
    if is_file_item(item.data.as_ref()) {
        return None;
    }
    let source_kind = kind_from_data(item.data.as_ref());
    let source_uri: Uri = item.uri.clone();
    let source_name = &item.name;

    // Find the definition's byte range in the CST
    let file_symbols = module_graph.get_symbols(&source_uri)?;
    let source_sym = file_symbols
        .definitions
        .iter()
        .find(|s| s.name == *source_name && s.kind == source_kind)?;
    let sym_range = source_sym.range;

    let green = module_graph.get_green(&source_uri)?;
    let root = SyntaxNode::new_root(green);

    // Find the FUNCTION_RULE or MIXIN_RULE node at sym_range
    let callable_node = root.descendants().find(|n| {
        (n.kind() == SyntaxKind::FUNCTION_RULE || n.kind() == SyntaxKind::MIXIN_RULE)
            && n.text_range() == sym_range
    })?;

    // Collect outgoing calls within this node
    // Key: (target_uri, target_name, target_kind) → (resolved symbol, call ranges)
    let mut grouped: HashMap<(Uri, String, symbols::SymbolKind), (symbols::Symbol, Vec<Range>)> =
        HashMap::new();

    let li = module_graph.line_index(&source_uri)?;
    let src = module_graph.source_text(&source_uri)?;

    for node in callable_node.descendants() {
        // Skip calls inside nested function/mixin definitions — they belong to the inner callable
        if is_inside_nested_callable(&node, &callable_node) {
            continue;
        }

        let (namespace, name, kind) = match node.kind() {
            SyntaxKind::NAMESPACE_REF => {
                let Some(ref_info) = extract_namespace_ref_info(&node) else {
                    continue;
                };
                if !is_callable(ref_info.kind) {
                    continue;
                }
                (ref_info.namespace, ref_info.name, ref_info.kind)
            }
            SyntaxKind::FUNCTION_CALL => {
                if node
                    .parent()
                    .is_some_and(|p| p.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    continue;
                }
                let Some(ident) = node
                    .children_with_tokens()
                    .filter_map(rowan::NodeOrToken::into_token)
                    .find(|t| t.kind() == SyntaxKind::IDENT)
                else {
                    continue;
                };
                (
                    None,
                    ident.text().to_string(),
                    symbols::SymbolKind::Function,
                )
            }
            SyntaxKind::INCLUDE_RULE => {
                if node
                    .children()
                    .any(|c| c.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    continue;
                }
                let Some(ident) = node
                    .children_with_tokens()
                    .filter_map(rowan::NodeOrToken::into_token)
                    .filter(|t| t.kind() == SyntaxKind::IDENT)
                    .nth(1)
                else {
                    continue;
                };
                (None, ident.text().to_string(), symbols::SymbolKind::Mixin)
            }
            _ => continue,
        };

        let resolved =
            module_graph.resolve_reference(&source_uri, namespace.as_deref(), &name, kind);
        let Some((target_uri, target_sym)) = resolved else {
            continue;
        };

        let call_range = text_range_to_lsp(node.text_range(), &li, &src);
        grouped
            .entry((target_uri, name, kind))
            .or_insert_with(|| (target_sym, Vec::new()))
            .1
            .push(call_range);
    }

    let mut results = Vec::new();
    for ((target_uri, _, _), (target_sym, from_ranges)) in grouped {
        let Some(target_li) = module_graph.line_index(&target_uri) else {
            continue;
        };
        let Some(target_src) = module_graph.source_text(&target_uri) else {
            continue;
        };

        let to = symbol_to_call_item(&target_uri, &target_sym, &target_li, &target_src);
        results.push(CallHierarchyOutgoingCall { to, from_ranges });
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn is_callable(kind: symbols::SymbolKind) -> bool {
    matches!(
        kind,
        symbols::SymbolKind::Function | symbols::SymbolKind::Mixin
    )
}

fn is_file_item(data: Option<&serde_json::Value>) -> bool {
    data.and_then(|v| v.get("kind"))
        .and_then(serde_json::Value::as_str)
        == Some("file")
}

fn kind_from_data(data: Option<&serde_json::Value>) -> symbols::SymbolKind {
    data.and_then(|v| v.get("kind"))
        .and_then(serde_json::Value::as_str)
        .map_or(symbols::SymbolKind::Function, |k| match k {
            "mixin" => symbols::SymbolKind::Mixin,
            _ => symbols::SymbolKind::Function,
        })
}

fn symbol_to_call_item(
    uri: &Uri,
    symbol: &symbols::Symbol,
    line_index: &LineIndex,
    source: &str,
) -> CallHierarchyItem {
    let range = text_range_to_lsp(symbol.range, line_index, source);
    let selection_range = text_range_to_lsp(symbol.selection_range, line_index, source);

    let kind_str = match symbol.kind {
        symbols::SymbolKind::Mixin => "mixin",
        _ => "function",
    };

    let detail = match symbol.kind {
        symbols::SymbolKind::Mixin => Some(
            symbol
                .params
                .as_ref()
                .map_or_else(|| "@mixin".to_owned(), |p| format!("@mixin{p}")),
        ),
        symbols::SymbolKind::Function => symbol.params.clone(),
        _ => None,
    };

    CallHierarchyItem {
        name: symbol.name.clone(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        detail,
        uri: uri.clone(),
        range,
        selection_range,
        data: Some(json!({"kind": kind_str})),
    }
}

fn file_level_item(uri: &Uri) -> CallHierarchyItem {
    let path_str = uri.path().as_str();
    let name = path_str.rsplit('/').next().unwrap_or("file").to_string();

    CallHierarchyItem {
        name,
        kind: SymbolKind::FILE,
        tags: None,
        detail: None,
        uri: uri.clone(),
        range: Range::default(),
        selection_range: Range::default(),
        data: Some(json!({"kind": "file"})),
    }
}

fn find_enclosing_callable_in(
    module_graph: &ModuleGraph,
    uri: &Uri,
    root: &SyntaxNode,
    offset: TextSize,
) -> Option<symbols::Symbol> {
    let token = root.token_at_offset(offset).right_biased()?;

    for node in token.parent()?.ancestors() {
        match node.kind() {
            SyntaxKind::FUNCTION_RULE | SyntaxKind::MIXIN_RULE => {
                let node_range = node.text_range();
                let syms = module_graph.get_symbols(uri)?;
                return syms
                    .definitions
                    .iter()
                    .find(|s| s.range == node_range && is_callable(s.kind))
                    .cloned();
            }
            _ => {}
        }
    }

    None
}

fn is_inside_nested_callable(node: &SyntaxNode, root_callable: &SyntaxNode) -> bool {
    node.ancestors()
        .take_while(|ancestor| *ancestor != *root_callable)
        .any(|ancestor| {
            ancestor.kind() == SyntaxKind::FUNCTION_RULE
                || ancestor.kind() == SyntaxKind::MIXIN_RULE
        })
}
