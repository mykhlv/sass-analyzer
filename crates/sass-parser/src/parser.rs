mod indented;

use crate::event::Event;
use crate::input::Input;
use crate::syntax_kind::SyntaxKind;
use crate::text_range::{TextRange, TextSize};
use crate::token_set::TokenSet;

use indented::IndentedState;
pub use indented::{HeaderLines, Joined};

const MAX_DEPTH: u32 = 256;

/// How blocks and statements are delimited in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// By real `{`, `}` and `;` tokens. Besides the end of input, the only fence is a
    /// brace that a string left unclosed swallowed, put back at its line break.
    Braced,
    /// By line breaks and indentation, as in indented Sass. A line break the grammar
    /// does not absorb ends the statement before it, and its indentation opens or
    /// closes blocks: the parser reads those as zero-width `;`, `{` and `}`.
    Indented,
}

/// The first position at or after the cursor where the token stream stops being the
/// input's: from `index` on, lookahead reads `kind`, a token with no source text.
///
/// End of input is a fence of kind `EOF`. Any other fence stands for a zero-width
/// token that the layout implies, which `bump` emits as `Event::Virtual` without
/// advancing the cursor, at `trivia` in the trivia before `index`.
#[derive(Debug, Clone, Copy)]
struct Fence {
    index: usize,
    kind: SyntaxKind,
    trivia: u32,
}

pub struct Parser<'src> {
    input: Input,
    source: &'src str,
    pos: usize,
    events: Vec<Event>,
    /// Error message storage, indexed by `Event::Error::msg_index`.
    /// Kept separate from events so `Event` stays at 16 bytes. On well-formed
    /// input this Vec is empty (zero allocations). Interning common messages
    /// (e.g. `Cow<'static, str>`) was considered but the gain is marginal since
    /// errors only occur on malformed input where parsing speed is not critical.
    error_messages: Vec<String>,
    depth: u32,
    layout: Layout,
    fence: Fence,
    /// Brackets (`()`, `[]`, `#{}`) the cursor is inside, counted by
    /// [`TransparentGuard`]. Line breaks there are plain trivia: the input records how
    /// deep in brackets each one lies, and the guards end a bracket that an error left
    /// unclosed.
    transparent: u32,
    /// Where the innermost of those brackets opens.
    bracket_start: TextSize,
    /// The last virtual token emitted: the cursor position and the trivia index it
    /// went at. Trivia before that index lies in front of it, not in front of the
    /// token at the cursor, which `has_whitespace_before` must not see.
    last_virtual: (usize, u32),
    /// Layout state of [`Layout::Indented`]; unused when braced.
    indented: IndentedState,
    /// Strings reported unclosed so far; see [`Parser::unclosed_strings`].
    unclosed_strings: u32,
    /// The cursor right after the last of them; see [`Parser::at_unclosed_string_break`].
    unclosed_string_end: usize,
    /// Where the last of them starts, and how its text ends.
    unclosed_string_start: TextSize,
    unclosed_string_tail: StringTail,
    /// Where each interpolated string bumped and not yet ended starts. One the lexer ended
    /// unclosed right after an interpolation never sees its `STRING_END`, and stays: it
    /// lies below every string opened after it, so it is never taken for one of them.
    open_strings: Vec<TextSize>,
    /// In the braced layout, where each `{` and `#{` not yet closed starts, innermost last.
    open_braces: Vec<TextSize>,
    /// For [`Parser::bracket_closes_in_kind`]: the first bracket it was asked about, and
    /// the openers from there on that do not close in kind, in order.
    unclosed_in_kind: Option<(usize, Vec<usize>)>,
    /// What that reading took each string left unclosed to have swallowed, by token
    /// index, for checking that the parser puts back the same brace.
    #[cfg(debug_assertions)]
    swallowed_in_kind: Vec<(usize, Option<SyntaxKind>)>,
}

impl<'src> Parser<'src> {
    pub fn new(input: Input, source: &'src str) -> Self {
        Self::with_layout(input, source, Layout::Braced)
    }

    /// A parser for `layout`. [`Layout::Indented`] needs the input's layout facts, so its
    /// input must come from [`Input::from_source_indented`].
    pub fn with_layout(input: Input, source: &'src str, layout: Layout) -> Self {
        let estimated_events = input.len().saturating_mul(2);
        let fence = eof_fence(&input);
        let mut parser = Self {
            input,
            source,
            pos: 0,
            events: Vec::with_capacity(estimated_events),
            error_messages: Vec::new(),
            depth: 0,
            layout,
            fence,
            transparent: 0,
            bracket_start: TextSize::from(0),
            last_virtual: (usize::MAX, 0),
            indented: IndentedState::new(),
            unclosed_strings: 0,
            unclosed_string_end: usize::MAX,
            unclosed_string_start: TextSize::from(0),
            unclosed_string_tail: StringTail::Plain,
            open_strings: Vec::new(),
            open_braces: Vec::new(),
            unclosed_in_kind: None,
            #[cfg(debug_assertions)]
            swallowed_in_kind: Vec::new(),
        };
        if layout == Layout::Indented {
            parser.find_line_start(0);
            parser.fence = parser.locate_indented();
        }
        parser
    }

    pub fn finish(mut self) -> (Vec<Event>, Vec<String>, Input, &'src str) {
        debug_assert_eq!(self.depth, 0, "depth guard leaked");
        debug_assert_eq!(self.transparent, 0, "transparent guard leaked");
        self.indented.debug_assert_finished();
        if self.layout == Layout::Indented {
            self.report_unterminated_comment();
        }
        (self.events, self.error_messages, self.input, self.source)
    }

