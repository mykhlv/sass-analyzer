use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp_server::ls_types::{
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, Uri,
};

use sass_parser::line_index::LineIndex;
use sass_parser::syntax::SyntaxNode;

use crate::DocumentState;
use crate::convert::{lsp_position_to_offset, text_range_to_lsp};
use crate::navigation::{find_definition_at_offset, find_reference_at_offset};
use crate::symbols::{FileSymbols, RefKind, SymbolKind};

pub(crate) fn handle_document_highlight(
    documents: &DashMap<Uri, DocumentState>,
    params: DocumentHighlightParams,
) -> Option<Vec<DocumentHighlight>> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let (green, text, line_index, symbols) = {
        let doc = documents.get(&uri)?;
        (
            doc.green.clone(),
            doc.text.clone(),
            doc.line_index.clone(),
            Arc::clone(&doc.symbols),
        )
    };

    let offset = lsp_position_to_offset(&text, &line_index, position)?;
    let root = SyntaxNode::new_root(green);

    let (name, kind) = if let Some(ref_info) = find_reference_at_offset(&root, offset) {
        // Namespaced references (ns.$var, ns.func()) are cross-file — no same-file highlights
        if ref_info.namespace.is_some() {
            return None;
        }
        (ref_info.name, ref_info.kind)
    } else if let Some(sym) = find_definition_at_offset(&symbols, offset) {
        (sym.name.clone(), sym.kind)
    } else {
        return None;
    };

    collect_highlights(&name, kind, &symbols, &line_index, &text)
}

fn collect_highlights(
    name: &str,
    kind: SymbolKind,
    symbols: &FileSymbols,
    line_index: &LineIndex,
    text: &str,
) -> Option<Vec<DocumentHighlight>> {
    let mut highlights = Vec::new();

    for def in &symbols.definitions {
        if def.name == name && def.kind == kind {
            highlights.push(DocumentHighlight {
                range: text_range_to_lsp(def.selection_range, line_index, text),
                kind: Some(DocumentHighlightKind::WRITE),
            });
        }
    }

    let ref_kind = symbol_kind_to_ref_kind(kind);
    for reference in &symbols.references {
        if reference.name == name && reference.kind == ref_kind {
            highlights.push(DocumentHighlight {
                range: text_range_to_lsp(reference.selection_range, line_index, text),
                kind: Some(DocumentHighlightKind::READ),
            });
        }
    }

    if highlights.is_empty() {
        None
    } else {
        highlights.sort_by_key(|h| (h.range.start.line, h.range.start.character));
        Some(highlights)
    }
}

fn symbol_kind_to_ref_kind(kind: SymbolKind) -> RefKind {
    match kind {
        SymbolKind::Variable => RefKind::Variable,
        SymbolKind::Function => RefKind::Function,
        SymbolKind::Mixin => RefKind::Mixin,
        SymbolKind::Placeholder => RefKind::Placeholder,
    }
}
