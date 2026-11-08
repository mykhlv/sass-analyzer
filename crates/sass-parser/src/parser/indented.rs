//! The indented layout: statement and block boundaries from line breaks.
//!
//! A line break that the grammar does not absorb is a *line start*. At each one the
//! parser emits, as zero-width tokens, what the indentation implies, by the same rules
//! the indented syntax has always followed here:
//!
//! - after content, a deeper line opens a block (`{`); a line at the same or a lower
//!   column ends the statement (`;`);
//! - every open block deeper than the line closes (`}`), including at comment-only lines;
//! - the end of input ends the statement and closes every block.
//!
//! The grammar absorbs a line break where the statement goes on: with
//! [`Parser::continue_line`] right after a token that leaves it unfinished (an operator, a
//! comma, a position of a header that requires more), and with
//! [`Parser::continue_header`], which declares how an at-rule's header goes on after its
//! name or one of its words. Line breaks inside brackets are plain trivia.

use std::cell::OnceCell;

use super::{Fence, Parser};
use crate::event::Event;
use crate::input::{IndentKind, LineBreak, bracket_depth_after};
use crate::syntax_kind::SyntaxKind;
use crate::text_range::{TextRange, TextSize};
use crate::token_set::TokenSet;

/// How the header of a statement — an at-rule's prelude — goes on across line breaks,
/// declared by the grammar function that parses it. It holds until the statement ends
/// or its block opens.
///
/// A header goes on over a line break when the line ends with the header's name and
/// `name_continues` (`@mixin` / `  name`), or with one of `end_words` (`@for $i from` /
/// `  1 through 3`), blank lines included. Elsewhere the grammar continues the
/// line itself where the header cannot end (see [`Parser::continue_line`]).
#[derive(Debug)]
pub struct HeaderLines {
    pub end_words: &'static [&'static str],
    pub name_continues: bool,
}

/// Tokens after which a header's next token starts an operand.
#[rustfmt::skip]
const OPERAND_STARTS_AFTER: TokenSet = {
    #[allow(clippy::enum_glob_use)]
    use SyntaxKind::*;
    TokenSet::new(&[
        LPAREN, LBRACKET, HASH_LBRACE, COMMA, COLON, BANG, PLUS, MINUS, STAR, SLASH, PERCENT,
        EQ_EQ, BANG_EQ, GT_EQ, LT_EQ, GT, LT,
    ])
};

/// How many lines [`Parser::bracket_left_unclosed`] reads at most: far beyond any
/// selector argument or at-rule prelude laid out over lines. Every line counts, those of
/// the block below the bracket's line included.
const UNCLOSED_LOOKAHEAD_LINES: u32 = 256;

/// How far [`Parser::joined`] reads past the cursor at most, in tokens: beyond any
/// grammar scanner's budget.
const JOINED_LOOKAHEAD: usize = 256;

/// Where [`Parser::joined`] lookahead ends, and what it reads there.
#[derive(Debug, Clone, Copy)]
pub struct Joined {
    limit: usize,
    kind: SyntaxKind,
    trivia: u32,
}

pub(super) struct IndentedState {
    /// Columns of the open blocks, innermost last; the file itself is at 0.
    columns: Vec<u32>,
    line: Line,
    /// The token the current statement started with: the last line start processed, or 0.
    statement_start: usize,
    /// The header being parsed, with the index of its name token.
    header: Option<(&'static HeaderLines, usize)>,
    /// The line start an error has ended the header at: the grammar cannot go on to it.
    header_ended: Option<usize>,
    /// Brackets left unclosed by an error when the function that opened them returned.
    /// The line breaks after them count as being that much less deep.
    unwound: u16,
    brackets: Vec<Bracket>,
    /// The last answer of `line_at_or_before_column`: `(column, from, found)`. It holds for
    /// any start from `from` up to `found`, as for the brackets opened or erroring on one
    /// line, which would otherwise each walk the same lines again.
    line_search: Option<(u32, usize, usize)>,
    /// For the searches over line breaks that may reach far: see [`BreakSearch`]. Built the
    /// first time a search does, and again after the input's line breaks change.
    break_search: OnceCell<BreakSearch>,
    /// How the file indents: decided by its first indented line.
    indent_kind: IndentKind,
    reported_mixed: bool,
    reported_kind: bool,
}

/// The next line start: the token after a line break the layout has not yet dealt with,
/// or EOF.
#[derive(Clone, Copy)]
struct Line {
    index: usize,
    /// The line breaks before `index` still to process, `next..end`, as indices into all
    /// of the input's line breaks. A comment-only line in between has a break of its own.
    next: u32,
    end: u32,
    /// A token has come since the last line break processed: the statement is open.
    saw_content: bool,
    /// The break and the column of the last block closed at it.
    popped: Option<(u32, u32)>,
    /// The grammar has found the line above unfinished: its line breaks go on.
    goes_on: GoesOn,
}

/// Which line breaks a line that the grammar found unfinished goes on over.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GoesOn {
    No,
    /// Up to a blank line.
    Lines,
    /// Across blank lines too, as after a comma.
    AcrossBlank,
}