    // ── Lookahead ────────────────────────────────────────────────────

    #[inline]
    pub fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    /// The kind `offset` tokens ahead. Lookahead does not cross a fence: every offset
    /// that reaches it reads the fence's kind.
    #[inline]
    pub fn nth(&self, offset: usize) -> SyntaxKind {
        let target = self.pos.saturating_add(offset);
        if target < self.fence.index {
            self.input.kind(target)
        } else {
            self.fence.kind
        }
    }

    #[inline]
    pub fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    #[inline]
    pub fn at_ts(&self, set: TokenSet) -> bool {
        set.contains(self.current())
    }

    #[inline]
    pub fn at_end(&self) -> bool {
        self.pos >= self.fence.index && self.fence.kind == SyntaxKind::EOF
    }

    /// Whether the cursor is at a zero-width token of the indented layout: a `{`, `;` or
    /// `}` that the indentation implies.
    #[inline]
    pub fn at_virtual(&self) -> bool {
        self.pos >= self.fence.index && self.fence.kind != SyntaxKind::EOF
    }

    /// Whether the cursor is at the `{` the indented layout implies before lines indented
    /// beneath the current one.
    #[inline]
    pub fn at_lines_beneath(&self) -> bool {
        self.layout == Layout::Indented && self.at(SyntaxKind::LBRACE) && self.at_virtual()
    }

    /// Whether the bracket at the cursor closes in kind: read on from it, its own closer
    /// comes before any closer of another kind, every bracket opened in between closing in
    /// kind too, and before the end of input. Braced layout only: the indented one lexes
    /// some lines again as it goes.
    ///
    /// Read once from the first bracket asked about: every bracket still open when a
    /// closer of another kind comes, or when the input ends, is one that does not close,
    /// whichever bracket the reading started at, and the cursor never goes back, so later
    /// ones are looked up.
    pub fn bracket_closes_in_kind(&mut self) -> bool {
        debug_assert_eq!(
            self.layout,
            Layout::Braced,
            "tokens may change when indented"
        );
        debug_assert!(bracket_closer(self.current()).is_some(), "not at a bracket");
        let pos = self.pos;
        let (from, unclosed) = if let Some(cached) = self.unclosed_in_kind.take() {
            cached
        } else {
            #[cfg(debug_assertions)]
            let mut swallowed = Vec::new();
            let unclosed = self.openers_not_closing_in_kind(
                pos,
                #[cfg(debug_assertions)]
                &mut swallowed,
            );
            #[cfg(debug_assertions)]
            {
                self.swallowed_in_kind = swallowed;
            }
            (pos, unclosed)
        };
        debug_assert!(
            from <= pos,
            "asked about a bracket before the first one asked"
        );
        let closes = unclosed.binary_search(&pos).is_err();
        self.unclosed_in_kind = Some((from, unclosed));
        closes
    }

    /// The openers from `from` on that do not close in kind. A string left unclosed counts
    /// as the brace it swallowed where the parser puts that brace back (see
    /// [`Parser::swallowed_brace`]), and as nothing otherwise.
    fn openers_not_closing_in_kind(
        &self,
        from: usize,
        #[cfg(debug_assertions)] swallowed: &mut Vec<(usize, Option<SyntaxKind>)>,
    ) -> Vec<usize> {
        let mut open: Vec<(usize, SyntaxKind)> = Vec::new();
        let mut unclosed = Vec::new();
        // Where each `{` and `#{` open stands, as `open_braces` will when the parser gets
        // there: every brace token opens or closes one, whatever the kinds around it.
        let mut braces = self.open_braces.clone();
        let mut string_starts: Vec<TextSize> = Vec::new();
        for index in from..self.input.len() {
            let range = self.input.range(index);
            let mut kind = self.input.kind(index);
            let string = match kind {
                SyntaxKind::QUOTED_STRING => Some((range.start(), &self.source[range][1..])),
                SyntaxKind::STRING_START => {
                    string_starts.push(range.start());
                    None
                }
                SyntaxKind::STRING_END => string_starts
                    .pop()
                    .map(|start| (start, &self.source[range])),
                _ => None,
            };
            if let Some((start, end_text)) = string {
                if !self.string_left_unclosed(start, end_text) {
                    continue;
                }
                let innermost = braces.last().copied();
                let brace = self.brace_swallowed(end_text.trim_end(), start, index + 1, innermost);
                #[cfg(debug_assertions)]
                swallowed.push((index, brace));
                match brace {
                    Some(brace) => kind = brace,
                    None => continue,
                }
            }
            match kind {
                SyntaxKind::LBRACE | SyntaxKind::HASH_LBRACE => {
                    // A brace put back after a string stands where the string ends.
                    let at = if self.input.kind(index) == kind {
                        range.start()
                    } else {
                        range.end()
                    };
                    braces.push(at);
                }
                SyntaxKind::RBRACE => {
                    braces.pop();
                }
                _ => {}
            }
            if let Some(closer) = bracket_closer(kind) {
                open.push((index, closer));
                continue;
            }
            if !matches!(
                kind,
                SyntaxKind::RPAREN | SyntaxKind::RBRACKET | SyntaxKind::RBRACE
            ) {
                continue;
            }
            match open.last() {
                Some(&(_, closer)) if closer == kind => {
                    open.pop();
                }
                Some(_) => unclosed.extend(open.drain(..).map(|(index, _)| index)),
                // A closer of nothing: no bracket is open to see it.
                None => {}
            }
        }
        unclosed.extend(open.into_iter().map(|(index, _)| index));
        unclosed
    }

