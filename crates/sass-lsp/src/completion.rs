use std::sync::Arc;

use dashmap::DashMap;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextSize;
use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, MarkupContent,
    MarkupKind, Uri,
};

use crate::DocumentState;
use crate::convert::lsp_position_to_offset;
use crate::css_properties;
use crate::css_values;
use crate::symbols;
use crate::workspace::ModuleGraph;

// ── Completion context detection ─────────────────────────────────

#[derive(Debug)]
pub(crate) enum CompletionContext {
    /// After `$` — only variables
    Variable,
    /// After `@include ` — only mixins
    IncludeMixin,
    /// After `@extend %` — only placeholders
    Extend,
    /// After `ns.` — only symbols from that namespace
    Namespace(String),
    /// After `@use "` or `@forward "` — module path completions
    UseModulePath(String),
    /// In property-name position (start of line or after `;`/`{`)
    PropertyName(String),
    /// After `:` in a declaration — property value context.
    /// Contains (`property_name`, `partial_value`) for keyword lookup + fuzzy filtering.
    PropertyValue(String, String),
    /// Default — show all symbols
    General,
}

/// Detect completion context from a single line of text and the cursor's UTF-16
/// character offset within that line. Avoids cloning the entire document.
pub(crate) fn detect_completion_context(line: &str, character: u32) -> CompletionContext {
    // Get text before cursor on this line (character is UTF-16 offset)
    let target_utf16 = character as usize;
    let mut utf16_count = 0;
    let mut byte_offset = 0;
    for ch in line.chars() {
        if utf16_count >= target_utf16 {
            break;
        }
        utf16_count += ch.len_utf16();
        byte_offset += ch.len_utf8();
    }
    let before = &line[..byte_offset];
    let trimmed = before.trim_start();

    // @use "path" or @forward "path" → module path completion
    if let Some(rest) = trimmed
        .strip_prefix("@use")
        .or_else(|| trimmed.strip_prefix("@forward"))
    {
        let rest = rest.trim_start();
        if let Some(partial) = rest.strip_prefix('"') {
            return CompletionContext::UseModulePath(partial.to_owned());
        }
        if let Some(partial) = rest.strip_prefix('\'') {
            return CompletionContext::UseModulePath(partial.to_owned());
        }
    }

    // @include name — mixin completion (but check for namespace first)
    if let Some(rest) = trimmed.strip_prefix("@include") {
        let partial = rest.trim_start();
        // Only if we haven't started the argument list
        if !partial.contains('(') {
            // Check for namespace prefix: `@include ns.` → Namespace context
            if let Some(dot_pos) = partial.rfind('.') {
                let ns = &partial[..dot_pos];
                if !ns.is_empty() && ns.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
                    return CompletionContext::Namespace(ns.to_owned());
                }
            }
            return CompletionContext::IncludeMixin;
        }
    }

    // @extend % — placeholder completion
    if trimmed.starts_with("@extend") {
        return CompletionContext::Extend;
    }

    // namespace.member — after `ns.`
    if let Some(dot_pos) = before.rfind('.') {
        let before_dot = &before[..dot_pos];
        // Extract the namespace identifier (last word before dot)
        let ns: String = before_dot
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        // Must start with a letter or underscore (not a digit — avoids `1.5` false positive)
        if ns.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            return CompletionContext::Namespace(ns);
        }
    }

    // After `$` — variable completion
    if (before.ends_with('$') || (before.contains('$') && !before.ends_with(' ')))
        && let Some(dollar_pos) = before.rfind('$')
    {
        let after_dollar = &before[dollar_pos + 1..];
        if after_dollar
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return CompletionContext::Variable;
        }
    }

    // Property value position: after `property:` (with possible whitespace).
    // Guard against pseudo-selectors (`a:hover`, `&:focus`, `:root`) by requiring
    // the text before `:` to look like a CSS property name — must contain a hyphen
    // or be at least 2 chars long (excludes single-letter tag selectors like `a:hover`).
    if let Some(colon_pos) = trimmed.find(':')
        && !trimmed.starts_with('@')
        && !trimmed.starts_with('$')
        && !trimmed.starts_with('&')
        && !trimmed.starts_with(':')
        && colon_pos > 0
    {
        let prop_candidate = trimmed[..colon_pos].trim();
        let looks_like_property = prop_candidate.len() >= 2
            && prop_candidate
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if looks_like_property {
            let prop = prop_candidate.to_owned();
            let partial = trimmed[colon_pos + 1..].trim_start().to_owned();
            return CompletionContext::PropertyValue(prop, partial);
        }
    }

    // Property name position: line starts with a letter/hyphen (typical for properties)
    // or after `{` or `;`
    if !trimmed.is_empty()
        && !trimmed.starts_with('$')
        && !trimmed.starts_with('@')
        && !trimmed.starts_with('.')
        && !trimmed.starts_with('#')
        && !trimmed.starts_with('&')
        && !trimmed.starts_with('%')
        && !trimmed.starts_with('>')
        && !trimmed.starts_with('+')
        && !trimmed.starts_with('~')
        && !trimmed.starts_with('/')
        && !trimmed.starts_with('*')
        && !trimmed.contains(':')
    {
        // Could be a property name if we're inside a block
        // Simple heuristic: if the line starts with a lowercase letter or hyphen
        if trimmed.starts_with(|c: char| c.is_ascii_lowercase() || c == '-') {
            return CompletionContext::PropertyName(trimmed.to_owned());
        }
    }

    CompletionContext::General
}