/// A bracket being parsed, for recovery when it is left unclosed.
struct Bracket {
    /// Its opening token.
    pos: usize,
    /// Errors reported before it opened.
    errors: usize,
    /// Indentation of the line it opened on.
    column: u32,
    /// Once an error is reported inside it: the first line start after the error at a
    /// column no deeper than `column`, where the input reads as ending.
    sync: Option<usize>,
}

impl IndentedState {
    pub(super) fn new() -> Self {
        Self {
            columns: vec![0],
            line: Line {
                index: 0,
                next: 0,
                end: 0,
                saw_content: false,
                popped: None,
                goes_on: GoesOn::No,
            },
            statement_start: 0,
            header: None,
            header_ended: None,
            unwound: 0,
            brackets: Vec::new(),
            line_search: None,
            break_search: OnceCell::new(),
            indent_kind: IndentKind::None,
            reported_mixed: false,
            reported_kind: false,
        }
    }

    pub(super) fn debug_assert_finished(&self) {
        debug_assert!(self.brackets.is_empty(), "bracket guard leaked");
    }
}

impl<'src> Parser<'src> {
    // ── Grammar interface ────────────────────────────────────────────

    /// Absorb the line break ahead, if the cursor is at the end of its line: the
    /// statement goes on on the next line, whatever its indentation. A blank line stops
    /// it. Returns whether there was a line break to absorb.
    #[inline]
    pub fn continue_line(&mut self) -> bool {
        self.layout == super::Layout::Indented && self.absorb_line_breaks(false)
    }

    /// [`Parser::continue_line`] across blank lines too, as after a comma.
    #[inline]
    pub fn continue_line_across_blank(&mut self) -> bool {
        self.layout == super::Layout::Indented && self.absorb_line_breaks(true)
    }

    /// [`Parser::continue_line`], across blank lines too if the line after them starts
    /// with one of `starts`, as an operator's operand does, and not like a declaration
    /// (`name:`), which is no operand.
    #[inline]
    pub fn continue_line_to(&mut self, starts: TokenSet) -> bool {
        if self.layout != super::Layout::Indented {
            return false;
        }
        let index = self.indented.line.index;
        let across_blank = index < self.input.len()
            && starts.contains(self.input.kind(index))
            && !self.starts_declaration(index);
        self.absorb_line_breaks(across_blank)
    }

    /// When the token at the cursor ends its line: whether the line that
    /// [`Parser::continue_line_to`] goes on to after it, past any blank lines, starts like
    /// a declaration (`name:`). Always false when braced.
    pub fn next_line_starts_declaration(&self) -> bool {
        let line = self.indented.line;
        self.layout == super::Layout::Indented
            && line.index == self.pos + 1
            && line.next < line.end
            && self.starts_declaration(line.index)
    }

    fn starts_declaration(&self, index: usize) -> bool {
        index + 1 < self.input.len()
            && self.input.kind(index) == SyntaxKind::IDENT
            && self.input.kind(index + 1) == SyntaxKind::COLON
    }

    /// Declare how the header of the statement being parsed goes on across lines (see
    /// [`HeaderLines`]); the token before the cursor is its name. Ignored when braced.
    #[inline]
    pub fn continue_header(&mut self, lines: &'static HeaderLines) {
        if self.layout != super::Layout::Indented {
            return;
        }
        self.indented.header = Some((lines, self.pos.wrapping_sub(1)));
        self.fence = self.locate_fence();
    }

    /// The grammar is at the first token of an at-rule. A header still declared then is
    /// that of a statement an error ended before its line did, and the at-rule is a new
    /// statement on the same line: the rest of the line is no longer that header, and a
    /// deeper line below cannot go on with it. Whether that line goes on was decided ahead
    /// of the cursor, with the old header; decide it again. Tokens that start no statement
    /// are left to the broken one.
    #[inline]
    pub fn begin_at_rule(&mut self) {
        if self.layout == super::Layout::Indented && self.indented.header.is_some() {
            self.drop_stale_header();
        }
    }

    #[cold]
    fn drop_stale_header(&mut self) {
        self.indented.header = None;
        // The line breaks before the cursor went on with the statement; those after the
        // at-rule's `@` are decided ahead, but nothing at them has been emitted yet.
        if self.indented.line.index > self.pos {
            self.find_line_start(self.pos + 1);
            self.fence = self.locate_fence();
        }
    }

    /// Whether the cursor is at the first token of a statement: the first of a line the
    /// layout ended the previous statement at, or of the input. Always false when braced.
    #[inline]
    pub fn at_statement_start(&self) -> bool {
        self.layout == super::Layout::Indented
            && self.pos == self.indented.statement_start
            && self.pos < self.fence.index
    }

    /// At `=name` or `+name` starting a statement: the indented syntax's shorthands for
    /// `@mixin name` and `@include name`.
    #[inline]
    pub fn at_shorthand(&self) -> bool {
        if !self.at_statement_start() {
            return false;
        }
        match self.input.kind(self.pos) {
            SyntaxKind::EQ => true,
            SyntaxKind::PLUS => {
                self.pos + 1 < self.fence.index
                    && self.input.kind(self.pos + 1) == SyntaxKind::IDENT
                    && self.input.trivia_before(self.pos + 1).is_empty()
            }
            _ => false,
        }
    }