    /// Where the contents of the innermost bracket end, read on from the cursor, when an
    /// error has left them unread: the number of tokens through the end, and what ends
    /// them. That is its closer `close`, once every bracket opened before it has closed in
    /// kind, or with `items` a `,` outside those brackets first, which ends the item at
    /// the cursor; or a string left unclosed, which swallowed the closer with the rest of
    /// its line. `None` if a `;`, a `{` outside interpolation, a `}` of no interpolation,
    /// a closer that does not match a bracket opened since the cursor, or the next fence
    /// comes first, or nothing within reach: the bracket is left unclosed instead.
    pub fn rest_of_bracket(&self, close: SyntaxKind, items: bool) -> Option<(usize, BracketRest)> {
        const REACH: usize = 4096;
        let mut open: Vec<SyntaxKind> = Vec::new();
        let mut string_starts: Vec<TextSize> = Vec::new();
        let end = self.fence.index.min(self.pos.saturating_add(REACH));
        for index in self.pos..end {
            let count = index - self.pos + 1;
            let range = self.input.range(index);
            let kind = self.input.kind(index);
            let string = match kind {
                SyntaxKind::SEMICOLON | SyntaxKind::LBRACE => return None,
                SyntaxKind::COMMA if items && open.is_empty() => {
                    return Some((count, BracketRest::Comma));
                }
                SyntaxKind::QUOTED_STRING => Some((range.start(), &self.source[range][1..])),
                SyntaxKind::STRING_START => {
                    string_starts.push(range.start());
                    None
                }
                SyntaxKind::STRING_END => string_starts
                    .pop()
                    .map(|start| (start, &self.source[range])),
                _ => None,
            };
            if let Some((start, end_text)) = string
                && self.string_left_unclosed(start, end_text)
            {
                return Some((count, BracketRest::UnclosedString));
            }
            if let Some(closer) = bracket_closer(kind) {
                open.push(closer);
            } else if matches!(
                kind,
                SyntaxKind::RPAREN | SyntaxKind::RBRACKET | SyntaxKind::RBRACE
            ) {
                match open.last() {
                    Some(&closer) if closer == kind => {
                        open.pop();
                    }
                    Some(_) => return None,
                    None if kind == close => return Some((count, BracketRest::Closer)),
                    // A stray `)` or `]` is more of what could not be read; a `}` closes
                    // the block.
                    None if kind == SyntaxKind::RBRACE => return None,
                    None => {}
                }
            }
        }
        None
    }

    /// Whether the string starting at `start`, whose text ends in `end_text`, lacks its
    /// closing quote.
    fn string_left_unclosed(&self, start: TextSize, end_text: &str) -> bool {
        let quote = self.source.as_bytes()[usize::from(start)];
        !crate::lexer::closes_string(end_text, quote)
    }

    /// Whether a line break comes before the token at the cursor, in the trivia after the
    /// token before it.
    pub fn at_line_break(&self) -> bool {
        self.pos < self.fence.index
            && self
                .input
                .trivia_before(self.pos)
                .iter()
                .any(|&(kind, range)| {
                    kind == SyntaxKind::WHITESPACE
                        && self.source[range].contains(['\n', '\r', '\x0c'])
                })
    }

    /// Whether blocks and statements are delimited by real tokens only. In the indented
    /// layouts a selector line with nothing indented beneath it is a rule with no block,
    /// and a bracket left open recovers at a line start, not at a brace.
    #[inline]
    pub fn braced(&self) -> bool {
        self.layout == Layout::Braced
    }

    /// The kind of the input token before the cursor, zero-width tokens aside; `EOF` at
    /// the start of the input.
    pub fn prev_kind(&self) -> SyntaxKind {
        match self.pos.checked_sub(1) {
            Some(prev) => self.input.kind(prev),
            None => SyntaxKind::EOF,
        }
    }

