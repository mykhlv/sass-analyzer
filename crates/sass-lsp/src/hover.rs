use dashmap::DashMap;
use sass_parser::syntax::SyntaxNode;
use tower_lsp_server::ls_types::{
    Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Range, Uri,
};

use crate::DocumentState;
use crate::convert::{lsp_position_to_offset, text_range_to_lsp};
use crate::navigation::{find_definition_at_offset, find_reference_at_offset};
use crate::sassdoc;
use crate::symbols::{self, Symbol};
use crate::workspace::ModuleGraph;

pub(crate) fn handle(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: HoverParams,
) -> Option<Hover> {
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let (green, offset, file_symbols, line_index, doc_text) = {
        let doc = documents.get(&uri)?;
        let offset = lsp_position_to_offset(&doc.text, &doc.line_index, position)?;
        (
            doc.green.clone(),
            offset,
            doc.symbols.clone(),
            doc.line_index.clone(),
            doc.text.clone(),
        )
    };

    let root = SyntaxNode::new_root(green);

    // 1. Try reference at cursor → resolve to definition
    if let Some(ref_info) = find_reference_at_offset(&root, offset) {
        let resolved = module_graph.resolve_reference(
            &uri,
            ref_info.namespace.as_deref(),
            &ref_info.name,
            ref_info.kind,
        );

        if let Some((target_uri, symbol)) = resolved {
            let source = if target_uri == uri {
                None
            } else {
                Some(&target_uri)
            };
            let range = Some(text_range_to_lsp(ref_info.range, &line_index, &doc_text));
            return Some(make_hover(&symbol, source, range));
        }
        return None;
    }

    // 2. Try definition at cursor (hovering on a declaration name)
    if let Some(symbol) = find_definition_at_offset(&file_symbols, offset) {
        let range = Some(text_range_to_lsp(
            symbol.selection_range,
            &line_index,
            &doc_text,
        ));
        return Some(make_hover(symbol, None, range));
    }

    None
}

pub(crate) fn make_hover(sym: &Symbol, source_uri: Option<&Uri>, range: Option<Range>) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format_hover_markdown(sym, source_uri),
        }),
        range,
    }
}

pub(crate) fn format_hover_markdown(sym: &Symbol, source_uri: Option<&Uri>) -> String {
    let signature = match sym.kind {
        symbols::SymbolKind::Variable => {
            if let Some(value) = &sym.value {
                format!("${}: {value}", sym.name)
            } else {
                format!("${}", sym.name)
            }
        }
        symbols::SymbolKind::Function => {
            let params = sym.params.as_deref().unwrap_or("()");
            format!("@function {}{params}", sym.name)
        }
        symbols::SymbolKind::Mixin => {
            let params = sym.params.as_deref().unwrap_or("");
            format!("@mixin {}{params}", sym.name)
        }
        symbols::SymbolKind::Placeholder => format!("%{}", sym.name),
    };

    let mut parts = vec![format!("```scss\n{signature}\n```")];

    if let Some(doc) = &sym.doc {
        if sassdoc::has_annotations(doc) {
            let parsed = sassdoc::parse(doc);
            parts.push(sassdoc::format_markdown(&parsed));
        } else {
            parts.push(doc.clone());
        }
    }

    if let Some(uri) = source_uri {
        if let Some(module) = crate::builtins::builtin_name_from_uri(uri.as_str()) {
            let anchor = match sym.kind {
                symbols::SymbolKind::Variable => format!("%24{}", sym.name),
                _ => sym.name.clone(),
            };
            let url = format!("https://sass-lang.com/documentation/modules/{module}/#{anchor}");
            parts.push(format!("`sass:{module}` · [docs]({url})"));
        } else if let Some(path) = uri.to_file_path() {
            if let Some(name) = path.file_name() {
                parts.push(format!("Defined in `{}`", name.to_string_lossy()));
            }
        }
    }

    parts.join("\n\n")
}
