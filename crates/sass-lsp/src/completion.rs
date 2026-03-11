use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, MarkupContent, MarkupKind, Position,
};

use crate::symbols;

// ── Completion context detection ─────────────────────────────────

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
    /// After `:` in a declaration — property value context (variables + functions)
    PropertyValue,
    /// Default — show all symbols
    General,
}

pub(crate) fn detect_completion_context(text: &str, position: Position) -> CompletionContext {
    let line_idx = position.line as usize;
    let Some(line) = text.lines().nth(line_idx) else {
        return CompletionContext::General;
    };

    // Get text before cursor on this line (position.character is UTF-16 offset)
    let target_utf16 = position.character as usize;
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

    // @include name — mixin completion
    if let Some(rest) = trimmed.strip_prefix("@include") {
        let partial = rest.trim_start();
        // Only if we haven't started the argument list
        if !partial.contains('(') {
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
        if !ns.is_empty() {
            return CompletionContext::Namespace(ns);
        }
    }

    // After `$` — variable completion
    if before.ends_with('$') || before.contains('$') && !before.ends_with(' ') {
        if let Some(dollar_pos) = before.rfind('$') {
            let after_dollar = &before[dollar_pos + 1..];
            if after_dollar
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
                return CompletionContext::Variable;
            }
        }
    }

    // Property value position: after `property:` (with possible whitespace)
    if trimmed.contains(':') && !trimmed.starts_with('@') && !trimmed.starts_with('$') {
        return CompletionContext::PropertyValue;
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
        tower_lsp_server::ls_types::Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc.clone(),
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
        if bi.find(|&&c| c == qch).is_none() {
            return false;
        }
    }
    true
}
