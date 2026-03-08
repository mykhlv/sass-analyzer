use crate::syntax_kind::SyntaxKind;

/// Bit-set of `SyntaxKind` values, limited to tokens only.
///
/// 256-bit capacity via `[u64; 4]`. Fully `const`-constructible.
/// Compile-time assertion rejects node kinds.
#[derive(Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct TokenSet([u64; 4]);

impl TokenSet {
    pub const EMPTY: TokenSet = TokenSet([0; 4]);

    pub const fn new(kinds: &[SyntaxKind]) -> TokenSet {
        let mut bits = [0u64; 4];
        let mut i = 0;
        while i < kinds.len() {
            let kind = kinds[i] as u16;
            const { assert!(SyntaxKind::__LAST_TOKEN as u16 <= 256) };
            assert!(
                kind < SyntaxKind::__LAST_TOKEN as u16,
                "TokenSet can only contain tokens, not nodes",
            );
            let word = kind as usize / 64;
            let bit = kind as usize % 64;
            bits[word] |= 1 << bit;
            i += 1;
        }
        TokenSet(bits)
    }

    pub const fn contains(self, kind: SyntaxKind) -> bool {
        let kind = kind as u16;
        if kind >= SyntaxKind::__LAST_TOKEN as u16 {
            return false;
        }
        let word = kind as usize / 64;
        let bit = kind as usize % 64;
        self.0[word] & (1 << bit) != 0
    }

    pub const fn union(self, other: TokenSet) -> TokenSet {
        TokenSet([
            self.0[0] | other.0[0],
            self.0[1] | other.0[1],
            self.0[2] | other.0[2],
            self.0[3] | other.0[3],
        ])
    }
}

impl std::fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TokenSet").field(&self.0).finish()
    }
}
