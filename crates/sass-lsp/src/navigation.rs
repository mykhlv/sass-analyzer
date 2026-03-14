use std::collections::HashMap;

use dashmap::DashMap;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;
use tower_lsp_server::ls_types::{
    DocumentLink, DocumentLinkParams, GotoDefinitionParams, GotoDefinitionResponse, Location,
    PrepareRenameResponse, ReferenceParams, RenameParams, TextDocumentPositionParams, TextEdit,
    Uri, WorkspaceEdit,
};

use crate::DocumentState;
use crate::ast_helpers::{
    dollar_ident_name_range, ident_text_range_of, name_only_range, nth_ident_text_range_of,
    percent_ident_name_range,
};
use crate::convert::{lsp_position_to_offset, text_range_to_lsp};
use crate::symbols;
use crate::workspace::ModuleGraph;

pub(crate) struct ReferenceInfo {
    pub(crate) namespace: Option<String>,
    pub(crate) name: String,
    pub(crate) kind: symbols::SymbolKind,
    pub(crate) range: TextRange,
}

pub(crate) fn find_reference_at_offset(
    root: &SyntaxNode,
    offset: sass_parser::text_range::TextSize,
) -> Option<ReferenceInfo> {
    let token = root.token_at_offset(offset).right_biased()?;

    for node in token.parent()?.ancestors() {
        match node.kind() {
            SyntaxKind::NAMESPACE_REF => {
                return extract_namespace_ref_info(&node);
            }
            SyntaxKind::VARIABLE_REF => {
                if node
                    .parent()
                    .is_some_and(|p| p.kind() == SyntaxKind::VARIABLE_DECL)
                {
                    return None;
                }
                let (name, range) = dollar_ident_name_range(&node)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Variable,
                    range,
                });
            }
            SyntaxKind::FUNCTION_CALL => {
                if node
                    .parent()
                    .is_some_and(|p| p.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    continue;
                }
                let (name, range) = ident_text_range_of(&node)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Function,
                    range,
                });
            }
            SyntaxKind::INCLUDE_RULE => {
                if node
                    .children()
                    .any(|c| c.kind() == SyntaxKind::NAMESPACE_REF)
                {
                    return None;
                }
                let (name, range) = nth_ident_text_range_of(&node, 1)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Mixin,
                    range,
                });
            }
            SyntaxKind::EXTEND_RULE => {
                let (name, range) = percent_ident_name_range(&node)?;
                return Some(ReferenceInfo {
                    namespace: None,
                    name,
                    kind: symbols::SymbolKind::Placeholder,
                    range,
                });
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn extract_namespace_ref_info(node: &SyntaxNode) -> Option<ReferenceInfo> {
    let tokens: Vec<_> = node
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .collect();

    let namespace = tokens
        .iter()
        .find(|t| t.kind() == SyntaxKind::IDENT)?
        .text()
        .to_string();

    // ns.$var pattern: IDENT DOT DOLLAR IDENT
    if let Some(dollar) = tokens.iter().find(|t| t.kind() == SyntaxKind::DOLLAR) {
        let ident = tokens
            .iter()
            .skip_while(|t| t.kind() != SyntaxKind::DOLLAR)
            .find(|t| t.kind() == SyntaxKind::IDENT)?;
        let range = TextRange::new(dollar.text_range().start(), ident.text_range().end());
        return Some(ReferenceInfo {
            namespace: Some(namespace),
            name: ident.text().to_string(),
            kind: symbols::SymbolKind::Variable,
            range,
        });
    }

    // ns.func() pattern: has FUNCTION_CALL child
    if let Some(func_call) = node
        .children()
        .find(|c| c.kind() == SyntaxKind::FUNCTION_CALL)
    {
        let (name, range) = ident_text_range_of(&func_call)?;
        return Some(ReferenceInfo {
            namespace: Some(namespace),
            name,
            kind: symbols::SymbolKind::Function,
            range,
        });
    }

    // ns.mixin pattern: IDENT DOT IDENT (inside @include)
    let dot_pos = tokens.iter().position(|t| t.kind() == SyntaxKind::DOT)?;
    let ident = tokens[dot_pos + 1..]
        .iter()
        .find(|t| t.kind() == SyntaxKind::IDENT)?;

    let is_mixin = node
        .parent()
        .is_some_and(|p| p.kind() == SyntaxKind::INCLUDE_RULE);

    Some(ReferenceInfo {
        namespace: Some(namespace),
        name: ident.text().to_string(),
        kind: if is_mixin {
            symbols::SymbolKind::Mixin
        } else {
            symbols::SymbolKind::Function
        },
        range: ident.text_range(),
    })
}

pub(crate) fn find_definition_at_offset(
    symbols: &symbols::FileSymbols,
    offset: sass_parser::text_range::TextSize,
) -> Option<&symbols::Symbol> {
    symbols
        .definitions
        .iter()
        .find(|s| s.selection_range.contains(offset))
}

// ── Handlers ────────────────────────────────────────────────────────

pub(crate) fn handle_goto_definition(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let (green, offset) = {
        let doc = documents.get(&uri)?;
        let offset = lsp_position_to_offset(&doc.text, &doc.line_index, position)?;
        (doc.green.clone(), offset)
    };

    let root = SyntaxNode::new_root(green);

    // If cursor is on the import path of @use/@forward/@import, jump to the file
    if let Some(resp) = try_goto_import(&root, offset, &uri, module_graph) {
        return Some(resp);
    }

    let ref_info = find_reference_at_offset(&root, offset)?;

    let resolved = module_graph.resolve_reference(
        &uri,
        ref_info.namespace.as_deref(),
        &ref_info.name,
        ref_info.kind,
    );

    let (target_uri, symbol) = resolved?;

    let target_line_index = module_graph.line_index(&target_uri)?;
    let target_source = module_graph.source_text(&target_uri)?;

    let range = text_range_to_lsp(symbol.selection_range, &target_line_index, &target_source);
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: target_uri,
        range,
    }))
}