    /// The source text of the input token before the cursor; empty at the start of the
    /// input.
    pub fn prev_text(&self) -> &'src str {
        self.pos
            .checked_sub(1)
            .map_or("", |prev| &self.source[self.input.range(prev)])
    }

    /// Returns `false` for pos 0 (no preceding significant token) or at EOF.
    pub fn has_whitespace_before(&self) -> bool {
        self.nth_has_whitespace_before(0)
    }

    /// Returns the source text of the current token.
    pub fn current_text(&self) -> &'src str {
        self.nth_text(0)
    }

    /// Returns the source text of the token at `pos + offset`; empty at or past a fence.
    pub fn nth_text(&self, offset: usize) -> &'src str {
        let target = self.pos.saturating_add(offset);
        if target >= self.fence.index {
            return "";
        }
        let range = self.input.range(target);
        &self.source[range]
    }

    /// Whether any whitespace trivia exists before the token at `pos + offset`, counting
    /// only the trivia after the token before it, virtual ones included. False for the
    /// first token of the input, at EOF, and past the first token of a fence.
    pub fn nth_has_whitespace_before(&self, offset: usize) -> bool {
        let target = self.pos.saturating_add(offset);
        let end = match target.cmp(&self.fence.index) {
            std::cmp::Ordering::Less => self.input.trivia_end_index(target),
            std::cmp::Ordering::Equal if self.fence.kind != SyntaxKind::EOF => {
                self.fence.trivia as usize
            }
            _ => return false,
        };
        let start = if self.last_virtual.0 == target {
            self.last_virtual.1 as usize
        } else if target == 0 {
            return false;
        } else {
            self.input.trivia_start_index(target)
        };
        self.input.has_whitespace_in(start, end)
    }

    // ── Consuming tokens ─────────────────────────────────────────────

    pub fn bump(&mut self) {
        if self.pos >= self.fence.index {
            self.bump_virtual();
            return;
        }
        let kind = self.input.kind(self.pos);
        let range = self.input.range(self.pos);
        self.events.push(Event::Token { kind, range });
        self.pos += 1;
        match kind {
            SyntaxKind::QUOTED_STRING => {
                let text = &self.source[range];
                self.check_string_end(range.start(), &text[1..], range.end());
            }
            SyntaxKind::STRING_START => self.open_strings.push(range.start()),
            SyntaxKind::STRING_END => {
                if let Some(start) = self.open_strings.pop() {
                    self.check_string_end(start, &self.source[range], range.end());
                }
            }
            SyntaxKind::LBRACE | SyntaxKind::HASH_LBRACE if self.layout == Layout::Braced => {
                self.open_braces.push(range.start());
            }
            SyntaxKind::RBRACE if self.layout == Layout::Braced => {
                self.open_braces.pop();
            }
            _ => {}
        }
    }

    /// Report the string from `start` to `end` if `end_text`, its end, lacks the closing
    /// quote: the lexer ended it at a newline or at the end of input instead. That is the
    /// only trace such a string leaves, as it lexes as an ordinary string otherwise.
    fn check_string_end(&mut self, start: TextSize, end_text: &str, end: TextSize) {
        if self.string_left_unclosed(start, end_text) {
            self.unclosed_string(TextRange::new(start, end), end_text);
        }
    }

    /// Report the interpolated string that the token before the cursor, the `}` of an
    /// interpolation, ends: the lexer ended it at the newline or end of input right after.
    pub fn error_unclosed_string(&mut self) {
        if let Some(prev) = self.pos.checked_sub(1)
            && let Some(start) = self.open_strings.pop()
        {
            self.unclosed_string(TextRange::new(start, self.input.range(prev).end()), "");
        }
    }

    /// How many strings have been reported unclosed so far. The lexer ends everything a
    /// string is nested in — an interpolation, an enclosing string — together with it, so
    /// a construct inside which this count grew will never see its closing token.
    pub fn unclosed_strings(&self) -> u32 {
        self.unclosed_strings
    }

    /// Whether the cursor is right after a string left unclosed, at the line break that
    /// ended it. Where [`Parser::eat_unclosed_string_break`] takes that break for a comma,
    /// the list goes on; otherwise the string most likely swallowed the end of its
    /// construct, `m("x);`, which ends at the break unless the line after closes it.
    pub fn at_unclosed_string_break(&self) -> bool {
        self.pos == self.unclosed_string_end
            && (self.unclosed_string_break_is_comma() || self.at_unclosed_string_stop())
    }

    /// Take the line break at [`Parser::at_unclosed_string_break`] as a comma, once. Inside
    /// brackets, it stands for the comma the string swallowed with the rest of its line,
    /// `(a: "x,` / `b: 2)`, or the one a list laid out an item per line puts there:
    /// `f(` / `"x` / `2)`.
    pub fn eat_unclosed_string_break(&mut self) -> bool {
        let at = self.pos == self.unclosed_string_end && self.unclosed_string_break_is_comma();
        if at {
            self.unclosed_string_end = usize::MAX;
        }
        at
    }

    /// Whether the innermost bracket takes the break for a comma. Asked again as each
    /// bracket ends: `(` / `a: f("x` / `b: 2)` stops `f(` but goes on with the map.
    fn unclosed_string_break_is_comma(&self) -> bool {
        self.transparent > 0
            && match self.unclosed_string_tail {
                StringTail::Comma => true,
                StringTail::Closer => false,
                // A bracket opened at the break itself is not the string's.
                StringTail::Plain => {
                    self.bracket_start < self.unclosed_string_start
                        && self.source
                            [TextRange::new(self.bracket_start, self.unclosed_string_start)]
                        .contains('\n')
                        && !self.at_statement_beneath_bracket()
                }
            }
    }

    /// Whether the line after the break starts a statement no deeper than the line that
    /// opened the innermost bracket: the statement after the string's, not the next item.
    /// `b: f(` / `"x` / `c: d;` ends the call there, while the items of `$m: (` / `a: "x`
    /// / `b: 2` lie deeper than its opening line.
    fn at_statement_beneath_bracket(&self) -> bool {
        if self.pos >= self.input.len() {
            return false;
        }
        let statement = match self.current() {
            SyntaxKind::AT | SyntaxKind::RBRACE => true,
            SyntaxKind::DOLLAR => {
                self.nth(1) == SyntaxKind::IDENT && self.nth(2) == SyntaxKind::COLON
            }
            SyntaxKind::IDENT => {
                self.nth(1) == SyntaxKind::COLON && !self.nth_has_whitespace_before(1)
            }
            _ => false,
        };
        statement
            && self.line_indent(self.input.range(self.pos).start())
                <= self.line_indent(self.bracket_start)
    }

    fn at_unclosed_string_stop(&self) -> bool {
        self.pos == self.unclosed_string_end
            && self.unclosed_string_tail != StringTail::Comma
            && !self.unclosed_string_break_is_comma()
            && !matches!(
                self.current(),
                SyntaxKind::RPAREN | SyntaxKind::RBRACKET | SyntaxKind::COMMA
            )
    }

    /// Report the string at `range`, whose text ends in `tail`.
    fn unclosed_string(&mut self, range: TextRange, tail: &str) {
        self.unclosed_strings += 1;
        self.unclosed_string_end = self.pos;
        self.unclosed_string_start = range.start();
        let tail = tail.trim_end();
        self.unclosed_string_tail = if tail.ends_with(',') {
            StringTail::Comma
        } else if tail.ends_with([')', ']', ';', '{', '}']) {
            StringTail::Closer
        } else {
            StringTail::Plain
        };
        let swallowed = self.swallowed_brace(tail, range.start());
        #[cfg(debug_assertions)]
        if let Ok(at) = self
            .swallowed_in_kind
            .binary_search_by_key(&(self.pos - 1), |&(index, _)| index)
        {
            debug_assert_eq!(
                self.swallowed_in_kind[at].1, swallowed,
                "the look ahead and the parser disagree on a swallowed brace"
            );
        }
        if let Some(kind) = swallowed {
            #[allow(clippy::cast_possible_truncation)]
            let trivia = self.input.trivia_start_index(self.pos) as u32;
            self.fence = Fence {
                index: self.pos,
                kind,
                trivia,
            };
        }
        self.error_at(range, "unterminated string");
    }

    /// The brace that a string left unclosed, starting at `start` and ending in `tail`,
    /// swallowed with the rest of its line, to put back at the line break as a virtual
    /// token; braced layout only. A `}` most likely closed the innermost block,
    /// `.a { b: "x }`, and a `{` opened a body, `@include m("x) {`. The next line has the
    /// last word, by its indentation: a body is indented beneath the line that opens it
    /// and ends in a `}` level with it, while a closed block's next line is no deeper than
    /// the line it opened on, nor a `}` level with that line, which would close it as
    /// written. A brace inside an interpolation is the interpolation's.
    fn swallowed_brace(&self, tail: &str, start: TextSize) -> Option<SyntaxKind> {
        if self.layout != Layout::Braced {
            return None;
        }
        self.brace_swallowed(tail, start, self.pos, self.open_braces.last().copied())
    }

    /// [`Parser::swallowed_brace`] of a string starting at `start` and ending in `tail`,
    /// the token after it at `next`, inside the `{` or `#{` that starts at `innermost`.
    fn brace_swallowed(
        &self,
        tail: &str,
        start: TextSize,
        next: usize,
        innermost: Option<TextSize>,
    ) -> Option<SyntaxKind> {
        if innermost.is_some_and(|at| self.source[usize::from(at)..].starts_with("#{")) {
            return None;
        }
        let opens = tail.matches('{').count();
        let closes = tail.matches('}').count();
        let next = (next < self.input.len()).then(|| {
            let indent = self.line_indent(self.input.range(next).start());
            (self.input.kind(next), indent)
        });
        if tail.ends_with('}') && closes > opens {
            let block = self.line_indent(innermost?);
            match next {
                None => Some(SyntaxKind::RBRACE),
                Some((kind, indent)) => (indent < block
                    || indent == block && kind != SyntaxKind::RBRACE)
                    .then_some(SyntaxKind::RBRACE),
            }
        } else if tail.ends_with('{') && opens > closes {
            let (kind, indent) = next?;
            let header = self.line_indent(start);
            (indent > header || indent == header && kind == SyntaxKind::RBRACE)
                .then_some(SyntaxKind::LBRACE)
        } else {
            None
        }
    }

    /// The width of the indentation of the line that `offset` lies on.
    fn line_indent(&self, offset: TextSize) -> usize {
        let before = &self.source[..usize::from(offset)];
        let line = &before[before.rfind('\n').map_or(0, |i| i + 1)..];
        line.len() - line.trim_start_matches([' ', '\t']).len()
    }

    /// Bump the current token as `kind` rather than the kind it was lexed as.
    pub fn bump_remap(&mut self, kind: SyntaxKind) {
        debug_assert!(self.pos < self.fence.index, "bump_remap at a fence");
        let range = self.input.range(self.pos);
        self.events.push(Event::Token { kind, range });
        self.pos += 1;
    }

    #[cold]
    #[inline(never)]
    fn bump_virtual(&mut self) {
        let Fence {
            index,
            kind,
            trivia,
        } = self.fence;
        assert!(kind != SyntaxKind::EOF, "bump at end of input");
        debug_assert_eq!(index, self.pos);
        if self.layout == Layout::Braced {
            // A brace put back after a string left unclosed, past which the string's
            // construct has ended.
            self.unclosed_string_end = usize::MAX;
            if kind == SyntaxKind::LBRACE {
                self.open_braces.push(self.virtual_offset());
            } else {
                self.open_braces.pop();
            }
        }
        self.events.push(Event::Virtual { kind, trivia });
        self.last_virtual = (index, trivia);
        if self.layout == Layout::Indented {
            self.virtual_bumped(kind);
        }
        self.fence = self.locate_fence();
    }

    /// The fence ahead of the cursor, from the layout's state.
    fn locate_fence(&mut self) -> Fence {
        match self.layout {
            Layout::Braced => eof_fence(&self.input),
            Layout::Indented => self.locate_indented(),
        }
    }

    pub fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub fn expect(&mut self, kind: SyntaxKind) -> bool {
        if self.eat(kind) {
            return true;
        }
        // Swallowed by a string left unclosed, already reported. A `}` is not: the block
        // it closes spans lines, so a missing one is a separate mistake, unless the string
        // swallowed it on the line it ends, which puts it back: see `swallowed_brace`.
        if matches!(
            kind,
            SyntaxKind::RPAREN | SyntaxKind::RBRACKET | SyntaxKind::SEMICOLON
        ) && self.at_unclosed_string_stop()
        {
            return false;
        }
        self.error(format!("expected {kind:?}"));
        false
    }

    // ── Markers ──────────────────────────────────────────────────────

    pub fn start(&mut self) -> Marker {
        let len = self.events.len();
        debug_assert!(u32::try_from(len).is_ok(), "event count overflows u32");
        #[allow(clippy::cast_possible_truncation)]
        let pos = len as u32;
        self.events.push(Event::Enter {
            kind: SyntaxKind::ERROR,
            forward_parent: None,
        });
        Marker::new(pos)
    }

    // ── Errors ───────────────────────────────────────────────────────

    pub fn error(&mut self, msg: impl Into<String>) {
        if self.layout == Layout::Indented {
            self.end_header_before_error();
        }
        let range = self.error_range();
        self.error_at(range, msg);
        if self.layout == Layout::Indented {
            self.error_reported();
        }
    }

    /// Where [`Parser::error`] reports at the cursor.
    pub fn error_range(&self) -> TextRange {
        if self.pos < self.fence.index {
            self.input.range(self.pos)
        } else if self.fence.kind != SyntaxKind::EOF {
            TextRange::empty(self.virtual_offset())
        } else if self.fence.index < self.input.len() {
            // A bracket's recovery point, which reads as the end of input.
            self.input.range(self.fence.index)
        } else {
            TextRange::empty(TextSize::of(self.source))
        }
    }

    /// Report `msg` at `range`, taken with [`Parser::error_range`] where `m` was started,
    /// as if it had been reported there and then: before the errors reported since. For
    /// a mistake known only once the tokens after it have been read. Braced layout only.
    pub fn error_at_start(&mut self, m: &Marker, range: TextRange, msg: impl Into<String>) {
        debug_assert_eq!(self.layout, Layout::Braced, "no bracket recovery to update");
        let len = self.events.len();
        self.error_at(range, msg);
        let event = self.events.remove(len);
        self.events.insert(m.pos as usize + 1, event);
    }

    /// How many errors have been reported so far.
    pub fn error_count(&self) -> usize {
        self.error_messages.len()
    }

    /// Report `msg` at the end of the input token before the cursor rather than at the
    /// cursor: where a missing token belongs, not past the trivia or on the line after.
    pub fn error_after_prev(&mut self, msg: impl Into<String>) {
        let Some(prev) = self.pos.checked_sub(1) else {
            self.error(msg);
            return;
        };
        let range = TextRange::empty(self.input.range(prev).end());
        self.error_at(range, msg);
        if self.layout == Layout::Indented {
            self.error_reported();
        }
    }

    fn error_at(&mut self, range: TextRange, msg: impl Into<String>) {
        let msg_len = self.error_messages.len();
        debug_assert!(
            u32::try_from(msg_len).is_ok(),
            "error message count overflows u32"
        );
        #[allow(clippy::cast_possible_truncation)]
        let msg_index = msg_len as u32;
        self.error_messages.push(msg.into());
        self.events.push(Event::Error { msg_index, range });
    }

    pub fn err_and_bump(&mut self, msg: impl Into<String>) {
        let m = self.start();
        self.error(msg);
        self.bump();
        let _ = m.complete(self, SyntaxKind::ERROR);
    }

    /// Where the virtual token at the fence sits in the source: at the start of the
    /// trivia token it goes in front of, or of the token after all of them.
    fn virtual_offset(&self) -> TextSize {
        let trivia = self.fence.trivia as usize;
        if trivia < self.input.trivia_end_index(self.fence.index) {
            self.input.all_trivia()[trivia].1.start()
        } else if self.fence.index < self.input.len() {
            self.input.range(self.fence.index).start()
        } else {
            TextSize::of(self.source)
        }
    }

    // ── Recursion depth ──────────────────────────────────────────────

    #[allow(clippy::result_unit_err)]
    pub fn depth_guard(&mut self) -> Result<DepthGuard<'_, 'src>, ()> {
        if self.depth >= MAX_DEPTH {
            self.error("nesting too deep");
            return Err(());
        }
        self.depth += 1;
        Ok(DepthGuard { parser: self })
    }

    // ── Layout scopes ────────────────────────────────────────────────

    /// Enter a bracketed construct (`()`, `[]`, `#{}`) at its opening token, where line
    /// breaks are plain trivia whatever the layout. The scope ends when the guard drops,
    /// so a bracket left unclosed by an error does not outlive the function that opened
    /// it: in the indented layout, the line breaks after it count again.
    pub fn transparent_guard(&mut self) -> TransparentGuard<'_, 'src> {
        debug_assert!(self.pos < self.fence.index, "transparent_guard at a fence");
        let outer_start =
            std::mem::replace(&mut self.bracket_start, self.input.range(self.pos).start());
        self.transparent += 1;
        if self.layout == Layout::Indented {
            self.enter_bracket();
        }
        TransparentGuard {
            parser: self,
            outer_start,
        }
    }
}

