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
pub struct Input {
    kinds: Vec<SyntaxKind>,
    ranges: Vec<TextRange>,
    all_trivia: Vec<(SyntaxKind, TextRange)>,
    trivia_starts: Vec<u32>,
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
        }
    }

    /// Lex source code and build `Input` in one step.
    pub fn from_source(source: &str) -> Self {
        let tokens = lexer::tokenize(source);
        Self::from_tokens(&tokens)
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
        }
    }

    #[inline]
    pub fn kind(&self, pos: usize) -> SyntaxKind {
        self.kinds.get(pos).copied().unwrap_or(SyntaxKind::EOF)
    }

    /// # Panics
    /// Panics if `pos >= self.len()`.
    #[inline]
    pub fn range(&self, pos: usize) -> TextRange {
        assert!(
            pos < self.len(),
            "Input::range: pos {pos} >= len {}",
            self.len()
        );
        self.ranges[pos]
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Trivia tokens immediately before significant token at `pos`.
    ///
    /// `pos` must be `< self.len()`.
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
    pub fn trailing_trivia(&self) -> &[(SyntaxKind, TextRange)] {
        let start = *self.trivia_starts.last().unwrap_or(&0) as usize;
        &self.all_trivia[start..]
    }

    /// Whether any whitespace trivia exists before the token at `pos`.
    ///
    /// `pos` must be `< self.len()`. For the very first token in the file
    /// this checks leading trivia; the `Parser` wrapper returns `false`
    /// for `pos == 0` since there is no preceding *significant* token.
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
}
