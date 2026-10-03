use crate::lexer;
use crate::syntax_kind::SyntaxKind;
use crate::text_range::{TextRange, TextSize};

/// Pre-processed lexer output: significant tokens separated from trivia.
///
/// Trivia is stored flat for allocation efficiency:
/// - `all_trivia`: single array of all trivia tokens in file order
/// - `trivia_starts[i]..trivia_starts[i+1]`: slice of trivia before token `i`
/// - `trivia_starts` has `n_tokens + 1` entries; the last entry (sentinel) marks
///   where trailing trivia begins in `all_trivia`
///
/// Indented input additionally carries layout facts (see [`LineBreak`]):
/// - `line_breaks`: one entry per newline-bearing WHITESPACE trivia, in file order
/// - `line_break_starts[i]..line_break_starts[i+1]`: the line breaks before token `i`;
///   `n_tokens + 2` entries, so that EOF (`i == n_tokens`) has a slice of its own.
///   Both are empty for braced (SCSS) input, which never pays for them.
pub struct Input {
    kinds: Vec<SyntaxKind>,
    ranges: Vec<TextRange>,
    all_trivia: Vec<(SyntaxKind, TextRange)>,
    trivia_starts: Vec<u32>,
    line_breaks: Vec<LineBreak>,
    line_break_starts: Vec<u32>,
}

/// How the indentation after a line break is made up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentKind {
    /// The line starts at column 0.
    None,
    Spaces,
    Tabs,
    /// Both spaces and tabs, in any order.
    Mixed,
}

/// Layout facts about one line break: a WHITESPACE trivia token containing `\n`, `\r` or
/// `\x0c`. Newlines inside comment tokens are not line breaks.
///
/// The column is exactly what the legacy indented lexer measures at each newline it
/// processes, which the layout decisions built on these facts must agree with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineBreak {
    column: u32,
    flags: u8,
}

impl LineBreak {
    const BLANK_LINE: u8 = 1 << 0;
    const SPACES: u8 = 1 << 1;
    const TABS: u8 = 1 << 2;

    /// Measure one WHITESPACE token, or `None` if it holds no newline.
    ///
    /// One forward pass: the indentation restarts at every newline byte, and only `\n`
    /// counts towards a blank line, so that a CRLF line break is one line, not two.
    pub fn measure(whitespace: &str) -> Option<Self> {
        let mut line_feeds = 0u32;
        let mut has_newline = false;
        let mut column = 0u32;
        let mut indent = 0u8;
        let mut indent_ended = false;
        for b in whitespace.bytes() {
            match b {
                b'\n' | b'\r' | b'\x0c' => {
                    line_feeds += u32::from(b == b'\n');
                    has_newline = true;
                    column = 0;
                    indent = 0;
                    indent_ended = false;
                }
                _ if indent_ended => {}
                b' ' => {
                    column += 1;
                    indent |= Self::SPACES;
                }
                b'\t' => {
                    column += 1;
                    indent |= Self::TABS;
                }
                _ => indent_ended = true,
            }
        }
        if !has_newline {
            return None;
        }
        let blank = if line_feeds >= 2 { Self::BLANK_LINE } else { 0 };
        Some(Self {
            column,
            flags: indent | blank,
        })
    }

    /// Indentation of the line that follows: the space and tab bytes after the last
    /// newline, each tab counting as one column, as in Dart Sass.
    #[inline]
    pub fn column(self) -> u32 {
        self.column
    }

    /// The break spans at least one blank line (two or more `\n`).
    #[inline]
    pub fn is_blank_line(self) -> bool {
        self.flags & Self::BLANK_LINE != 0
    }

    #[inline]
    pub fn indent_kind(self) -> IndentKind {
        match (self.flags & Self::SPACES != 0, self.flags & Self::TABS != 0) {
            (false, false) => IndentKind::None,
            (true, false) => IndentKind::Spaces,
            (false, true) => IndentKind::Tabs,
            (true, true) => IndentKind::Mixed,
        }
    }
}