/// What ends the contents of a bracket in [`Parser::rest_of_bracket`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketRest {
    Closer,
    Comma,
    UnclosedString,
}

/// How the text of a string left unclosed ends, before the line break that ended it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StringTail {
    /// In a comma it swallowed.
    Comma,
    /// In a token that closes a construct: `)`, `]`, `;`, `{` or `}`.
    Closer,
    Plain,
}

/// The closer of a bracket that `kind` opens, if it opens one.
fn bracket_closer(kind: SyntaxKind) -> Option<SyntaxKind> {
    match kind {
        SyntaxKind::LPAREN => Some(SyntaxKind::RPAREN),
        SyntaxKind::LBRACKET => Some(SyntaxKind::RBRACKET),
        SyntaxKind::LBRACE | SyntaxKind::HASH_LBRACE => Some(SyntaxKind::RBRACE),
        _ => None,
    }
}

fn eof_fence(input: &Input) -> Fence {
    #[allow(clippy::cast_possible_truncation)]
    let trivia = input.all_trivia().len() as u32;
    Fence {
        index: input.len(),
        kind: SyntaxKind::EOF,
        trivia,
    }
}

// ── DropBomb ─────────────────────────────────────────────────────────

struct DropBomb {
    defused: bool,
}

impl DropBomb {
    fn new() -> Self {
        Self { defused: false }
    }

