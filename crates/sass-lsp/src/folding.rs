use dashmap::DashMap;
use rowan::NodeOrToken;
use sass_parser::syntax::{SyntaxNode, SyntaxToken};
use sass_parser::syntax_kind::SyntaxKind;
use tower_lsp_server::ls_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams, Uri};

use crate::DocumentState;
use crate::convert::byte_to_lsp_pos;

pub(crate) fn handle_folding_range(
    documents: &DashMap<Uri, DocumentState>,
    params: FoldingRangeParams,
) -> Vec<FoldingRange> {
    let uri = params.text_document.uri;
    let Some(doc) = documents.get(&uri) else {
        return Vec::new();
    };
    let root = SyntaxNode::new_root(doc.green.clone());
    let mut ranges = Vec::new();
    collect_folds(&root, &doc.text, &doc.line_index, &mut ranges);
    ranges
}

#[rustfmt::skip]
fn is_foldable_block(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::RULE_SET        | SyntaxKind::MIXIN_RULE      | SyntaxKind::INCLUDE_RULE
      | SyntaxKind::FUNCTION_RULE   | SyntaxKind::IF_RULE         | SyntaxKind::ELSE_CLAUSE
      | SyntaxKind::FOR_RULE        | SyntaxKind::EACH_RULE       | SyntaxKind::WHILE_RULE
      | SyntaxKind::MEDIA_RULE      | SyntaxKind::SUPPORTS_RULE   | SyntaxKind::KEYFRAMES_RULE
      | SyntaxKind::LAYER_RULE      | SyntaxKind::CONTAINER_RULE  | SyntaxKind::SCOPE_RULE
      | SyntaxKind::PROPERTY_RULE   | SyntaxKind::AT_ROOT_RULE    | SyntaxKind::PAGE_RULE
      | SyntaxKind::FONT_FACE_RULE  | SyntaxKind::NESTED_PROPERTY | SyntaxKind::GENERIC_AT_RULE
    )
}

fn collect_folds(
    root: &SyntaxNode,
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
    out: &mut Vec<FoldingRange>,
) {
    let mut region_stack: Vec<u32> = Vec::new();
    let mut consecutive_comments: Vec<(u32, u32)> = Vec::new();

    for element in root.descendants_with_tokens() {
        match element {
            NodeOrToken::Node(node) => {
                if is_foldable_block(node.kind()) {
                    fold_block(&node, source, line_index, out);
                }
            }
            NodeOrToken::Token(token) => match token.kind() {
                SyntaxKind::MULTI_LINE_COMMENT => {
                    flush_consecutive_comments(&mut consecutive_comments, out);
                    fold_multiline_comment(&token, source, line_index, out);
                }
                SyntaxKind::SINGLE_LINE_COMMENT => {
                    let text = token.text().trim();
                    if text.starts_with("// #region") || text.starts_with("//#region") {
                        flush_consecutive_comments(&mut consecutive_comments, out);
                        let (line, _) =
                            byte_to_lsp_pos(source, line_index, token.text_range().start());
                        region_stack.push(line);
                    } else if text.starts_with("// #endregion") || text.starts_with("//#endregion")
                    {
                        flush_consecutive_comments(&mut consecutive_comments, out);
                        if let Some(start_line) = region_stack.pop() {
                            let (end_line, _) =
                                byte_to_lsp_pos(source, line_index, token.text_range().start());
                            if start_line < end_line {
                                out.push(FoldingRange {
                                    start_line,
                                    start_character: None,
                                    end_line,
                                    end_character: None,
                                    kind: Some(FoldingRangeKind::Region),
                                    collapsed_text: None,
                                });
                            }
                        }
                    } else {
                        let (line, _) =
                            byte_to_lsp_pos(source, line_index, token.text_range().start());
                        track_consecutive_comment(line, &mut consecutive_comments);
                    }
                }
                _ => {}
            },
        }
    }
    flush_consecutive_comments(&mut consecutive_comments, out);
}

fn first_non_trivia_offset(node: &SyntaxNode) -> Option<sass_parser::text_range::TextSize> {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(t) if !t.kind().is_trivia() => {
                return Some(t.text_range().start());
            }
            NodeOrToken::Node(n) => {
                return first_non_trivia_offset(&n).or_else(|| Some(n.text_range().start()));
            }
            NodeOrToken::Token(_) => {}
        }
    }
    None
}

fn fold_block(
    node: &SyntaxNode,
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
    out: &mut Vec<FoldingRange>,
) {
    let start_offset = first_non_trivia_offset(node).unwrap_or_else(|| node.text_range().start());
    let (start_line, _) = byte_to_lsp_pos(source, line_index, start_offset);
    let (end_line, _) = byte_to_lsp_pos(source, line_index, node.text_range().end());
    let end_line = end_line.saturating_sub(1);
    if start_line < end_line {
        out.push(FoldingRange {
            start_line,
            start_character: None,
            end_line,
            end_character: None,
            kind: None,
            collapsed_text: None,
        });
    }
}

fn fold_multiline_comment(
    token: &SyntaxToken,
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
    out: &mut Vec<FoldingRange>,
) {
    let range = token.text_range();
    let (start_line, _) = byte_to_lsp_pos(source, line_index, range.start());
    let (end_line, _) = byte_to_lsp_pos(source, line_index, range.end());
    let end_line = end_line.saturating_sub(1);
    if start_line < end_line {
        out.push(FoldingRange {
            start_line,
            start_character: None,
            end_line,
            end_character: None,
            kind: Some(FoldingRangeKind::Comment),
            collapsed_text: None,
        });
    }
}

fn track_consecutive_comment(line: u32, comments: &mut Vec<(u32, u32)>) {
    if let Some(last) = comments.last_mut() {
        if line == last.1 + 1 {
            last.1 = line;
            return;
        }
    }
    comments.push((line, line));
}

fn flush_consecutive_comments(comments: &mut Vec<(u32, u32)>, out: &mut Vec<FoldingRange>) {
    for &(start, end) in &*comments {
        if start < end {
            out.push(FoldingRange {
                start_line: start,
                start_character: None,
                end_line: end,
                end_character: None,
                kind: Some(FoldingRangeKind::Comment),
                collapsed_text: None,
            });
        }
    }
    comments.clear();
}