fn try_goto_import(
    root: &SyntaxNode,
    offset: sass_parser::text_range::TextSize,
    uri: &Uri,
    module_graph: &ModuleGraph,
) -> Option<GotoDefinitionResponse> {
    let token = root.token_at_offset(offset).right_biased()?;
    if token.kind() != SyntaxKind::QUOTED_STRING {
        return None;
    }
    let parent = token.parent()?;
    if !matches!(
        parent.kind(),
        SyntaxKind::USE_RULE | SyntaxKind::FORWARD_RULE | SyntaxKind::IMPORT_RULE
    ) {
        return None;
    }
    let text = token.text();
    if text.len() < 2 {
        return None;
    }
    let spec = &text[1..text.len() - 1];
    let target_uri = module_graph.resolve_import(uri, spec)?;
    let range = tower_lsp_server::ls_types::Range::default();
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: target_uri,
        range,
    }))
}

pub(crate) fn handle_document_link(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: DocumentLinkParams,
) -> Option<Vec<DocumentLink>> {
    let uri = params.text_document.uri;
    let doc = documents.get(&uri)?;

    let root = SyntaxNode::new_root(doc.green.clone());
    let line_index = &doc.line_index;
    let mut links = Vec::new();

    for node in root.descendants() {
        let kind = node.kind();
        if kind != SyntaxKind::USE_RULE
            && kind != SyntaxKind::FORWARD_RULE
            && kind != SyntaxKind::IMPORT_RULE
        {
            continue;
        }

        let Some(string_token) = node
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .find(|t| t.kind() == SyntaxKind::QUOTED_STRING)
        else {
            continue;
        };

        let text = string_token.text();
        if text.len() < 2 {
            continue;
        }
        let spec = &text[1..text.len() - 1];

        let Some(target_uri) = module_graph.resolve_import(&uri, spec) else {
            continue;
        };

        let range = text_range_to_lsp(string_token.text_range(), line_index, &doc.text);
        links.push(DocumentLink {
            range,
            target: Some(target_uri),
            tooltip: Some(spec.to_owned()),
            data: None,
        });
    }

    if links.is_empty() { None } else { Some(links) }
}