    fn defuse(&mut self) {
        self.defused = true;
    }
}

impl Drop for DropBomb {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(self.defused, "marker must be completed or abandoned");
        }
    }
}

// ── Marker ───────────────────────────────────────────────────────────

pub struct Marker {
    pos: u32,
    bomb: DropBomb,
}

impl Marker {
    fn new(pos: u32) -> Self {
        Self {
            pos,
            bomb: DropBomb::new(),
        }
    }

    pub fn complete(mut self, p: &mut Parser<'_>, kind: SyntaxKind) -> CompletedMarker {
        self.bomb.defuse();
        match &mut p.events[self.pos as usize] {
            Event::Enter { kind: slot, .. } => *slot = kind,
            _ => unreachable!(),
        }
        p.events.push(Event::Exit);
        CompletedMarker { pos: self.pos }
    }

    pub fn abandon(mut self, p: &mut Parser<'_>) {
        self.bomb.defuse();
        if self.pos as usize == p.events.len() - 1 {
            match p.events.pop() {
                Some(Event::Enter {
                    kind: SyntaxKind::ERROR,
                    forward_parent: None,
                }) => {}
                _ => unreachable!(),
            }
        } else {
            // Events were pushed after this marker — replace with Tombstone
            // so the bridge skips it (no matching Exit).
            p.events[self.pos as usize] = Event::Tombstone;
        }
    }
}

