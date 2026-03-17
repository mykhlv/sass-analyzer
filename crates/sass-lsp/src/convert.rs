use sass_parser::line_index::LineIndex;
use sass_parser::text_range::{TextRange, TextSize};
use tower_lsp_server::ls_types::{Position, Range};

/// Convert a byte offset to a 0-based (line, UTF-16 column) pair.
///
/// If `offset` exceeds the source length (stale parse tree), it is clamped to
/// avoid panicking on out-of-bounds slices.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn byte_to_lsp_pos(
    source: &str,
    line_index: &LineIndex,
    offset: TextSize,
) -> (u32, u32) {
    let clamped = TextSize::from((u32::from(offset) as usize).min(source.len()) as u32);
    let lc = line_index.line_col(clamped);
    let line_0 = lc.line - 1;
    let byte_offset = u32::from(clamped) as usize;
    let col_byte = lc.col as usize - 1;
    let line_start_byte = byte_offset.saturating_sub(col_byte);
    let end = byte_offset.min(source.len());
    let slice = &source[line_start_byte..end];
    let col_utf16 = slice.encode_utf16().count() as u32;
    (line_0, col_utf16)
}

/// UTF-16 length of a string slice.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// Convert a `TextRange` (byte offsets) to an LSP `Range` (line/UTF-16 column).
pub(crate) fn text_range_to_lsp(range: TextRange, line_index: &LineIndex, source: &str) -> Range {
    let start = byte_to_lsp_pos(source, line_index, range.start());
    let end = byte_to_lsp_pos(source, line_index, range.end());
    Range::new(Position::new(start.0, start.1), Position::new(end.0, end.1))
}

/// Convert an LSP Position (0-based line, 0-based UTF-16 col) to a byte offset.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn lsp_position_to_offset(
    source: &str,
    line_index: &LineIndex,
    position: Position,
) -> Option<TextSize> {
    let line_start = line_index.line_start(position.line)? as usize;
    let remaining = &source[line_start..];
    let line_text = remaining.split('\n').next().unwrap_or(remaining);

    let target_utf16 = position.character;
    let mut byte_offset = 0usize;
    let mut utf16_offset = 0u32;

    for ch in line_text.chars() {
        if utf16_offset >= target_utf16 {
            break;
        }
        byte_offset += ch.len_utf8();
        utf16_offset += ch.len_utf16() as u32;
    }

    Some(TextSize::from((line_start + byte_offset) as u32))
}