pub(crate) fn handle_references(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: ReferenceParams,
) -> Option<Vec<Location>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let (green, offset, file_symbols) = {
        let doc = documents.get(&uri)?;
        let offset = lsp_position_to_offset(&doc.text, &doc.line_index, position)?;
        (doc.green.clone(), offset, doc.symbols.clone())
    };

    let root = SyntaxNode::new_root(green);

    let (target_uri, target_name, target_kind) =
        if let Some(ref_info) = find_reference_at_offset(&root, offset) {
            let resolved = module_graph.resolve_reference(
                &uri,
                ref_info.namespace.as_deref(),
                &ref_info.name,
                ref_info.kind,
            );
            let (target_uri, sym) = resolved?;
            (target_uri, sym.name, sym.kind)
        } else if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
            (uri.clone(), sym.name.clone(), sym.kind)
        } else {
            return None;
        };

    let refs = module_graph.find_all_references(
        &target_uri,
        &target_name,
        target_kind,
        params.context.include_declaration,
    );

    if refs.is_empty() {
        return None;
    }

    let locations: Vec<Location> = refs
        .into_iter()
        .filter_map(|(ref_uri, range)| {
            let li = module_graph.line_index(&ref_uri)?;
            let src = module_graph.source_text(&ref_uri)?;
            Some(Location {
                uri: ref_uri,
                range: text_range_to_lsp(range, &li, &src),
            })
        })
        .collect();

    Some(locations)
}

pub(crate) fn handle_prepare_rename(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: TextDocumentPositionParams,
) -> Option<PrepareRenameResponse> {
    let uri = params.text_document.uri;
    let position = params.position;

    let (green, offset, file_symbols) = {
        let doc = documents.get(&uri)?;
        let offset = lsp_position_to_offset(&doc.text, &doc.line_index, position)?;
        (doc.green.clone(), offset, doc.symbols.clone())
    };

    let root = SyntaxNode::new_root(green);

    // Check if cursor is on a reference or definition
    if let Some(ref_info) = find_reference_at_offset(&root, offset) {
        let resolved = module_graph.resolve_reference(
            &uri,
            ref_info.namespace.as_deref(),
            &ref_info.name,
            ref_info.kind,
        );
        let (_, sym) = resolved?;
        let li = module_graph.line_index(&uri)?;
        let src = module_graph.source_text(&uri)?;
        let nr = name_only_range(ref_info.kind, ref_info.range);
        return Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: text_range_to_lsp(nr, &li, &src),
            placeholder: sym.name,
        });
    }

    if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
        let li = module_graph.line_index(&uri)?;
        let src = module_graph.source_text(&uri)?;
        let nr = name_only_range(sym.kind, sym.selection_range);
        return Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: text_range_to_lsp(nr, &li, &src),
            placeholder: sym.name.clone(),
        });
    }

    None
}

