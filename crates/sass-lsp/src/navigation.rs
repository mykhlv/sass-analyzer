use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;
use tower_lsp_server::ls_types::{Hover, HoverContents, MarkupContent, MarkupKind, Range, Uri};

use crate::ast_helpers::{
    dollar_ident_name_range, ident_text_range_of, nth_ident_text_range_of, percent_ident_name_range,
};
use crate::builtins;
use crate::convert::text_range_to_lsp;
use crate::symbols;

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

// ── Hover ───────────────────────────────────────────────────────────

pub(crate) fn find_definition_at_offset(
    symbols: &symbols::FileSymbols,
    offset: sass_parser::text_range::TextSize,
) -> Option<&symbols::Symbol> {
    symbols
        .definitions
        .iter()
        .find(|s| s.selection_range.contains(offset))
}

pub(crate) fn make_hover(
    sym: &symbols::Symbol,
    source_uri: Option<&Uri>,
    range: Option<Range>,
) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format_hover_markdown(sym, source_uri),
        }),
        range,
    }
}

pub(crate) fn format_hover_markdown(sym: &symbols::Symbol, source_uri: Option<&Uri>) -> String {
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
        parts.push(doc.clone());
    }

    if let Some(uri) = source_uri {
        if let Some(module) = builtins::builtin_name_from_uri(uri.as_str()) {
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