impl Input {
    pub fn new(
        kinds: Vec<SyntaxKind>,
        ranges: Vec<TextRange>,
        all_trivia: Vec<(SyntaxKind, TextRange)>,
        trivia_starts: Vec<u32>,
    ) -> Self {
        debug_assert_eq!(kinds.len(), ranges.len());
        debug_assert_eq!(trivia_starts.len(), kinds.len() + 1);
        #[allow(clippy::cast_possible_truncation)]
        {
            debug_assert!(
                *trivia_starts.last().unwrap_or(&0) <= all_trivia.len() as u32,
                "sentinel must not exceed all_trivia length",
            );
        }
        Self {
            kinds,
            ranges,
            all_trivia,
            trivia_starts,
            line_breaks: Vec::new(),
            line_break_starts: Vec::new(),
        }
    }

    /// Lex source code and build `Input` in one step.
    ///
    /// Lexes directly into the `Input` fields, avoiding the intermediate
    /// `Vec<(SyntaxKind, &str)>` allocation that `tokenize()` + `from_tokens()` would create.
    pub fn from_source(source: &str) -> Self {
        Self::lex::<false>(source)
    }

    /// Lex indented-Sass source and build `Input` with its layout facts.
    ///
    /// The tokens are exactly those of [`Input::from_source`]. The facts are gathered in
    /// the same pass, and only here: `from_source` compiles without that code.
    pub fn from_source_indented(source: &str) -> Self {
        Self::lex::<true>(source)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn lex<const LAYOUT: bool>(source: &str) -> Self {
        let estimated_tokens = source.len() / 5;
        let mut lexer = lexer::Lexer::new(source);
        let mut kinds = Vec::with_capacity(estimated_tokens);
        let mut ranges = Vec::with_capacity(estimated_tokens);
        let mut all_trivia = Vec::with_capacity(estimated_tokens / 2);
        let mut trivia_starts = Vec::with_capacity(estimated_tokens + 1);
        let mut line_breaks = Vec::new();
        let mut line_break_starts = Vec::new();
        if LAYOUT {
            line_breaks.reserve(estimated_tokens / 4);
            line_break_starts.reserve(estimated_tokens + 2);
        }
        let mut offset = 0u32;
        let mut pending_trivia_start = 0u32;
        let mut pending_line_break_start = 0u32;

        loop {
            let (kind, text) = lexer.next_token();
            if kind == SyntaxKind::EOF {
                break;
            }
            let len = text.len() as u32;
            let range = TextRange::new(TextSize::from(offset), TextSize::from(offset + len));

            if kind.is_trivia() {
                all_trivia.push((kind, range));
                if LAYOUT
                    && kind == SyntaxKind::WHITESPACE
                    && let Some(line_break) = LineBreak::measure(text)
                {
                    line_breaks.push(line_break);
                }
            } else {
                trivia_starts.push(pending_trivia_start);
                pending_trivia_start = all_trivia.len() as u32;
                if LAYOUT {
                    line_break_starts.push(pending_line_break_start);
                    pending_line_break_start = line_breaks.len() as u32;
                }
                kinds.push(kind);
                ranges.push(range);
            }

            offset += len;
        }

        trivia_starts.push(pending_trivia_start);
        if LAYOUT {
            line_break_starts.push(pending_line_break_start);
            line_break_starts.push(line_breaks.len() as u32);
        }

        Self {
            kinds,
            ranges,
            all_trivia,
            trivia_starts,
            line_breaks,
            line_break_starts,
        }
    }

    /// Build `Input` from raw lexer tokens.
    ///
    /// Each `(SyntaxKind, &str)` must appear in source order; the text slices
    /// are used only for length (byte offsets are computed cumulatively).
    pub fn from_tokens(tokens: &[(SyntaxKind, &str)]) -> Self {
        let mut kinds = Vec::with_capacity(tokens.len());
        let mut ranges = Vec::with_capacity(tokens.len());
        let mut all_trivia = Vec::new();
        let mut trivia_starts = Vec::with_capacity(tokens.len() + 1);
        let mut offset = 0u32;

        // Pending trivia count before the next significant token.
        let mut pending_trivia_start = 0u32;

        for &(kind, text) in tokens {
            #[allow(clippy::cast_possible_truncation)]
            let len = text.len() as u32;
            let range = TextRange::new(TextSize::from(offset), TextSize::from(offset + len));

            if kind.is_trivia() {
                all_trivia.push((kind, range));
            } else {
                #[allow(clippy::cast_possible_truncation)]
                {
                    trivia_starts.push(pending_trivia_start);
                    pending_trivia_start = all_trivia.len() as u32;
                }
                kinds.push(kind);
                ranges.push(range);
            }

            offset += len;
        }

        // Sentinel: marks start of trailing trivia in all_trivia.
        trivia_starts.push(pending_trivia_start);

        Self {
            kinds,
            ranges,
            all_trivia,
            trivia_starts,
            line_breaks: Vec::new(),
            line_break_starts: Vec::new(),
        }
    }

    #[inline]
    pub fn kind(&self, pos: usize) -> SyntaxKind {
        self.kinds.get(pos).copied().unwrap_or(SyntaxKind::EOF)
    }

    #[inline]
    pub fn range(&self, pos: usize) -> TextRange {
        debug_assert!(
            pos < self.len(),
            "Input::range: pos {pos} >= len {}",
            self.len()
        );
        self.ranges[pos]
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Trivia tokens immediately before significant token at `pos`.
    #[inline]
    pub fn trivia_before(&self, pos: usize) -> &[(SyntaxKind, TextRange)] {
        debug_assert!(
            pos < self.len(),
            "trivia_before: pos {pos} >= len {}",
            self.len()
        );
        let start = self.trivia_starts[pos] as usize;
        let end = self.trivia_starts[pos + 1] as usize;
        &self.all_trivia[start..end]
    }

    /// Trailing trivia after all significant tokens (attaches to `SOURCE_FILE`).
    #[inline]
    pub fn trailing_trivia(&self) -> &[(SyntaxKind, TextRange)] {
        let start = *self.trivia_starts.last().unwrap_or(&0) as usize;
        &self.all_trivia[start..]
    }

    /// Raw trivia start index for the bridge's linear walk.
    #[inline]
    pub fn trivia_start_index(&self, pos: usize) -> usize {
        self.trivia_starts[pos] as usize
    }

    /// The full trivia array, for direct access from the bridge.
    #[inline]
    pub fn all_trivia(&self) -> &[(SyntaxKind, TextRange)] {
        &self.all_trivia
    }

    /// Whether any whitespace trivia exists before the token at `pos`.
    #[inline]
    pub fn has_whitespace_before(&self, pos: usize) -> bool {
        debug_assert!(
            pos < self.len(),
            "has_whitespace_before: pos {pos} >= len {}",
            self.len()
        );
        let start = self.trivia_starts[pos] as usize;
        let end = self.trivia_starts[pos + 1] as usize;
        self.all_trivia[start..end]
            .iter()
            .any(|(kind, _)| *kind == SyntaxKind::WHITESPACE)
    }

    /// Line breaks in the trivia before the token at `pos`, in order; `pos == len()`
    /// gives those in the trailing trivia, before EOF. A comment-only line in between
    /// contributes a break of its own, and its column bears on layout as much as the
    /// last one's: a comment line indented less than the block closes it.
    ///
    /// Only for input built by [`Input::from_source_indented`].
    #[inline]
    pub fn line_breaks_before(&self, pos: usize) -> &[LineBreak] {
        debug_assert!(
            !self.line_break_starts.is_empty(),
            "line_breaks_before: layout facts exist only for indented input",
        );
        debug_assert!(
            pos <= self.len(),
            "line_breaks_before: pos {pos} > len {}",
            self.len()
        );
        let start = self.line_break_starts[pos] as usize;
        let end = self.line_break_starts[pos + 1] as usize;
        &self.line_breaks[start..end]
    }

    /// The line break that puts the token at `pos` at the start of its line: the last
    /// one before it. A token with no newline-bearing whitespace before it, the first
    /// token of the file included, does not start a line.
    #[inline]
    pub fn line_start(&self, pos: usize) -> Option<LineBreak> {
        self.line_breaks_before(pos).last().copied()
    }

    #[inline]
    pub fn starts_line(&self, pos: usize) -> bool {
        !self.line_breaks_before(pos).is_empty()
    }
}
