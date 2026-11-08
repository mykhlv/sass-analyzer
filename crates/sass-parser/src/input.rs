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
    /// One forward pass: the indentation restarts at every newline byte, and each line
    /// break counts once towards a blank line: `\r\n`, as a lone `\n`, `\r` or `\x0c`.
    pub fn measure(whitespace: &str) -> Option<Self> {
        let mut line_breaks = 0u32;
        let mut has_newline = false;
        let mut column = 0u32;
        let mut indent = 0u8;
        let mut indent_ended = false;
        let mut after_cr = false;
        for b in whitespace.bytes() {
            let crlf = after_cr && b == b'\n';
            after_cr = b == b'\r';
            match b {
                b'\n' | b'\r' | b'\x0c' => {
                    line_breaks += u32::from(!crlf);
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
        let blank = if line_breaks >= 2 {
            Self::BLANK_LINE
        } else {
            0
        };
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

    /// The break spans at least one blank line (two or more line breaks).
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
    /// the lines indented beneath it (see [`lexer::IndentedLexer::crossing_blank_lines`]),
    /// and that a loud comment never closed is trivia, not a significant `ERROR`.
    /// The facts are gathered in the same pass, and only here: `from_source` compiles
    /// without that code.
    pub fn from_source_indented(source: &str) -> Self {
        Self::lex::<true>(source, lexer::IndentedLexer::crossing_blank_lines(source))
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
            // Nothing follows such a comment, not even trivia: it becomes the trailing
            // trivia's last token, and the breaks before it the end of input's.
            if let (Some(&kind), Some(&range)) = (kinds.last(), ranges.last())
                && is_unterminated_comment(
                    kind,
                    &source[range],
                    range.end() == TextSize::of(source),
                )
            {
                kinds.pop();
                ranges.pop();
                trivia_starts.pop();
                line_break_starts.pop();
                all_trivia.push((kind, range));
            }
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
    /// Lexing stops where it meets the existing tokens again, and only the tokens in
    /// between are lexed, compared and replaced: that work is bounded by the segment, not
    /// by the input. What follows the segment moves over in the arrays, with the indices
    /// it holds shifted by the difference, but is not looked at otherwise. The tokens
    /// before the line break never change, nor do their indices.
    ///
    /// Returns whether any token changed.
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
        #[cfg(debug_assertions)]
        let expected =
            (self.len() < 2048).then(|| self.relexed_in_full(source, start, resume, &segment));
        self.splice(source, from, resume, &segment);
        #[cfg(debug_assertions)]
        if let Some(expected) = expected {
            self.debug_assert_same(&expected);
        }
        true
    }

    /// Replace the tokens from line break `from` up to `resume` with `segment`, which
    /// covers the same text, and shift the indices that the tokens after it hold.
    #[allow(clippy::cast_possible_truncation)]
    fn splice(
        &mut self,
        source: &str,
        from: u32,
        resume: TextSize,
        segment: &[(SyntaxKind, TextRange)],
    ) {
        let t0 = self.line_breaks[from as usize].trivia as usize;
        let start = self.all_trivia[t0].1.start();
        let k0 = self.ranges.partition_point(|r| r.start() < start);
        let k1 = k0 + self.ranges[k0..].partition_point(|r| r.start() < resume);
        let t1 = t0 + self.all_trivia[t0..].partition_point(|(_, r)| r.start() < resume);
        let b0 = from as usize;
        let b1 = b0 + self.line_breaks[b0..].partition_point(|b| (b.trivia as usize) < t1);

        // The loop of `lex`, from its state at `start`; the line break there is at depth 0.
        let mut kinds = Vec::new();
        let mut ranges = Vec::new();
        let mut trivia = Vec::new();
        let mut trivia_starts = Vec::new();
        let mut line_breaks = Vec::new();
        let mut line_break_starts = Vec::new();
        let mut pending_trivia_start = self.trivia_starts[k0];
        let mut pending_line_break_start = self.line_break_starts[k0];
        let mut depth = 0u32;
        for &(kind, range) in segment {
            let ends_input = range.end() == TextSize::of(source);
            if kind.is_trivia() || is_unterminated_comment(kind, &source[range], ends_input) {
                if kind == SyntaxKind::WHITESPACE
                    && let Some(mut line_break) = LineBreak::measure(&source[range])
                {
                    line_break.trivia = (t0 + trivia.len()) as u32;
                    line_break.depth = u16::try_from(depth).unwrap_or(u16::MAX);
                    line_breaks.push(line_break);
                }
                trivia.push((kind, range));
            } else {
                trivia_starts.push(pending_trivia_start);
                pending_trivia_start = (t0 + trivia.len()) as u32;
                line_break_starts.push(pending_line_break_start);
                pending_line_break_start = (b0 + line_breaks.len()) as u32;
                depth = bracket_depth_after(depth, kind);
                kinds.push(kind);
                ranges.push(range);
            }
        }
        // The first token after the segment: where its trivia and line breaks start.
        trivia_starts.push(pending_trivia_start);
        line_break_starts.push(pending_line_break_start);

        let added_trivia = trivia.len().wrapping_sub(t1 - t0) as u32;
        let added_line_breaks = line_breaks.len().wrapping_sub(b1 - b0) as u32;
        let new_line_breaks = line_breaks.len();

        self.kinds.splice(k0..k1, kinds);
        self.ranges.splice(k0..k1, ranges);
        let tail = k0 + trivia_starts.len();
        self.trivia_starts.splice(k0..=k1, trivia_starts);
        for start in &mut self.trivia_starts[tail..] {
            *start = start.wrapping_add(added_trivia);
        }
        self.line_break_starts.splice(k0..=k1, line_break_starts);
        for start in &mut self.line_break_starts[tail..] {
            *start = start.wrapping_add(added_line_breaks);
        }
        self.all_trivia.splice(t0..t1, trivia);
        self.line_breaks.splice(b0..b1, line_breaks);
        for line_break in &mut self.line_breaks[b0 + new_line_breaks..] {
            line_break.trivia = line_break.trivia.wrapping_add(added_trivia);
        }
    }

    /// What [`Input::splice`] must produce: the whole input lexed again with `segment` in
    /// place of the tokens from `start` up to `resume`.
    #[cfg(debug_assertions)]
    fn relexed_in_full(
        &self,
        source: &str,
        start: TextSize,
        resume: TextSize,
        segment: &[(SyntaxKind, TextRange)],
    ) -> Self {
        #[cfg(test)]
        let work = RELEX_WORK.get();
        let old: Vec<(SyntaxKind, TextRange)> = self.tokens_from(TextSize::from(0)).collect();
        #[cfg(test)]
        RELEX_WORK.set(work);
        let replay = old
            .iter()
            .copied()
            .take_while(|(_, range)| range.start() < start)
            .chain(segment.iter().copied())
            .chain(
                old.iter()
                    .copied()
                    .skip_while(|(_, range)| range.start() < resume),
            )
            .map(|(kind, range)| (kind, &source[range]));
        Self::lex::<true>(source, Replay(replay))
    }

    #[cfg(debug_assertions)]
    fn debug_assert_same(&self, expected: &Self) {
        debug_assert!(self.kinds == expected.kinds, "spliced kinds");
        debug_assert!(self.ranges == expected.ranges, "spliced ranges");
        debug_assert!(self.all_trivia == expected.all_trivia, "spliced trivia");
        debug_assert!(
            self.trivia_starts == expected.trivia_starts,
            "spliced trivia starts"
        );
        debug_assert!(
            self.line_breaks == expected.line_breaks,
            "spliced line breaks"
        );
        debug_assert!(
            self.line_break_starts == expected.line_break_starts,
            "spliced line break starts",
        );
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
            count_relex_work();
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
            .tokens_from(start)
            .take_while(|(_, r)| r.start() < end)
            .eq(segment.iter().copied())
    }

    /// Every token, trivia included, in source order from the first that starts at or
    /// after `offset`.
    fn tokens_from(&self, offset: TextSize) -> impl Iterator<Item = (SyntaxKind, TextRange)> {
        let mut token = self.ranges.partition_point(|r| r.start() < offset);
        let mut trivia = self.all_trivia.partition_point(|(_, r)| r.start() < offset);
        std::iter::from_fn(move || {
            count_relex_work();
            let next_trivia = self.all_trivia.get(trivia).copied();
            let next_token = self.ranges.get(token).map(|&r| (self.kinds[token], r));
            let next = match (next_trivia, next_token) {
                (Some(t), Some(k)) if t.1.start() < k.1.start() => {
                    trivia += 1;
                    t
                }
                (_, Some(k)) => {
                    token += 1;
                    k
                }
                (Some(t), None) => {
                    trivia += 1;
                    t
                }
                (None, None) => return None,
            };
            Some(next)
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
#[cfg(debug_assertions)]
struct Replay<I>(I);

#[cfg(debug_assertions)]
impl<'src, I: Iterator<Item = (SyntaxKind, &'src str)>> lexer::TokenSource<'src> for Replay<I> {
    fn next_token(&mut self) -> (SyntaxKind, &'src str) {
        self.0.next().unwrap_or((SyntaxKind::EOF, ""))
    }
}

#[cfg(test)]
thread_local! {
    /// Tokens lexed or visited by [`Input::relex_continued`] on this thread.
    pub(crate) static RELEX_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[inline]
fn count_relex_work() {
    #[cfg(test)]
    RELEX_WORK.set(RELEX_WORK.get() + 1);
}

/// A loud comment that is never closed: it runs to the end of the input, as an `ERROR`
/// token. Indented input keeps it as trivia — the last of all — so that the statement it
/// follows ends there, as at the end of the input, rather than at an unexpected token;
/// the parser reports it once, as unterminated.
fn is_unterminated_comment(kind: SyntaxKind, text: &str, ends_input: bool) -> bool {
    kind == SyntaxKind::ERROR && ends_input && text.starts_with("/*")
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