    /// Lookahead for a decision that has to be made before the grammar reaches the end
    /// of the line: it reads on past each line break that follows one of `joins`, as the
    /// grammar will absorb it there. Elsewhere it is [`Parser::nth`]'s lookahead.
    #[inline]
    pub fn joined(&self, joins: TokenSet) -> Joined {
        let joined = Joined {
            limit: self.fence.index,
            kind: self.fence.kind,
            trivia: self.fence.trivia,
        };
        if self.layout != super::Layout::Indented
            || self.fence.kind == SyntaxKind::EOF
            || self.pos >= self.fence.index
        {
            return joined;
        }
        self.joined_lines(joins, joined)
    }

    fn joined_lines(&self, joins: TokenSet, mut joined: Joined) -> Joined {
        let top = *self.indented.columns.last().unwrap_or(&0);
        let mut line = self.fence.index;
        loop {
            let last = self.input.kind(line - 1);
            if !joins.contains(last) {
                return joined;
            }
            let (mut next, end) = if line == self.indented.line.index {
                (self.indented.line.next, self.indented.line.end)
            } else {
                self.input.line_break_range(line)
            };
            while next < end
                && (last == SyntaxKind::COMMA || !self.input.line_break(next).is_blank_line())
            {
                next += 1;
            }
            if next < end {
                // A blank line ends the statement after all.
                let line_break = self.input.line_break(next);
                joined.kind = if line_break.column() > top {
                    SyntaxKind::LBRACE
                } else {
                    SyntaxKind::SEMICOLON
                };
                joined.trivia = line_break.trivia();
                return joined;
            }
            if line >= self.input.len() {
                #[allow(clippy::cast_possible_truncation)]
                let end = self.input.all_trivia().len() as u32;
                return Joined {
                    limit: line,
                    kind: SyntaxKind::SEMICOLON,
                    trivia: end,
                };
            }
            let (next_line, first_break) = self.line_start_after(line);
            joined = match first_break {
                Some(line_break) => Joined {
                    limit: next_line,
                    kind: if line_break.column() > top {
                        SyntaxKind::LBRACE
                    } else {
                        SyntaxKind::SEMICOLON
                    },
                    trivia: line_break.trivia(),
                },
                #[allow(clippy::cast_possible_truncation)]
                None => Joined {
                    limit: next_line,
                    kind: SyntaxKind::SEMICOLON,
                    trivia: self.input.all_trivia().len() as u32,
                },
            };
            if next_line - self.pos > JOINED_LOOKAHEAD {
                return joined;
            }
            line = next_line;
        }
    }

    /// [`Parser::nth`] over [`Parser::joined`] lookahead.
    #[inline]
    pub fn nth_joined(&self, joined: Joined, offset: usize) -> SyntaxKind {
        let target = self.pos.saturating_add(offset);
        if target < joined.limit {
            self.input.kind(target)
        } else {
            joined.kind
        }
    }

