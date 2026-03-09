use crate::text_range::TextSize;

/// Maps byte offsets to line/column numbers.
///
/// Built once per file, O(log n) lookup via binary search on newline offsets.
/// Line numbers are 1-based, column numbers are 1-based (in bytes).
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line. `newlines[0]` is always 0.
    newlines: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number (byte offset from line start + 1).
    pub col: u32,
}

impl std::fmt::Display for LineCol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl LineIndex {
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(source: &str) -> Self {
        // Source files >4 GiB are not supported; u32 offsets match text-size.
        let mut newlines = vec![0u32];
        for (i, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                newlines.push((i + 1) as u32);
            }
        }
        Self { newlines }
    }

    /// Convert a byte offset to a 1-based line/column.
    #[allow(clippy::cast_possible_truncation)]
    pub fn line_col(&self, offset: TextSize) -> LineCol {
        let offset = u32::from(offset);
        let line_idx = match self.newlines.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let line_start = self.newlines[line_idx];
        LineCol {
            line: (line_idx as u32) + 1,
            col: offset - line_start + 1,
        }
    }

    /// Byte offset of the start of a line (0-based line index).
    pub fn line_start(&self, line_0based: u32) -> Option<u32> {
        self.newlines.get(line_0based as usize).copied()
    }

    /// Total number of lines.
    #[allow(clippy::cast_possible_truncation)]
    pub fn line_count(&self) -> u32 {
        self.newlines.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let idx = LineIndex::new("hello");
        assert_eq!(idx.line_col(TextSize::from(0)), LineCol { line: 1, col: 1 });
        assert_eq!(idx.line_col(TextSize::from(4)), LineCol { line: 1, col: 5 });
    }

    #[test]
    fn two_lines() {
        let idx = LineIndex::new("ab\ncd");
        assert_eq!(idx.line_col(TextSize::from(0)), LineCol { line: 1, col: 1 });
        assert_eq!(idx.line_col(TextSize::from(2)), LineCol { line: 1, col: 3 });
        assert_eq!(idx.line_col(TextSize::from(3)), LineCol { line: 2, col: 1 });
        assert_eq!(idx.line_col(TextSize::from(4)), LineCol { line: 2, col: 2 });
    }

    #[test]
    fn empty_input() {
        let idx = LineIndex::new("");
        assert_eq!(idx.line_col(TextSize::from(0)), LineCol { line: 1, col: 1 });
        assert_eq!(idx.line_count(), 1);
    }

    #[test]
    fn trailing_newline() {
        let idx = LineIndex::new("a\n");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_col(TextSize::from(2)), LineCol { line: 2, col: 1 });
    }

    #[test]
    fn multiple_blank_lines() {
        let idx = LineIndex::new("\n\n\n");
        assert_eq!(idx.line_count(), 4);
        assert_eq!(idx.line_col(TextSize::from(0)), LineCol { line: 1, col: 1 });
        assert_eq!(idx.line_col(TextSize::from(1)), LineCol { line: 2, col: 1 });
        assert_eq!(idx.line_col(TextSize::from(2)), LineCol { line: 3, col: 1 });
        assert_eq!(idx.line_col(TextSize::from(3)), LineCol { line: 4, col: 1 });
    }

    #[test]
    fn crlf_counts_lf_only() {
        let idx = LineIndex::new("a\r\nb");
        assert_eq!(idx.line_col(TextSize::from(0)), LineCol { line: 1, col: 1 });
        // \r is col 2 on line 1, \n is col 3
        assert_eq!(idx.line_col(TextSize::from(3)), LineCol { line: 2, col: 1 });
    }

    #[test]
    fn display_line_col() {
        let lc = LineCol { line: 3, col: 12 };
        assert_eq!(format!("{lc}"), "3:12");
    }
}
