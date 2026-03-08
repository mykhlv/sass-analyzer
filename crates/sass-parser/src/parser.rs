use crate::event::Event;
use crate::input::Input;
use crate::syntax_kind::SyntaxKind;
use crate::text_range::TextRange;
use crate::token_set::TokenSet;

const MAX_DEPTH: u32 = 256;

pub struct Parser<'src> {
    input: Input,
    source: &'src str,
    pos: usize,
    events: Vec<Event>,
    error_messages: Vec<String>,
    depth: u32,
}

impl<'src> Parser<'src> {
    pub fn new(input: Input, source: &'src str) -> Self {
        Self {
            input,
            source,
            pos: 0,
            events: Vec::new(),
            error_messages: Vec::new(),
            depth: 0,
        }
    }

    pub fn finish(self) -> (Vec<Event>, Vec<String>, Input, &'src str) {
        (self.events, self.error_messages, self.input, self.source)
    }

    // ── Lookahead ────────────────────────────────────────────────────

    #[inline]
    pub fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    #[inline]
    pub fn nth(&self, offset: usize) -> SyntaxKind {
        self.input.kind(self.pos.saturating_add(offset))
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
        self.pos >= self.input.len()
    }

    /// Returns `false` for pos 0 (no preceding significant token).
    pub fn has_whitespace_before(&self) -> bool {
        if self.pos == 0 {
            return false;
        }
        self.input.has_whitespace_before(self.pos)
    }

    /// Returns the source text of the current token.
    pub fn current_text(&self) -> &'src str {
        if self.at_end() {
            return "";
        }
        let range = self.input.range(self.pos);
        &self.source[range]
    }

    /// Returns the source text of the token at `pos + offset`.
    pub fn nth_text(&self, offset: usize) -> &'src str {
        let target = self.pos.saturating_add(offset);
        if target >= self.input.len() {
            return "";
        }
        let range = self.input.range(target);
        &self.source[range]
    }

    /// Whether any whitespace trivia exists before the token at `pos + offset`.
    pub fn nth_has_whitespace_before(&self, offset: usize) -> bool {
        let target = self.pos.saturating_add(offset);
        if target == 0 || target >= self.input.len() {
            return false;
        }
        self.input.has_whitespace_before(target)
    }

    // ── Consuming tokens ─────────────────────────────────────────────

    pub fn bump(&mut self) {
        assert!(!self.at_end(), "bump at end of input");
        let kind = self.input.kind(self.pos);
        let range = self.input.range(self.pos);
        self.events.push(Event::Token { kind, range });
        self.pos += 1;
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
        self.error(format!("expected {kind:?}"));
        false
    }

    // ── Markers ──────────────────────────────────────────────────────

    pub fn start(&mut self) -> Marker {
        #[allow(clippy::cast_possible_truncation)]
        let pos = self.events.len() as u32;
        self.events.push(Event::Enter {
            kind: SyntaxKind::ERROR,
            forward_parent: None,
        });
        Marker::new(pos)
    }

    // ── Errors ───────────────────────────────────────────────────────

    pub fn error(&mut self, msg: impl Into<String>) {
        let range = if self.at_end() {
            let end = crate::text_range::TextSize::of(self.source);
            TextRange::empty(end)
        } else {
            self.input.range(self.pos)
        };
        #[allow(clippy::cast_possible_truncation)]
        let msg_index = self.error_messages.len() as u32;
        self.error_messages.push(msg.into());
        self.events.push(Event::Error { msg_index, range });
    }

    pub fn err_and_bump(&mut self, msg: impl Into<String>) {
        let m = self.start();
        self.error(msg);
        self.bump();
        let _ = m.complete(self, SyntaxKind::ERROR);
    }

    /// Skip tokens until one in `recovery` (or EOF) is found.
    /// Skipped tokens are wrapped in a single `ERROR` node.
    /// Returns `true` if any tokens were skipped.
    pub fn err_recover(&mut self, msg: impl Into<String>, recovery: TokenSet) -> bool {
        if self.at_end() || self.at_ts(recovery) {
            self.error(msg);
            return false;
        }
        let m = self.start();
        self.error(msg);
        while !self.at_end() && !self.at_ts(recovery) {
            self.bump();
        }
        let _ = m.complete(self, SyntaxKind::ERROR);
        true
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