    /// [`Parser::nth_text`] over [`Parser::joined`] lookahead.
    pub fn nth_text_joined(&self, joined: Joined, offset: usize) -> &'src str {
        let target = self.pos.saturating_add(offset);
        if target < joined.limit {
            self.token_text(target)
        } else {
            ""
        }
    }

    /// [`Parser::nth_has_whitespace_before`] over [`Parser::joined`] lookahead.
    pub fn nth_has_whitespace_before_joined(&self, joined: Joined, offset: usize) -> bool {
        let target = self.pos.saturating_add(offset);
        let end = match target.cmp(&joined.limit) {
            std::cmp::Ordering::Less => self.input.trivia_end_index(target),
            std::cmp::Ordering::Equal if joined.kind != SyntaxKind::EOF => joined.trivia as usize,
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

    /// The next line start after the token `line`, with its first line break; EOF has
    /// none when no line break comes before it.
    fn line_start_after(&self, line: usize) -> (usize, Option<LineBreak>) {
        let start = self.input.line_break_range(line + 1).0;
        let index = self.break_within(start, self.indented.unwound);
        if index < self.input.line_break_count() {
            return (
                self.input.line_break_owner(index),
                Some(self.input.line_break(index)),
            );
        }
        (self.input.len(), None)
    }

    /// The first line break from `index` on at bracket depth `depth` or less, or the
    /// number of line breaks if there is none.
    fn break_within(&self, index: u32, depth: u16) -> u32 {
        let count = self.input.line_break_count();
        let near = index.saturating_add(NEAR_BREAKS).min(count);
        (index..near)
            .find(|&i| {
                count_break_work();
                self.input.line_break(i).depth() <= depth
            })
            .unwrap_or_else(|| {
                self.break_search()
                    .depths
                    .first_at_most(near, u32::from(depth))
                    .unwrap_or(count)
            })
    }

    fn break_search(&self) -> &BreakSearch {
        self.indented
            .break_search
            .get_or_init(|| BreakSearch::new(&self.input))
    }

    /// Like [`Parser::nth`], but when the cursor is at a `;` the layout implies and no
    /// other zero-width token follows it, lookahead goes on into the line it ends at
    /// rather than repeating the `;`: what comes next is that line's first tokens.
    pub fn nth_past_virtual(&self, offset: usize) -> SyntaxKind {
        match self.past_virtual(offset) {
            Some(index) => self.input.kind(index),
            None => self.nth(offset),
        }
    }

    /// The text of [`Parser::nth_past_virtual`]'s token.
    pub fn nth_text_past_virtual(&self, offset: usize) -> &'src str {
        match self.past_virtual(offset) {
            Some(index) if index < self.input.len() => self.token_text(index),
            Some(_) => "",
            None => self.nth_text(offset),
        }
    }

    fn past_virtual(&self, offset: usize) -> Option<usize> {
        if offset == 0
            || self.layout != super::Layout::Indented
            || self.pos != self.fence.index
            || self.fence.kind != SyntaxKind::SEMICOLON
        {
            return None;
        }
        let line = self.indented.line;
        let top = *self.indented.columns.last().unwrap_or(&0);
        let closes = self.indented.columns.len() > 1
            && (line.index == self.input.len()
                || (line.next..line.end).any(|i| self.input.line_break(i).column() < top));
        (!closes).then(|| line.index + offset - 1)
    }

    /// Emit the `@` a shorthand stands for, in front of its `=` or `+`.
    pub fn bump_shorthand_at(&mut self) {
        debug_assert!(self.at_shorthand());
        #[allow(clippy::cast_possible_truncation)]
        let trivia = self.input.trivia_end_index(self.pos) as u32;
        self.events.push(Event::Virtual {
            kind: SyntaxKind::AT,
            trivia,
        });
        self.last_virtual = (self.pos, trivia);
    }

    // ── Fences ───────────────────────────────────────────────────────

    pub(super) fn locate_indented(&mut self) -> Fence {
        loop {
            if let Some(sync) = self.recovery_index()
                && sync <= self.indented.line.index
            {
                #[allow(clippy::cast_possible_truncation)]
                let trivia = self.input.trivia_start_index(sync) as u32;
                return Fence {
                    index: sync,
                    kind: SyntaxKind::EOF,
                    trivia,
                };
            }
            if let Some((kind, trivia)) = self.line_virtual() {
                return Fence {
                    index: self.indented.line.index,
                    kind,
                    trivia,
                };
            }
            self.leave_line();
        }
    }

    /// The next zero-width token at the current line start, or `None` once there is none
    /// left and the line's tokens are the input's own.
    fn line_virtual(&mut self) -> Option<(SyntaxKind, u32)> {
        loop {
            let line = self.indented.line;
            let top = *self.indented.columns.last().unwrap_or(&0);
            if line.next < line.end {
                let line_break = self.input.line_break(line.next);
                if self.goes_on_over(&line, line_break) {
                    if line.saw_content {
                        self.relex_continued_line();
                    }
                    self.indented.line.next += 1;
                    continue;
                }
                let column = self.break_column(line_break, &line, top);
                if line.saw_content {
                    let kind = if column > top {
                        SyntaxKind::LBRACE
                    } else {
                        SyntaxKind::SEMICOLON
                    };
                    return Some((kind, line_break.trivia()));
                }
                if self.indented.columns.len() > 1 && top > column {
                    return Some((SyntaxKind::RBRACE, line_break.trivia()));
                }
                self.indented.line.next += 1;
                continue;
            }
            if line.index < self.input.len() {
                return None;
            }
            #[allow(clippy::cast_possible_truncation)]
            let end = self.input.all_trivia().len() as u32;
            return Some(if line.saw_content {
                (SyntaxKind::SEMICOLON, end)
            } else if self.indented.columns.len() > 1 {
                (SyntaxKind::RBRACE, end)
            } else {
                (SyntaxKind::EOF, end)
            });
        }
    }

    /// The indentation a line break counts with. Trailing whitespace at the end of the
    /// input opens no block, however deep: the input ends at column 0.
    fn break_column(&self, line_break: LineBreak, line: &Line, top: u32) -> u32 {
        let trivia_len = self.input.all_trivia().len();
        if line.index == self.input.len()
            && line_break.trivia() as usize + 1 == trivia_len
            && line.next + 1 == line.end
        {
            line_break.column().min(top)
        } else {
            line_break.column()
        }
    }

    pub(super) fn virtual_bumped(&mut self, kind: SyntaxKind) {
        let state = &mut self.indented;
        match kind {
            SyntaxKind::LBRACE => {
                let line_break = self.input.line_break(state.line.next);
                state.columns.push(line_break.column());
                state.line.next += 1;
                state.line.saw_content = false;
                state.header = None;
            }
            SyntaxKind::SEMICOLON => {
                state.line.saw_content = false;
                state.header = None;
            }
            SyntaxKind::RBRACE => {
                if let Some(column) = state.columns.pop() {
                    state.line.popped = Some((state.line.next, column));
                }
            }
            _ => {}
        }
    }

    /// Every line break before the current line start is dealt with: its tokens are next.
    /// Unless one ended the statement above, the line continues that statement.
    fn leave_line(&mut self) {
        let line = self.indented.line;
        debug_assert!(line.index < self.input.len());
        if !line.saw_content || line.index == 0 {
            debug_assert_eq!(
                line.index, self.pos,
                "a line start left ahead of the cursor"
            );
            self.indented.statement_start = line.index;
            if line.index > 0 && line.end > self.input.line_break_range(line.index).0 {
                self.check_indentation(&line);
            }
        }
        self.find_line_start(line.index + 1);
    }

    /// Make the first line start at or after token `from` the current one.
    pub(super) fn find_line_start(&mut self, from: usize) {
        let count = self.input.line_break_count();
        let start = self.input.line_break_range(from).0;
        // The first break no deeper than `unwound`: those before it are inside brackets.
        let index = self.break_within(start, self.indented.unwound);
        if index < count {
            // A closer with no opener in sight closes an unwound bracket too.
            self.indented.unwound = self.input.line_break(index).depth();
            {
                // The token the break comes before; lines are short, so walk to it, unless
                // the break lies far ahead.
                let mut token = from;
                while self.input.line_break_range(token).1 <= index {
                    count_break_work();
                    token += 1;
                    if token - from > NEAR_TOKENS {
                        token = self.input.line_break_owner(index);
                        break;
                    }
                }
                let (next, end) = self.input.line_break_range(token);
                self.indented.line = Line {
                    index: token,
                    next,
                    end,
                    saw_content: token > 0,
                    popped: None,
                    goes_on: GoesOn::No,
                };
                return;
            }
        }
        let len = self.input.len();
        let (_, end) = self.input.line_break_range(len);
        self.indented.line = Line {
            index: len,
            next: end,
            end,
            saw_content: len > 0,
            popped: None,
            goes_on: GoesOn::No,
        };
    }

    fn absorb_line_breaks(&mut self, across_blank: bool) -> bool {
        let line = self.indented.line;
        if !line.saw_content
            || line.index != self.pos
            || line.next >= line.end
            || self.indented.header_ended == Some(self.pos)
        {
            return false;
        }
        if self.recovery_index().is_some_and(|sync| sync <= self.pos) {
            return false;
        }
        self.indented.line.goes_on = if across_blank {
            GoesOn::AcrossBlank
        } else {
            GoesOn::Lines
        };
        let absorbed = self.goes_on_over(&self.indented.line, self.input.line_break(line.next));
        self.fence = self.locate_fence();
        absorbed
    }

    /// Whether the statement goes on over `line_break`, the next to process before `line`,
    /// because the line above it is unfinished: where the grammar said so, or the header
    /// being parsed, whose name or word that requires more goes on across blank lines too.
    /// Otherwise a blank line stops it, except after a comma.
    fn goes_on_over(&self, line: &Line, line_break: LineBreak) -> bool {
        match line.goes_on {
            GoesOn::AcrossBlank => true,
            GoesOn::Lines if !line_break.is_blank_line() => true,
            _ => self.header_goes_on_at_end_of_line(line),
        }
    }

    /// The current line start continues the statement above from its next line break
    /// on: a comment that starts a line there is an ordinary one, which the input, built
    /// before anyone knew, may have read as reaching over the lines beneath it.
    fn relex_continued_line(&mut self) {
        let line = self.indented.line;
        let after_break = self.input.line_break(line.next).trivia() as usize + 1;
        let comment_follows = after_break < self.input.trivia_end_index(line.index)
            && self.input.all_trivia()[after_break].0 != SyntaxKind::WHITESPACE;
        let across_blank =
            line.goes_on == GoesOn::AcrossBlank || self.header_goes_on_at_end_of_line(&line);
        if comment_follows
            && self
                .input
                .relex_continued(self.source, line.next, across_blank)
        {
            self.indented.line.end = self.input.line_break_range(line.index).1;
            self.indented.line_search = None;
            self.indented.break_search = OnceCell::new();
        }
    }

    // ── Headers ──────────────────────────────────────────────────────

    /// Whether the header goes on over the line breaks before `line` because of how the
    /// line above ends. A variable's name is no end word: `through $to` ends with a value.
    fn header_goes_on_at_end_of_line(&self, line: &Line) -> bool {
        self.header_goes_on_before(line.index)
    }

    /// [`Parser::header_goes_on_at_end_of_line`] for the line start at token `index`.
    fn header_goes_on_before(&self, index: usize) -> bool {
        let Some((header, name)) = self.indented.header else {
            return false;
        };
        let last = index - 1;
        (last == name && header.name_continues)
            || (self.input.kind(last) == SyntaxKind::IDENT
                && !self.is_variable_name(last)
                && header.end_words.contains(&self.token_text(last))
                && (!matches!(self.token_text(last), "and" | "or")
                    || self.operand_before(header, last, name)))
    }

    /// Whether an operand ends right before the header token `index`, so that an `and` or
    /// `or` there is an operator, not an operand of its own: `@if $a and` goes on below,
    /// `@if and` and `@if not and` end with their operand.
    fn operand_before(&self, header: &HeaderLines, index: usize, name: usize) -> bool {
        let before = index - 1;
        if before <= name {
            return false;
        }
        match self.input.kind(before) {
            SyntaxKind::IDENT => {
                self.is_variable_name(before)
                    || !header.end_words.contains(&self.token_text(before))
            }
            // A unit: `10%`.
            SyntaxKind::PERCENT
                if self.input.kind(before - 1) == SyntaxKind::NUMBER
                    && self.input.trivia_before(before).is_empty() =>
            {
                true
            }
            kind => !OPERAND_STARTS_AFTER.contains(kind),
        }
    }

    fn is_variable_name(&self, index: usize) -> bool {
        self.input.kind(index.wrapping_sub(1)) == SyntaxKind::DOLLAR
            && self.input.trivia_before(index).is_empty()
    }

    fn token_text(&self, index: usize) -> &'src str {
        &self.source[self.input.range(index)]
    }

    /// A `/*` that does not start its line and is never closed runs to the end of the
    /// input as an `ERROR` token, which the input keeps as its last trivia; the braced
    /// layout's missing `}` reports that, but here nothing else would.
    pub(super) fn report_unterminated_comment(&mut self) {
        if let Some(&(SyntaxKind::ERROR, range)) = self.input.trailing_trivia().last()
            && self.source[range].starts_with("/*")
        {
            self.error_at(range, "unterminated comment");
        }
    }

    // ── Indentation diagnostics ──────────────────────────────────────

    /// Diagnose the indentation of a line start that has just been processed: a dedent
    /// to no open block's column, at every such line; a mix of tabs and spaces, and the
    /// other kind of indentation than the file's first indented line, once per file, as
    /// Dart Sass stops at the first. The blocks still follow the columns, as before.
    fn check_indentation(&mut self, line: &Line) {
        let last = self.input.line_break(line.end - 1);
        let top = *self.indented.columns.last().unwrap_or(&0);
        if let Some((at, column)) = line.popped
            && at == line.end - 1
            && last.column() > top
        {
            let unit = match self.indented.indent_kind {
                IndentKind::Tabs => "tabs",
                _ => "spaces",
            };
            self.error_at(
                self.indentation_range(last),
                format!("inconsistent indentation, expected {column} {unit}"),
            );
        }
        match last.indent_kind() {
            IndentKind::None => {}
            IndentKind::Mixed => {
                if !self.indented.reported_mixed {
                    self.indented.reported_mixed = true;
                    self.error_at(
                        self.indentation_range(last),
                        "tabs and spaces may not be mixed",
                    );
                }
            }
            kind => {
                if self.indented.indent_kind == IndentKind::None {
                    self.indented.indent_kind = kind;
                } else if self.indented.indent_kind != kind && !self.indented.reported_kind {
                    self.indented.reported_kind = true;
                    let message = if kind == IndentKind::Spaces {
                        "expected tabs, was spaces"
                    } else {
                        "expected spaces, was tabs"
                    };
                    self.error_at(self.indentation_range(last), message);
                }
            }
        }
    }

    /// The indentation of the line that `line_break` starts.
    fn indentation_range(&self, line_break: LineBreak) -> TextRange {
        let range = self.input.all_trivia()[line_break.trivia() as usize].1;
        let text = &self.source[range];
        let line_start = text
            .rfind(['\n', '\r', '\x0c'])
            .map_or(0, |offset| offset + 1);
        #[allow(clippy::cast_possible_truncation)]
        let start = range.start() + TextSize::from(line_start as u32);
        TextRange::new(start, range.end())
    }

    // ── Brackets ─────────────────────────────────────────────────────

    pub(super) fn enter_bracket(&mut self) {
        debug_assert!(
            matches!(
                self.input.kind(self.pos),
                SyntaxKind::LPAREN | SyntaxKind::LBRACKET | SyntaxKind::HASH_LBRACE
            ),
            "a bracket scope must start at its opening token",
        );
        let (_, end) = self.input.line_break_range(self.pos);
        let column = if end == 0 {
            0
        } else {
            self.input.line_break(end - 1).column()
        };
        self.indented.brackets.push(Bracket {
            pos: self.pos,
            errors: self.error_messages.len(),
            column,
            sync: None,
        });
    }

    pub(super) fn exit_bracket(&mut self) {
        let Some(bracket) = self.indented.brackets.pop() else {
            debug_assert!(false, "bracket scope exited twice");
            return;
        };
        if self.error_messages.len() == bracket.errors {
            if bracket.sync.is_some() {
                self.fence = self.locate_fence();
            }
            return;
        }
        let mut depth = 1u32;
        for index in bracket.pos + 1..self.pos {
            depth = bracket_depth_after(depth, self.input.kind(index));
            if depth == 0 {
                break;
            }
        }
        if depth > 0 {
            let depth = u16::try_from(depth).unwrap_or(u16::MAX);
            self.indented.unwound = self.indented.unwound.saturating_add(depth);
            // Line breaks after the cursor may count now. Those before it do only if the
            // layout has not yet emitted anything there.
            let from = if self.last_virtual.0 == self.pos {
                self.pos + 1
            } else {
                self.pos
            };
            if from <= self.indented.line.index {
                self.find_line_start(from);
            }
        }
        self.fence = self.locate_fence();
    }

    /// An error is about to be reported at the cursor. If its token starts a line that the
    /// header went on to because the line above left it requiring more, that line does
    /// not start what the header requires: the header ends above it, where the error is
    /// reported, and the line is left to parse as a statement of its own or as the block.
    /// A deeper line right below the header, with no blank line between, is rather the
    /// rest of the header written on a line of its own, as `@mixin` / `  ($a)`, and the
    /// header goes on there as before.
    pub(super) fn end_header_before_error(&mut self) {
        if self.indented.header.is_none()
            || self.pos == 0
            || self.pos >= self.fence.index
            || self.indented.line.index <= self.pos
        {
            return;
        }
        let (next, end) = self.input.line_break_range(self.pos);
        let unwound = self.indented.unwound;
        let Some(last) = (next..end)
            .rev()
            .map(|i| self.input.line_break(i))
            .find(|line_break| line_break.depth() <= unwound)
        else {
            return;
        };
        let top = *self.indented.columns.last().unwrap_or(&0);
        let blank = (next..end).any(|i| self.input.line_break(i).is_blank_line());
        if (last.column() > top && !blank) || !self.header_goes_on_before(self.pos) {
            return;
        }
        self.indented.header = None;
        self.indented.header_ended = Some(self.pos);
        self.find_line_start(self.pos);
        self.fence = self.locate_fence();
    }

    /// An error was just reported: every bracket it is inside of now stops at the next
    /// line no deeper than the one the bracket opened on.
    pub(super) fn error_reported(&mut self) {
        if self.indented.brackets.is_empty() {
            return;
        }
        let mut changed = false;
        for i in 0..self.indented.brackets.len() {
            if self.indented.brackets[i].sync.is_none() {
                let column = self.indented.brackets[i].column;
                self.indented.brackets[i].sync =
                    Some(self.line_at_or_before_column(self.pos, column));
                changed = true;
            }
        }
        if changed {
            self.fence = self.locate_fence();
        }
    }

    /// The bracket just entered, at its `open` token, is one whose contents the grammar
    /// takes as they come, counting only `open` and `close`, and reports nothing inside.
    /// If it is left unclosed (see [`Parser::bracket_left_unclosed`]), it ends as one
    /// with an error inside does, at the next line no deeper than the one it opened on, or
    /// at the end of input: the grammar reports it unclosed there. Braced, it ends at a
    /// brace instead.
    pub fn end_unclosed_bracket(&mut self, open: SyntaxKind, close: SyntaxKind) {
        if self.layout != super::Layout::Indented {
            return;
        }
        let Some(&Bracket {
            pos, column, sync, ..
        }) = self.indented.brackets.last()
        else {
            debug_assert!(false, "no bracket entered");
            return;
        };
        debug_assert_eq!(pos, self.pos, "the bracket is entered at its opening token");
        debug_assert_eq!(self.input.kind(pos), open, "the bracket opens with `open`");
        if sync.is_some() || !self.bracket_left_unclosed(pos, column, open, close) {
            return;
        }
        let sync = self.line_at_or_before_column(pos + 1, column);
        if let Some(bracket) = self.indented.brackets.last_mut() {
            bracket.sync = Some(sync);
        }
        self.fence = self.locate_fence();
    }

    /// Whether the bracket opened at `pos`, on a line indented by `column`, is left
    /// unclosed: the input ends, or a line below heads a block, before its `close`. Such
    /// a line is indented no deeper than `column`, is followed by a deeper one, and does
    /// not end in a comma, which goes on with the line after it whatever its indentation.
    /// Inside a bracket Dart Sass reads line breaks as whitespace, so a bracket that closes
    /// on a later line, `.a:is(.b,` / `.c,` / `.d)`, is valid, but the deeper line below
    /// such a header is its block, which no selector or prelude takes in. Brackets are
    /// matched by kind: `open` nests, any other closer is content. The search gives up
    /// after `UNCLOSED_LOOKAHEAD_LINES` lines with the bracket still open, as unclosed.
    fn bracket_left_unclosed(
        &self,
        pos: usize,
        column: u32,
        open: SyntaxKind,
        close: SyntaxKind,
    ) -> bool {
        let mut depth = 1u32;
        let mut lines = 0;
        // The indentation of the line being read, if it may head a block.
        let mut header: Option<u32> = None;
        for index in pos + 1..self.input.len() {
            let (start, end) = self.input.line_break_range(index);
            if end > start {
                let indent = self.input.line_break(end - 1).column();
                if header.is_some_and(|header| indent > header)
                    && self.input.kind(index - 1) != SyntaxKind::COMMA
                {
                    return true;
                }
                lines += 1;
                if lines > UNCLOSED_LOOKAHEAD_LINES {
                    return true;
                }
                header = (indent <= column).then_some(indent);
            }
            let kind = self.input.kind(index);
            if kind == open {
                depth += 1;
            } else if kind == close {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
        }
        true
    }

    /// The first token at or after `from` that starts a line indented no deeper than
    /// `column`, inside brackets or not; the input's length if there is none. Walks the
    /// lines, not their tokens.
    fn line_at_or_before_column(&mut self, from: usize, column: u32) -> usize {
        if let Some((last_column, last_from, found)) = self.indented.line_search
            && last_column == column
            && (last_from..=found).contains(&from)
        {
            return found;
        }
        let found = self.find_line_at_or_before_column(from, column);
        self.indented.line_search = Some((column, from, found));
        found
    }

    fn find_line_at_or_before_column(&self, from: usize, column: u32) -> usize {
        let count = self.input.line_break_count();
        let mut index = self.input.line_break_range(from).0;
        let mut lines = 0;
        while index < count {
            let token = self.input.line_break_owner(index);
            let (_, end) = self.input.line_break_range(token);
            if self.input.line_break(end - 1).column() <= column {
                return token;
            }
            index = end;
            lines += 1;
            count_break_work();
            if lines == NEAR_BREAKS {
                return self
                    .break_search()
                    .line_columns
                    .first_at_most(index, column)
                    .map_or(self.input.len(), |last| self.input.line_break_owner(last));
            }
        }
        self.input.len()
    }

    fn recovery_index(&self) -> Option<usize> {
        self.indented
            .brackets
            .iter()
            .filter_map(|bracket| bracket.sync)
            .min()
    }
}

