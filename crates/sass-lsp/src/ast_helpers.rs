use sass_parser::syntax::{SyntaxNode, SyntaxToken};
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;

use crate::convert::utf16_len;
use crate::symbols;

/// Check if a reference at the given range has a namespace prefix (e.g. `ns.$var`).
/// Returns the namespace identifier if found.
pub(crate) fn namespace_of_ref(root: &SyntaxNode, ref_range: TextRange) -> Option<String> {
    let token = root.token_at_offset(ref_range.start()).right_biased()?;
    for node in token.parent()?.ancestors() {
        if node.kind() == SyntaxKind::NAMESPACE_REF {
            let ns_ident = node
                .children_with_tokens()
                .filter_map(rowan::NodeOrToken::into_token)
                .find(|t| t.kind() == SyntaxKind::IDENT)?;
            return Some(ns_ident.text().to_string());
        }
        if matches!(
            node.kind(),
            SyntaxKind::DECLARATION
                | SyntaxKind::RULE_SET
                | SyntaxKind::BLOCK
                | SyntaxKind::SOURCE_FILE
        ) {
            break;
        }
    }
    None
}

/// Find the first IDENT token among direct children.
pub(crate) fn first_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::IDENT)
}

/// Find the Nth IDENT token (0-indexed) among direct children.
pub(crate) fn nth_ident_token(node: &SyntaxNode, n: usize) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|t| t.kind() == SyntaxKind::IDENT)
        .nth(n)
}

/// Compute combined range from DOLLAR to the following IDENT in direct children.
/// Returns `(range, utf16_length)`.
pub(crate) fn dollar_ident_range(node: &SyntaxNode) -> Option<(TextRange, u32)> {
    let mut dollar_start = None;
    let mut ident_end = None;
    let mut ident_len_utf16 = 0u32;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::DOLLAR => dollar_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if dollar_start.is_some() => {
                    ident_end = Some(token.text_range().end());
                    // $name → UTF-16 length = 1 (for $) + ident chars
                    ident_len_utf16 = 1 + utf16_len(token.text());
                    break;
                }
                _ => {}
            }
        }
    }
    let start = dollar_start?;
    let end = ident_end?;
    Some((TextRange::new(start, end), ident_len_utf16))
}

/// Extract `$name` → (name, DOLLAR..IDENT range) from direct children.
pub(crate) fn dollar_ident_name_range(node: &SyntaxNode) -> Option<(String, TextRange)> {
    let mut dollar_start = None;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::DOLLAR => dollar_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if dollar_start.is_some() => {
                    let range = TextRange::new(dollar_start.unwrap(), token.text_range().end());
                    return Some((token.text().to_string(), range));
                }
                _ => {}
            }
        }
    }
    None
}

/// Extract first IDENT → (text, range).
pub(crate) fn ident_text_range_of(node: &SyntaxNode) -> Option<(String, TextRange)> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::IDENT)
        .map(|t| (t.text().to_string(), t.text_range()))
}

/// Extract nth IDENT → (text, range).
pub(crate) fn nth_ident_text_range_of(node: &SyntaxNode, n: usize) -> Option<(String, TextRange)> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|t| t.kind() == SyntaxKind::IDENT)
        .nth(n)
        .map(|t| (t.text().to_string(), t.text_range()))
}

/// Extract `%name` → (name, PERCENT..IDENT range) from direct children.
pub(crate) fn percent_ident_name_range(node: &SyntaxNode) -> Option<(String, TextRange)> {
    let mut pct_start = None;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::PERCENT => pct_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if pct_start.is_some() => {
                    let range = TextRange::new(pct_start.unwrap(), token.text_range().end());
                    return Some((token.text().to_string(), range));
                }
                _ => {}
            }
        }
    }
    None
}

/// For variables (`$name`) and placeholders (`%name`), strip the sigil
/// to get just the IDENT range. For functions/mixins, the range is already
/// just the IDENT.
pub(crate) fn name_only_range(kind: symbols::SymbolKind, range: TextRange) -> TextRange {
    match kind {
        symbols::SymbolKind::Variable | symbols::SymbolKind::Placeholder => {
            // Skip 1-byte sigil ($ or %)
            let start = range.start() + sass_parser::text_range::TextSize::from(1u32);
            if start < range.end() {
                TextRange::new(start, range.end())
            } else {
                range
            }
        }
        symbols::SymbolKind::Function | symbols::SymbolKind::Mixin => range,
    }
}