// ── CompletedMarker ──────────────────────────────────────────────────

#[must_use]
pub struct CompletedMarker {
    pos: u32,
}

impl CompletedMarker {
    pub fn precede(self, p: &mut Parser<'_>) -> Marker {
        let new_pos = p.start();
        match &mut p.events[self.pos as usize] {
            Event::Enter { forward_parent, .. } => {
                *forward_parent = Some(new_pos.pos);
            }
            _ => unreachable!(),
        }
        new_pos
    }

    #[allow(clippy::indexing_slicing)]
    pub fn kind(&self, p: &Parser<'_>) -> SyntaxKind {
        match p.events[self.pos as usize] {
            Event::Enter { kind, .. } => kind,
            _ => unreachable!(),
        }
    }
}

// ── DepthGuard ───────────────────────────────────────────────────────

pub struct DepthGuard<'p, 'src> {
    parser: &'p mut Parser<'src>,
}

impl<'src> std::ops::Deref for DepthGuard<'_, 'src> {
    type Target = Parser<'src>;

    fn deref(&self) -> &Parser<'src> {
        self.parser
    }
}

impl<'src> std::ops::DerefMut for DepthGuard<'_, 'src> {
    fn deref_mut(&mut self) -> &mut Parser<'src> {
        self.parser
    }
}

impl Drop for DepthGuard<'_, '_> {
    fn drop(&mut self) {
        self.parser.depth -= 1;
    }
}

// ── TransparentGuard ─────────────────────────────────────────────────

pub struct TransparentGuard<'p, 'src> {
    parser: &'p mut Parser<'src>,
    outer_start: TextSize,
}

impl<'src> std::ops::Deref for TransparentGuard<'_, 'src> {
    type Target = Parser<'src>;

    fn deref(&self) -> &Parser<'src> {
        self.parser
    }
}

impl<'src> std::ops::DerefMut for TransparentGuard<'_, 'src> {
    fn deref_mut(&mut self) -> &mut Parser<'src> {
        self.parser
    }
}

impl Drop for TransparentGuard<'_, '_> {
    fn drop(&mut self) {
        self.parser.transparent -= 1;
        self.parser.bracket_start = self.outer_start;
        if self.parser.layout == Layout::Indented {
            self.parser.exit_bracket();
        }
    }
}