#[cfg(test)]
thread_local! {
    /// Line breaks, lines and tokens the searches over line breaks walk on this thread,
    /// and the searches [`BreakSearch`] answers.
    pub(crate) static BREAK_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[inline]
fn count_break_work() {
    #[cfg(test)]
    BREAK_WORK.set(BREAK_WORK.get() + 1);
}

/// How many line breaks, or lines, a search walks before it asks [`BreakSearch`] instead.
const NEAR_BREAKS: u32 = 64;

/// How many tokens [`Parser::find_line_start`] walks to the token a line break comes
/// before, before it looks the token up.
const NEAR_TOKENS: usize = 64;

/// The searches over the input's line breaks that a run of brackets left unclosed would
/// otherwise repeat over the same lines: each answered in `O(log n)`, so that the parse
/// stays near-linear in the input however the brackets nest.
struct BreakSearch {
    /// Each line break's bracket depth.
    depths: MinTree,
    /// Each line's indentation, at the last of the line breaks before its first token;
    /// the other breaks take no part.
    line_columns: MinTree,
}

impl BreakSearch {
    fn new(input: &crate::input::Input) -> Self {
        let count = input.line_break_count();
        let depths = MinTree::new((0..count).map(|i| u32::from(input.line_break(i).depth())));
        let mut columns = vec![u32::MAX; count as usize];
        for token in 0..=input.len() {
            let (start, end) = input.line_break_range(token);
            if end > start {
                columns[end as usize - 1] = input.line_break(end - 1).column();
            }
        }
        Self {
            depths,
            line_columns: MinTree::new(columns.into_iter()),
        }
    }
}

/// A minimum over a fixed sequence of keys, for the first key from an index on that is at
/// most a bound.
struct MinTree {
    /// The number of leaves: a power of two, at least the number of keys.
    leaves: usize,
    /// Node `i` holds the minimum of nodes `2i` and `2i + 1`; the leaves start at `leaves`,
    /// padded with `u32::MAX`.
    nodes: Vec<u32>,
}

impl MinTree {
    fn new(keys: impl ExactSizeIterator<Item = u32>) -> Self {
        let leaves = keys.len().next_power_of_two();
        let mut nodes = vec![u32::MAX; 2 * leaves];
        for (i, key) in keys.enumerate() {
            nodes[leaves + i] = key;
        }
        for i in (1..leaves).rev() {
            nodes[i] = nodes[2 * i].min(nodes[2 * i + 1]);
        }
        Self { leaves, nodes }
    }

    /// The first index from `from` on whose key is at most `bound`.
    fn first_at_most(&self, from: u32, bound: u32) -> Option<u32> {
        count_break_work();
        let from = from as usize;
        if from >= self.leaves {
            return None;
        }
        // Up from the leaf, to the first subtree right of it that holds such a key ...
        let mut i = self.leaves + from;
        while self.nodes[i] > bound {
            while i & 1 == 1 {
                i >>= 1;
            }
            if i == 0 {
                return None;
            }
            i += 1;
        }
        // ... and down it, leftmost first.
        while i < self.leaves {
            i = if self.nodes[2 * i] <= bound {
                2 * i
            } else {
                2 * i + 1
            };
        }
        #[allow(clippy::cast_possible_truncation)]
        Some((i - self.leaves) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::MinTree;

    #[test]
    fn min_tree_finds_the_first_key_at_most_the_bound() {
        let keys: [u32; 7] = [5, 3, 7, 1, 9, 2, 8];
        let tree = MinTree::new(keys.iter().copied());
        for from in 0..=8u32 {
            for bound in 0..10 {
                let expected = (from..7).find(|&i| keys[i as usize] <= bound);
                let actual = tree.first_at_most(from, bound);
                assert_eq!(actual, expected, "from {from}, bound {bound}");
            }
        }
        assert_eq!(MinTree::new(std::iter::empty()).first_at_most(0, 0), None);
    }
}
