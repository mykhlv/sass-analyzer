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
    /// Index in [`Input::all_trivia`] of the WHITESPACE token.
    trivia: u32,
    /// Brackets open at the break; see [`LineBreak::depth`].
    depth: u16,
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
            trivia: 0,
            depth: 0,
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

    /// Index in [`Input::all_trivia`] of the whitespace this break is in.
    #[inline]
    pub fn trivia(self) -> u32 {
        self.trivia
    }

    /// Number of brackets — `(`, `[`, `#{`, and `{` inside one of those — open at this
    /// break. Inside a bracket a line break is plain whitespace, not layout.
    #[inline]
    pub fn depth(self) -> u16 {
        self.depth
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
        Self::lex::<false>(source, lexer::Lexer::new(source))
    }

    /// Lex indented-Sass source and build `Input` with its layout facts.
    ///
    /// The tokens are those of [`Input::from_source`] except where a comment reaches over
    /// the lines indented beneath it (see [`lexer::IndentedLexer`]). The facts are gathered
    /// in the same pass, and only here: `from_source` compiles without that code.
    pub fn from_source_indented(source: &str) -> Self {
        Self::lex::<true>(source, lexer::IndentedLexer::new(source))
    }

    #[allow(clippy::cast_possible_truncation)]
    fn lex<'src, const LAYOUT: bool>(
        source: &'src str,
        mut lexer: impl lexer::TokenSource<'src>,
    ) -> Self {
        let estimated_tokens = source.len() / 5;
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
        let mut depth = 0u32;

        loop {
            let (kind, text) = lexer.next_token();
            if kind == SyntaxKind::EOF {
                break;
            }
            let len = text.len() as u32;
            let range = TextRange::new(TextSize::from(offset), TextSize::from(offset + len));

            if kind.is_trivia() {
                if LAYOUT
                    && kind == SyntaxKind::WHITESPACE
                    && let Some(mut line_break) = LineBreak::measure(text)
                {
                    line_break.trivia = all_trivia.len() as u32;
                    line_break.depth = u16::try_from(depth).unwrap_or(u16::MAX);
                    line_breaks.push(line_break);
                }
                all_trivia.push((kind, range));
            } else {
                trivia_starts.push(pending_trivia_start);
                pending_trivia_start = all_trivia.len() as u32;
                if LAYOUT {
                    line_break_starts.push(pending_line_break_start);
                    pending_line_break_start = line_breaks.len() as u32;
                    depth = bracket_depth_after(depth, kind);
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

    /// Lex again from line break `from` on, as on lines that continue the statement
    /// above it: a comment that starts such a line is an ordinary comment, not one that
    /// reaches over the lines indented beneath it. That holds up to the next significant
    /// token, and past a blank line only if `across_blank`. Only the parser knows which
    /// lines continue a statement, so [`Input::from_source_indented`] reads every comment
    /// that starts a line as starting a statement.
    ///
    /// Returns whether any token changed. The tokens before the line break never do.
    pub fn relex_continued(&mut self, source: &str, from: u32, across_blank: bool) -> bool {
        let line_break = self.line_breaks[from as usize];
        if line_break.depth != 0 {
            // Inside brackets no comment starts a statement in the first place.
            return false;
        }
        let start = self.all_trivia[line_break.trivia as usize].1.start();
        let Some((segment, resume)) = self.relexed_segment(source, start, across_blank) else {
            return false;
        };
        let old: Vec<(SyntaxKind, TextRange)> = self.tokens_in_order().collect();
        let replay = old
            .iter()
            .copied()
            .take_while(|(_, range)| range.start() < start)
            .chain(segment)
            .chain(
                old.iter()
                    .copied()
                    .skip_while(|(_, range)| range.start() < resume),
            )
            .map(|(kind, range)| (kind, &source[range]));
        *self = Self::lex::<true>(source, Replay(replay));
        true
    }

    /// The tokens from `start` up to the first point where lexing as continued lines
    /// meets the existing tokens again, and that point; `None` if they are the same.
    fn relexed_segment(
        &self,
        source: &str,
        start: TextSize,
        across_blank: bool,
    ) -> Option<(Vec<(SyntaxKind, TextRange)>, TextSize)> {
        let mut lexer = lexer::IndentedLexer::resume(source, usize::from(start));
        let mut continued = true;
        let mut segment: Vec<(SyntaxKind, TextRange)> = Vec::new();
        let mut depth = 0u32;
        let mut offset = start;
        loop {
            let (kind, text) = lexer.next_token_where(!continued);
            if kind == SyntaxKind::EOF {
                break;
            }
            #[allow(clippy::cast_possible_truncation)]
            let range = TextRange::at(offset, TextSize::from(text.len() as u32));
            if kind == SyntaxKind::WHITESPACE
                && !continued
                && lexer.is_clean()
                && LineBreak::measure(text).is_some()
                && self.meets_again(&segment, range, depth)
            {
                return self
                    .differs(&segment, start, range.start())
                    .then_some((segment, range.start()));
            }
            if !kind.is_trivia() {
                continued = false;
                depth = bracket_depth_after(depth, kind);
            } else if kind == SyntaxKind::WHITESPACE
                && !across_blank
                && LineBreak::measure(text).is_some_and(LineBreak::is_blank_line)
            {
                continued = false;
            }
            segment.push((kind, range));
            offset = range.end();
        }
        let end = TextSize::of(source);
        self.differs(&segment, start, end).then_some((segment, end))
    }

    /// Whether the existing tokens have the line break `range` too, at bracket depth
    /// `depth`, after the same token as `segment` ends with: lexing from there on goes as
    /// it went before.
    fn meets_again(
        &self,
        segment: &[(SyntaxKind, TextRange)],
        range: TextRange,
        depth: u32,
    ) -> bool {
        let Some(&(prev_kind, prev_range)) = segment.last() else {
            return false;
        };
        if prev_kind == SyntaxKind::SINGLE_LINE_COMMENT {
            return false;
        }
        let index = self
            .all_trivia
            .partition_point(|(_, r)| r.start() < range.start());
        if self.all_trivia.get(index) != Some(&(SyntaxKind::WHITESPACE, range)) {
            return false;
        }
        let Ok(trivia) = u32::try_from(index) else {
            return false;
        };
        let Ok(line_break) = self.line_breaks.binary_search_by_key(&trivia, |b| b.trivia) else {
            return false;
        };
        if u32::from(self.line_breaks[line_break].depth) != depth {
            return false;
        }
        // The token before it, trivia or not, is the same one.
        let owner = self.trivia_starts.partition_point(|&start| start <= trivia) - 1;
        let prev = if (self.trivia_starts[owner] as usize) < index {
            Some(self.all_trivia[index - 1])
        } else {
            owner
                .checked_sub(1)
                .map(|token| (self.kinds[token], self.ranges[token]))
        };
        prev == Some((prev_kind, prev_range))
    }

    /// Whether `segment` differs from the existing tokens between `start` and `end`.
    fn differs(&self, segment: &[(SyntaxKind, TextRange)], start: TextSize, end: TextSize) -> bool {
        !self
            .tokens_in_order()
            .skip_while(|(_, r)| r.start() < start)
            .take_while(|(_, r)| r.start() < end)
            .eq(segment.iter().copied())
    }

    /// Every token, trivia included, in source order.
    fn tokens_in_order(&self) -> impl Iterator<Item = (SyntaxKind, TextRange)> + '_ {
        (0..=self.len()).flat_map(move |pos| {
            let trivia = if pos < self.len() {
                self.trivia_before(pos)
            } else {
                self.trailing_trivia()
            };
            let token = (pos < self.len()).then(|| (self.kinds[pos], self.ranges[pos]));
            trivia.iter().copied().chain(token)
        })
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
        self.has_whitespace_in(start, end)
    }

    /// Whether any of the trivia tokens `start..end` of [`Input::all_trivia`] is whitespace.
    #[inline]
    pub fn has_whitespace_in(&self, start: usize, end: usize) -> bool {
        self.all_trivia[start..end]
            .iter()
            .any(|(kind, _)| *kind == SyntaxKind::WHITESPACE)
    }

    /// End (exclusive) in [`Input::all_trivia`] of the trivia before the token at `pos`;
    /// `pos == len()` gives the end of the trailing trivia.
    #[inline]
    pub fn trivia_end_index(&self, pos: usize) -> usize {
        if pos < self.len() {
            self.trivia_starts[pos + 1] as usize
        } else {
            self.all_trivia.len()
        }
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

    /// The line breaks before the token at `pos` as indices into the whole sequence of
    /// line breaks, for [`Input::line_break`]; `pos == len()` gives those before EOF.
    #[inline]
    pub fn line_break_range(&self, pos: usize) -> (u32, u32) {
        (self.line_break_starts[pos], self.line_break_starts[pos + 1])
    }

    /// The `index`th line break of the input.
    #[inline]
    pub fn line_break(&self, index: u32) -> LineBreak {
        self.line_breaks[index as usize]
    }

    /// Number of line breaks in the input.
    #[inline]
    pub fn line_break_count(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation)]
        let count = self.line_breaks.len() as u32;
        count
    }

    /// The token (or `len()` for EOF) that the `index`th line break comes before.
    pub fn line_break_owner(&self, index: u32) -> usize {
        // `line_break_starts` is non-decreasing; the owner is the last token whose
        // breaks start at or before `index`.
        self.line_break_starts
            .partition_point(|&start| start <= index)
            - 1
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

/// Tokens lexed before, fed back through [`Input::lex`].
struct Replay<I>(I);

impl<'src, I: Iterator<Item = (SyntaxKind, &'src str)>> lexer::TokenSource<'src> for Replay<I> {
    fn next_token(&mut self) -> (SyntaxKind, &'src str) {
        self.0.next().unwrap_or((SyntaxKind::EOF, ""))
    }
}

/// Bracket depth after a significant token, counted as the indented lexer counts it for
/// comment extent: `(`, `[` and `#{` open, `{` opens only inside another bracket (a map
/// or an interpolation's braces), and a closer never takes the depth below zero.
#[inline]
pub(crate) fn bracket_depth_after(depth: u32, kind: SyntaxKind) -> u32 {
    match kind {
        SyntaxKind::LPAREN | SyntaxKind::LBRACKET | SyntaxKind::HASH_LBRACE => depth + 1,
        SyntaxKind::LBRACE if depth > 0 => depth + 1,
        SyntaxKind::RPAREN | SyntaxKind::RBRACKET | SyntaxKind::RBRACE => depth.saturating_sub(1),
        _ => depth,
    }
}