pub(crate) fn handle_rename(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: RenameParams,
) -> tower_lsp_server::jsonrpc::Result<Option<WorkspaceEdit>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let new_name = params.new_name;

    let (green, offset, file_symbols) = {
        let Some(doc) = documents.get(&uri) else {
            return Ok(None);
        };
        let Some(offset) = lsp_position_to_offset(&doc.text, &doc.line_index, position) else {
            return Ok(None);
        };
        (doc.green.clone(), offset, doc.symbols.clone())
    };

    let root = SyntaxNode::new_root(green);

    let (target_uri, target_name, target_kind) =
        if let Some(ref_info) = find_reference_at_offset(&root, offset) {
            let resolved = module_graph.resolve_reference(
                &uri,
                ref_info.namespace.as_deref(),
                &ref_info.name,
                ref_info.kind,
            );
            let Some((target_uri, sym)) = resolved else {
                return Ok(None);
            };
            (target_uri, sym.name, sym.kind)
        } else if let Some(sym) = find_definition_at_offset(&file_symbols, offset) {
            (uri.clone(), sym.name.clone(), sym.kind)
        } else {
            return Ok(None);
        };

    // Conflict detection: check if new_name already exists in the target file
    if module_graph.check_name_conflict(&target_uri, &new_name, target_kind) {
        let kind_label = match target_kind {
            symbols::SymbolKind::Variable => "variable",
            symbols::SymbolKind::Function => "function",
            symbols::SymbolKind::Mixin => "mixin",
            symbols::SymbolKind::Placeholder => "placeholder",
        };
        let sigil = if target_kind == symbols::SymbolKind::Variable {
            "$"
        } else if target_kind == symbols::SymbolKind::Placeholder {
            "%"
        } else {
            ""
        };
        return Err(tower_lsp_server::jsonrpc::Error {
            code: tower_lsp_server::jsonrpc::ErrorCode::InvalidParams,
            message: format!("A {kind_label} '{sigil}{new_name}' already exists in this scope")
                .into(),
            data: None,
        });
    }

    // Find all references + declaration
    let refs = module_graph.find_all_references(
        &target_uri,
        &target_name,
        target_kind,
        true, // always include declaration for rename
    );

    if refs.is_empty() {
        return Ok(None);
    }

    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    for (ref_uri, range) in refs {
        let Some(li) = module_graph.line_index(&ref_uri) else {
            continue;
        };
        let Some(src) = module_graph.source_text(&ref_uri) else {
            continue;
        };
        let edit_range = name_only_range(target_kind, range);
        changes.entry(ref_uri).or_default().push(TextEdit {
            range: text_range_to_lsp(edit_range, &li, &src),
            new_text: new_name.clone(),
        });
    }

    // Update @forward show/hide clauses that mention the old name
    let forward_refs =
        module_graph.find_forward_show_hide_references(&target_uri, &target_name, target_kind);
    for (fwd_uri, range) in forward_refs {
        let Some(li) = module_graph.line_index(&fwd_uri) else {
            continue;
        };
        let Some(src) = module_graph.source_text(&fwd_uri) else {
            continue;
        };
        changes.entry(fwd_uri).or_default().push(TextEdit {
            range: text_range_to_lsp(range, &li, &src),
            new_text: new_name.clone(),
        });
    }

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        ..WorkspaceEdit::default()
    }))
}

#[allow(deprecated)]
pub(crate) fn to_lsp_document_symbol(
    sym: &symbols::Symbol,
    line_index: &sass_parser::line_index::LineIndex,
    source: &str,
) -> tower_lsp_server::ls_types::DocumentSymbol {
    let range = text_range_to_lsp(sym.range, line_index, source);
    let selection_range = text_range_to_lsp(sym.selection_range, line_index, source);
    let (kind, detail) = match sym.kind {
        symbols::SymbolKind::Variable => (tower_lsp_server::ls_types::SymbolKind::VARIABLE, None),
        symbols::SymbolKind::Function => (
            tower_lsp_server::ls_types::SymbolKind::FUNCTION,
            sym.params.clone(),
        ),
        symbols::SymbolKind::Mixin => (
            tower_lsp_server::ls_types::SymbolKind::FUNCTION,
            Some(
                sym.params
                    .as_ref()
                    .map_or_else(|| "@mixin".to_owned(), |p| format!("@mixin{p}")),
            ),
        ),
        symbols::SymbolKind::Placeholder => (tower_lsp_server::ls_types::SymbolKind::CLASS, None),
    };
    tower_lsp_server::ls_types::DocumentSymbol {
        name: sym.name.clone(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: None,
    }
}