#[cfg(test)]
impl Parser<'_> {
    /// Puts a fence where no layout would yet: the unit tests below drive the virtual-token
    /// machinery with it. The braced layout puts the fence back at EOF after one bump.
    fn set_fence(&mut self, index: usize, kind: SyntaxKind, trivia: u32) {
        debug_assert!(index >= self.pos && index <= self.input.len());
        self.fence = Fence {
            index,
            kind,
            trivia,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;
    use crate::event::Event;
    use crate::input::Input;
    use crate::syntax_kind::SyntaxKind::{DOT, EOF, IDENT, LBRACE};

    #[test]
    fn lookahead_reads_the_fence_kind_from_the_fence_on() {
        let source = ".a\n  b";
        let mut p = Parser::new(Input::from_source(source), source);
        // After the line break, as a shorthand's `@` goes.
        p.set_fence(2, LBRACE, 1);
        assert_eq!(
            [p.nth(0), p.nth(1), p.nth(2), p.nth(3), p.nth(9)],
            [DOT, IDENT, LBRACE, LBRACE, LBRACE]
        );
        assert_eq!(p.nth_text(1), "a");
        assert_eq!(p.nth_text(2), "");
        p.bump();
        p.bump();
        assert!(p.at(LBRACE));
        assert!(!p.at_end());
        assert!(p.has_whitespace_before());
        assert_eq!(p.current_text(), "");

        p.bump();
        assert_eq!(
            p.events.last(),
            Some(&Event::Virtual {
                kind: LBRACE,
                trivia: 1
            })
        );
        assert_eq!(p.pos, 2, "a virtual token consumes no input");
        assert_eq!([p.nth(0), p.nth(1)], [IDENT, EOF]);
        assert_eq!(p.current_text(), "b");
        assert!(
            !p.has_whitespace_before(),
            "the whitespace is the virtual token's"
        );
        p.bump();
        assert!(p.at_end());
    }

    #[test]
    fn an_error_at_a_virtual_token_is_empty_where_the_token_goes() {
        let source = ".a // c\n  b";
        let mut p = Parser::new(Input::from_source(source), source);
        // Trivia: " ", "// c", "\n  " — the block opens in front of the line break.
        p.set_fence(2, LBRACE, 2);
        p.bump();
        p.bump();
        p.error("here");
        assert_eq!(
            p.events.last(),
            Some(&Event::Error {
                msg_index: 0,
                range: crate::text_range::TextRange::empty(7.into()),
            })
        );
    }

    /// Re-lexing a comment on a continued line lexes and compares only the tokens around
    /// the lines it changes, wherever in the input they are: that work grows with the
    /// number of such comments, not with that number times the length of the input.
    #[test]
    fn relexing_continued_lines_is_linear_in_the_input() {
        for statement in [
            // The comment reaches over `2` until it is re-lexed: the tokens change.
            "$a: 1 +\n  // c\n    2\n",
            // Nothing deeper follows the comment: re-lexing changes nothing.
            ".a,\n// c\n.b\n  c: d\n",
        ] {
            let work = |count: usize| {
                let source = statement.repeat(count);
                crate::input::RELEX_WORK.set(0);
                let input = Input::from_source_indented(&source);
                let mut p = Parser::with_layout(input, &source, super::Layout::Indented);
                crate::grammar::source_file(&mut p);
                let _ = p.finish();
                crate::input::RELEX_WORK.get()
            };
            let (small, large) = (work(200), work(2_000));
            assert!(
                small > 0 && large <= small * 11,
                "{statement:?}: {small} tokens lexed or visited for 200 statements, \
                 {large} for 2000",
            );
            assert!(
                large <= 2_000 * 32,
                "{statement:?}: {large} for 2000 statements"
            );
        }
    }

    /// Bump every token the indented layout presents, real or zero-width, and list
    /// them: `{}` and `;` for the zero-width ones, the source text for the others.
    fn indented_stream(source: &str) -> (String, Vec<String>) {
        let input = Input::from_source_indented(source);
        let mut p = Parser::with_layout(input, source, super::Layout::Indented);
        let mut out = Vec::new();
        while !p.at_end() {
            let text = p.current_text();
            out.push(if text.is_empty() {
                format!("<{:?}>", p.current())
            } else {
                text.to_owned()
            });
            p.bump();
        }
        let (_, errors, _, _) = p.finish();
        (out.join(" "), errors)
    }

    #[test]
    fn indentation_opens_and_closes_blocks_and_ends_statements() {
        let (stream, errors) = indented_stream(".a\n  b: c\n  .d\n    e: f\n.g\n");
        assert_eq!(
            stream,
            ". a <LBRACE> b : c <SEMICOLON> . d <LBRACE> e : f <SEMICOLON> <RBRACE> <RBRACE> . g \
             <SEMICOLON>",
        );
        assert!(errors.is_empty());
    }

    #[test]
    fn brackets_and_comment_lines_do_not_end_statements() {
        let (stream, _) = indented_stream("$a: f(1,\n2)\n  // c\n$b: 3\n");
        assert_eq!(
            stream, "$ a : f ( 1 , 2 ) <LBRACE> <RBRACE> $ b : 3 <SEMICOLON>",
            "the comment line deeper than the statement opens a block that it closes",
        );
    }

    #[test]
    fn indentation_whitespace_at_the_end_opens_no_block() {
        let (stream, _) = indented_stream(".a\n  b: c\n    ");
        assert_eq!(stream, ". a <LBRACE> b : c <SEMICOLON> <RBRACE>");
    }

    #[test]
    fn a_dedent_to_no_open_column_is_reported_once_per_line() {
        let (stream, errors) = indented_stream(".a\n    .b\n      c: d\n  .e\n    f: g\n");
        assert!(stream.contains("<RBRACE> <RBRACE> . e"), "{stream}");
        assert_eq!(errors, ["inconsistent indentation, expected 4 spaces"]);
    }

    #[test]
    #[should_panic(expected = "bump at end of input")]
    fn bump_at_end_of_input_panics() {
        let mut p = Parser::new(Input::from_source(""), "");
        p.bump();
    }

    #[test]
    fn transparent_guard_is_balanced() {
        let source = "(a)";
        let mut p = Parser::new(Input::from_source(source), source);
        {
            let mut g = p.transparent_guard();
            assert_eq!(g.transparent, 1);
            g.bump();
        }
        assert_eq!(p.transparent, 0);
        assert!(p.at(IDENT));
    }
}
