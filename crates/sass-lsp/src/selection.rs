use dashmap::DashMap;
use sass_parser::line_index::LineIndex;
use sass_parser::syntax::SyntaxNode;
use sass_parser::text_range::{TextRange, TextSize};
use tower_lsp_server::ls_types::{Position, Range, SelectionRange, SelectionRangeParams, Uri};

use crate::DocumentState;
use crate::convert::{lsp_position_to_offset, text_range_to_lsp};

pub(crate) fn handle_selection_range(
    documents: &DashMap<Uri, DocumentState>,
    params: SelectionRangeParams,
) -> Option<Vec<SelectionRange>> {
    let uri = params.text_document.uri;
    let (green, text, line_index) = {
        let doc = documents.get(&uri)?;
        (doc.green.clone(), doc.text.clone(), doc.line_index.clone())
    };
    let root = SyntaxNode::new_root(green);

    let results: Vec<SelectionRange> = params
        .positions
        .into_iter()
        .map(|pos| {
            lsp_position_to_offset(&text, &line_index, pos)
                .and_then(|offset| build_selection_range(&root, offset, &line_index, &text))
                .unwrap_or_else(|| fallback_selection_range(pos))
        })
        .collect();

    Some(results)
}

fn fallback_selection_range(pos: Position) -> SelectionRange {
    let range = Range::new(pos, pos);
    SelectionRange {
        range,
        parent: None,
    }
}

fn build_selection_range(
    root: &SyntaxNode,
    offset: TextSize,
    line_index: &LineIndex,
    source: &str,
) -> Option<SelectionRange> {
    let token = root.token_at_offset(offset).right_biased()?;

    let mut ranges: Vec<TextRange> = Vec::new();
    ranges.push(token.text_range());

    let mut node = token.parent();
    while let Some(n) = node {
        let range = n.text_range();
        if ranges.last().is_none_or(|last| *last != range) {
            ranges.push(range);
        }
        node = n.parent();
    }

    let mut result: Option<SelectionRange> = None;
    for range in ranges.into_iter().rev() {
        result = Some(SelectionRange {
            range: text_range_to_lsp(range, line_index, source),
            parent: result.map(Box::new),
        });
    }

    result
}