/// Refine line-based completion context using the parsed CST. Corrects two cases:
/// 1. Map entries misdetected as `PropertyValue` → `General`
/// 2. Multi-line values misdetected as `PropertyName` → `PropertyValue`
///
/// Returns `Some(corrected)` when the AST overrides line-based, `None` to keep it.
fn ast_refine_context(
    root: &SyntaxNode,
    offset: TextSize,
    line_ctx: &CompletionContext,
) -> Option<CompletionContext> {
    let token = root.token_at_offset(offset).left_biased()?;

    for ancestor in token.parent_ancestors() {
        match ancestor.kind() {
            SyntaxKind::MAP_ENTRY | SyntaxKind::MAP_EXPR => {
                // Line-based detected PropertyValue but we're inside a map — suppress.
                if matches!(line_ctx, CompletionContext::PropertyValue(..)) {
                    return Some(CompletionContext::General);
                }
                return None;
            }
            SyntaxKind::VALUE => {
                // Inside a VALUE node — check if it belongs to a declaration.
                // VALUE nodes can be nested; only act when parent is a declaration.
                let Some(parent) = ancestor.parent() else {
                    continue;
                };
                if !is_declaration_kind(parent.kind()) {
                    continue;
                }
                // Only override if line-based missed it (PropertyName or General).
                if matches!(
                    line_ctx,
                    CompletionContext::PropertyName(_) | CompletionContext::General
                ) {
                    let prop_name = extract_property_name(&parent)?;
                    let partial = partial_value_at_offset(&ancestor, offset);
                    return Some(CompletionContext::PropertyValue(prop_name, partial));
                }
                return None;
            }
            k if is_declaration_kind(k) => {
                // Cursor in declaration after colon but no VALUE node yet (empty value).
                if matches!(
                    line_ctx,
                    CompletionContext::PropertyName(_) | CompletionContext::General
                ) {
                    let colon_end = ancestor.children_with_tokens().find_map(|child| {
                        if child.kind() == SyntaxKind::COLON {
                            Some(child.text_range().end())
                        } else {
                            None
                        }
                    })?;
                    if offset >= colon_end {
                        let prop_name = extract_property_name(&ancestor)?;
                        return Some(CompletionContext::PropertyValue(prop_name, String::new()));
                    }
                }
                return None;
            }
            SyntaxKind::BLOCK => {
                // Cursor in whitespace after a declaration that has no VALUE
                // (e.g., `display:\n    |`). Find the last declaration before offset
                // that is missing a VALUE child.
                if !matches!(
                    line_ctx,
                    CompletionContext::PropertyName(_) | CompletionContext::General
                ) {
                    continue;
                }
                let prev_decl = ancestor
                    .children()
                    .filter(|c| is_declaration_kind(c.kind()))
                    .filter(|c| c.text_range().end() <= offset)
                    .last();
                if let Some(decl) = prev_decl {
                    let has_value = decl.children().any(|c| c.kind() == SyntaxKind::VALUE);
                    if !has_value {
                        let prop_name = extract_property_name(&decl)?;
                        return Some(CompletionContext::PropertyValue(prop_name, String::new()));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_declaration_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::DECLARATION | SyntaxKind::CUSTOM_PROPERTY_DECL | SyntaxKind::NESTED_PROPERTY
    )
}

fn extract_property_name(decl: &SyntaxNode) -> Option<String> {
    decl.children()
        .find(|c| c.kind() == SyntaxKind::PROPERTY)
        .map(|prop| prop.text().to_string().trim().to_owned())
}

fn partial_value_at_offset(value_node: &SyntaxNode, offset: TextSize) -> String {
    let value_text = value_node.text().to_string();
    let start = value_node.text_range().start();
    // Guard against offset before the node (can happen at token boundaries).
    let byte_offset: usize = offset.checked_sub(start).map_or(0, Into::into);
    let clamped = byte_offset.min(value_text.len());
    // Retreat to a char boundary to avoid panics on multi-byte UTF-8.
    let mut safe = clamped;
    while safe > 0 && !value_text.is_char_boundary(safe) {
        safe -= 1;
    }
    let before = &value_text[..safe];
    // Take the last "word" (non-whitespace run) as the partial
    before
        .rsplit_once(char::is_whitespace)
        .map_or(before, |(_, w)| w)
        .to_owned()
}

pub(crate) fn symbol_to_completion_item(
    prefix: Option<&str>,
    sym: &symbols::Symbol,
    is_builtin: bool,
) -> CompletionItem {
    let (label, insert_text, kind, detail) = match sym.kind {
        symbols::SymbolKind::Variable => {
            let label = if let Some(ns) = prefix {
                format!("{ns}.${}", sym.name)
            } else {
                format!("${}", sym.name)
            };
            let detail = sym.value.clone();
            (label, None, CompletionItemKind::VARIABLE, detail)
        }
        symbols::SymbolKind::Function => {
            let label = if let Some(ns) = prefix {
                format!("{ns}.{}", sym.name)
            } else {
                sym.name.clone()
            };
            let detail = sym.params.clone();
            (label, None, CompletionItemKind::FUNCTION, detail)
        }
        symbols::SymbolKind::Mixin => {
            let label = if let Some(ns) = prefix {
                format!("{ns}.{}", sym.name)
            } else {
                sym.name.clone()
            };
            let detail = Some(
                sym.params
                    .as_ref()
                    .map_or_else(|| "@mixin".to_owned(), |p| format!("@mixin{p}")),
            );
            (label, None, CompletionItemKind::METHOD, detail)
        }
        symbols::SymbolKind::Placeholder => {
            let label = format!("%{}", sym.name);
            (label, None, CompletionItemKind::CLASS, None)
        }
    };

    // 3-tier sort: 0_ local, 1_ imported, 2_ builtin
    let tier = if is_builtin {
        "2"
    } else if prefix.is_some() {
        "1"
    } else {
        "0"
    };
    let sort_text = Some(format!("{tier}_{label}"));

    let documentation = sym.doc.as_ref().map(|doc| {
        let value = if crate::sassdoc::has_annotations(doc) {
            let parsed = crate::sassdoc::parse(doc);
            crate::sassdoc::format_markdown(&parsed)
        } else {
            doc.clone()
        };
        tower_lsp_server::ls_types::Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        })
    });

    CompletionItem {
        label,
        kind: Some(kind),
        detail,
        insert_text,
        sort_text,
        documentation,
        ..CompletionItem::default()
    }
}

// ── Handler ─────────────────────────────────────────────────────────

pub(crate) async fn handle(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &Arc<ModuleGraph>,
    params: CompletionParams,
) -> tower_lsp_server::jsonrpc::Result<Option<CompletionResponse>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let (cursor_line, green, text, line_index) = {
        let Some(doc) = documents.get(&uri) else {
            return Ok(None);
        };
        let line_idx = position.line as usize;
        let line = match doc.text.lines().nth(line_idx) {
            Some(l) => l.to_owned(),
            None => return Ok(None),
        };
        (
            line,
            doc.green.clone(),
            doc.text.clone(),
            doc.line_index.clone(),
        )
    };

    let line_ctx = detect_completion_context(&cursor_line, position.character);

    // Use AST to correct line-based detection for two cases:
    // 1. Map entries misdetected as PropertyValue → General
    // 2. Multi-line values misdetected as PropertyName/General → PropertyValue
    let ctx = lsp_position_to_offset(&text, &line_index, position)
        .and_then(|offset| {
            let root = SyntaxNode::new_root(green);
            ast_refine_context(&root, offset, &line_ctx)
        })
        .unwrap_or(line_ctx);

    match ctx {
        CompletionContext::UseModulePath(partial) => {
            let graph = Arc::clone(module_graph);
            let uri_clone = uri.clone();
            let items =
                tokio::task::spawn_blocking(move || graph.complete_use_paths(&uri_clone, &partial))
                    .await
                    .unwrap_or_default();
            if items.is_empty() {
                return Ok(None);
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }
        CompletionContext::PropertyName(partial) => {
            let mut scored: Vec<(u32, &str)> = css_properties::CSS_PROPERTIES
                .iter()
                .filter_map(|p| {
                    let score = fuzzy_score(p, &partial)?;
                    Some((score, *p))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            let items: Vec<CompletionItem> = scored
                .into_iter()
                .map(|(score, p)| CompletionItem {
                    label: p.to_owned(),
                    kind: Some(CompletionItemKind::PROPERTY),
                    sort_text: Some(format!("0_{:04}_{p}", 1000 - score)),
                    ..CompletionItem::default()
                })
                .collect();
            if items.is_empty() {
                return Ok(None);
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }
        CompletionContext::PropertyValue(ref prop_name, ref partial) => {
            let prop_values = css_values::values_for_property(prop_name);
            let all_values: Vec<&str> = prop_values
                .iter()
                .chain(css_values::GLOBAL_KEYWORDS.iter())
                .copied()
                .collect();

            // Note: if partial starts with `$`, the Variable context is detected
            // before PropertyValue (line ~101), so we never reach here with a `$` prefix.
            let mut items: Vec<CompletionItem> = if partial.is_empty() {
                // No partial typed — show all values unfiltered
                all_values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| CompletionItem {
                        label: (*v).to_owned(),
                        kind: Some(CompletionItemKind::ENUM_MEMBER),
                        sort_text: Some(format!("0_{i:04}_{v}")),
                        ..CompletionItem::default()
                    })
                    .collect()
            } else {
                // Fuzzy-filter values against partial
                let mut scored: Vec<(u32, &str)> = all_values
                    .iter()
                    .filter_map(|v| {
                        let score = fuzzy_score(v, partial)?;
                        Some((score, *v))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                scored
                    .into_iter()
                    .map(|(score, v)| CompletionItem {
                        label: v.to_owned(),
                        kind: Some(CompletionItemKind::ENUM_MEMBER),
                        sort_text: Some(format!("0_{:04}_{v}", 1000 - score)),
                        ..CompletionItem::default()
                    })
                    .collect()
            };

            // Also include Sass symbols (variables, functions) — valid in value position
            let visible = module_graph.visible_symbols(&uri);
            let symbol_items: Vec<CompletionItem> = visible
                .into_iter()
                .filter(|(_, _, sym)| {
                    sym.kind == symbols::SymbolKind::Variable
                        || sym.kind == symbols::SymbolKind::Function
                })
                .map(|(prefix, sym_uri, sym)| {
                    let is_builtin = crate::builtins::is_builtin_uri(sym_uri.as_str());
                    let mut item = symbol_to_completion_item(prefix.as_deref(), &sym, is_builtin);
                    // Bump symbol sort after keyword values (replace tier prefix)
                    if let Some(ref mut st) = item.sort_text {
                        let suffix = st.find('_').map_or(st.as_str(), |i| &st[i..]);
                        *st = format!("1{suffix}");
                    }
                    item
                })
                .collect();
            items.extend(symbol_items);

            if items.is_empty() {
                return Ok(None);
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }
        _ => {}
    }

    let visible = module_graph.visible_symbols(&uri);
    if visible.is_empty() {
        return Ok(None);
    }

    let items: Vec<CompletionItem> = visible
        .into_iter()
        .filter(|(prefix, _, sym)| match &ctx {
            CompletionContext::Variable => sym.kind == symbols::SymbolKind::Variable,
            CompletionContext::IncludeMixin => sym.kind == symbols::SymbolKind::Mixin,
            CompletionContext::Namespace(ns) => prefix.as_ref().is_some_and(|p| p == ns),
            CompletionContext::Extend => sym.kind == symbols::SymbolKind::Placeholder,
            CompletionContext::General => true,
            // These contexts are handled above and return early.
            CompletionContext::PropertyValue(..)
            | CompletionContext::PropertyName(_)
            | CompletionContext::UseModulePath(_) => false,
        })
        .map(|(prefix, sym_uri, sym)| {
            let is_builtin = crate::builtins::is_builtin_uri(sym_uri.as_str());
            symbol_to_completion_item(prefix.as_deref(), &sym, is_builtin)
        })
        .collect();

    if items.is_empty() {
        return Ok(None);
    }
    Ok(Some(CompletionResponse::Array(items)))
}

// ── Workspace symbol search ─────────────────────────────────────────

/// Fuzzy-score a symbol name against a query. Returns `None` if no match.
/// Higher score = better match. Scoring tiers:
///   - Exact match: 1000
///   - Prefix match: 500 + (100 × coverage ratio)
///   - Word-boundary match: 200 + (100 × coverage ratio)
///   - Subsequence match: 100 × coverage ratio
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn fuzzy_score(name: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let name_lower = name.to_lowercase();
    let query_lower = query.to_lowercase();

    // Exact match.
    if name_lower == query_lower {
        return Some(1000);
    }

    // Prefix match.
    if name_lower.starts_with(&query_lower) {
        let coverage = (query.len() * 100 / name.len()) as u32;
        return Some(500 + coverage);
    }

    // Word-boundary match: query chars align with starts of words (after `-`, `_`, or camelCase).
    if word_boundary_match(&name_lower, name, &query_lower) {
        let coverage = (query.len() * 100 / name.len()) as u32;
        return Some(200 + coverage);
    }

    // Subsequence match.
    let mut name_chars = name_lower.chars();
    for qch in query_lower.chars() {
        name_chars.find(|&c| c == qch)?;
    }
    let coverage = (query.len() * 100 / name.len()) as u32;
    Some(coverage)
}

pub(crate) fn word_boundary_match(
    name_lower: &str,
    name_original: &str,
    query_lower: &str,
) -> bool {
    let boundaries: Vec<char> = std::iter::once(name_lower.chars().next())
        .flatten()
        .chain(
            name_original
                .chars()
                .zip(name_original.chars().skip(1))
                .filter(|&(prev, cur)| {
                    prev == '-' || prev == '_' || (prev.is_lowercase() && cur.is_uppercase())
                })
                .map(|(_, cur)| cur.to_lowercase().next().unwrap_or(cur)),
        )
        .collect();

    let mut bi = boundaries.iter();
    for qch in query_lower.chars() {
        if !bi.any(|&c| c == qch) {
            return false;
        }
    }
    true
}
