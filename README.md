# sass-analyzer

A hand-written recursive descent SCSS parser in Rust, built for IDE tooling.

**62+ MB/s** parse throughput | **99.11%** sass-spec compatibility | **684 tests** | **0 panics** on 668 real-world files

## Why

Existing SCSS tools for VS Code struggle in monorepos: regex-based parsing, no real syntax tree, poor error recovery. sass-analyzer is a foundation for fast, correct IDE support — a lossless CST parser that preserves every byte of the original source, including whitespace and comments.

Includes a full-featured LSP server (`sass-lsp`) and VS Code extension with go-to-definition, completions, hover, rename, and more.

## Design

Follows [rust-analyzer](https://rust-analyzer.github.io/book/contributing/architecture.html)'s architecture:

- **Events-based parser** emits `Enter`/`Token`/`Exit`/`Error` events — no tree allocation during parsing
- **rowan green-red trees** (v0.16) provide lossless, immutable CST with cheap cloning and incremental reparsing
- **Selective token cache** in the bridge layer deduplicates fixed-text tokens (punctuation, operators) via `Arc` sharing — variable-text tokens bypass the cache
- **Pratt parsing** for expressions with context-aware disambiguation (`/` as division vs separator, `min()`/`max()` as Sass vs CSS)
- **Resilient error recovery** — every grammar production has first/follow token sets; parse errors are localized, and correct syntax after an error parses correctly

```
Source text
    │
    ▼
  Lexer ──► Input (token kinds + trivia offsets)
    │
    ▼
  Parser ──► Events (Enter/Token/Exit/Error)
    │
    ▼
  Bridge ──► rowan GreenNode tree + diagnostics
    │
    ▼
  Typed AST wrappers (UseRule, FunctionCall, ...)
```

## Features

**Full SCSS syntax** — selectors, declarations, nested rules, `&` parent selector, interpolation `#{...}` everywhere (selectors, properties, values, strings, `url()`).

**Expressions** — arithmetic, comparison, logical operators, Pratt-parsed with correct precedence. Maps, lists, bracketed lists, function calls with keyword/rest args.

**At-rules** — `@use`/`@forward` (with `as`, `show`/`hide`, `with()`), `@import`, `@mixin`/`@include` (with content blocks), `@function`/`@return`, `@if`/`@else`, `@each`/`@for`/`@while`, `@extend`, `@at-root`, `@media`, `@supports`, `@keyframes`, `@layer`, `@container`, `@property`, `@scope`, CSS at-rules, and generic at-rule fallback.

**Calculations** — `calc()`, `min()`, `max()`, `clamp()` with full CSS calculation context (variables allowed, `/` always division).

**Special functions** — `url()` with unquoted content and interpolation, `element()`, `progid:...()`.

**Module system** — `@use`/`@forward` path resolution, built-in module recognition (`sass:math`, `sass:color`, etc.), `meta.load-css()` dynamic import detection for dependency graphs.

**Incremental reparsing** — on each edit, only the affected subtree is re-parsed and spliced back into the old tree via rowan's structural sharing. Falls back to full reparse when the edit touches braces or spans all children.

## VS Code Extension

Install from the VS Code Marketplace or build from source (see `editors/code/`).

- Real-time diagnostics with error recovery
- Semantic highlighting (variables, functions, mixins, parameters, properties, placeholders)
- Go to definition, find references, rename
- Completions with fuzzy scoring (variables, functions, mixins, CSS properties, built-in modules)
- Hover with doc comments and value previews
- Signature help for functions and `@include`
- Document/workspace symbols
- Document links for `@use`, `@forward`, `@import`

## Performance

Benchmarked on Angular Material (~1.6 MB SCSS, 279 files concatenated) with `mimalloc`:

| Stage | Throughput |
|-------|-----------|
| Lex only | 200+ MB/s |
| Parse + tree build | 62+ MB/s |
| Incremental reparse (single edit) | **110x** faster than full reparse |

Memory profile (per KB of input): ~420 allocations, ~286 green nodes, ~134 green tokens.

## Compatibility

Tested against the [sass-spec](https://github.com/sass/sass-spec) conformance suite:

- **10,865 / 10,963** valid inputs parse without error (99.11%)
- Remaining 98 mismatches are edge cases (plain CSS `@import` conditions, exotic color syntax)
- 2,252 false positives (inputs dart-sass rejects but we accept) — 93% are semantic errors a parser cannot catch

**Real-world corpus** — 0 panics, 0 round-trip failures, 0 parse errors:

| Library | Files |
|---------|-------|
| Angular Material | 279 |
| Primer | 113 |
| Foundation | 106 |
| Bootstrap | 97 |
| Bulma | 73 |

## Usage

### As a library

```rust
use sass_parser::syntax::SyntaxNode;

let source = r#"
$primary: #3498db;
.button {
  color: $primary;
  &:hover { opacity: 0.8; }
}
"#;

let (green, errors) = sass_parser::parse(source);
let tree = SyntaxNode::new_root(green);

// Lossless: every byte preserved
assert_eq!(tree.text().to_string(), source);

// Walk the typed AST
for error in &errors {
    println!("{}..{}: {}", error.1.start(), error.1.end(), error.0);
}
```

### Collecting imports (for dependency graphs)

```rust
use sass_parser::imports::{collect_imports, ImportKind};
use sass_parser::syntax::SyntaxNode;

let source = r#"@use "sass:meta";
@use "colors";
@forward "mixins";
"#;

let (green, _) = sass_parser::parse(source);
let tree = SyntaxNode::new_root(green);

for imp in collect_imports(&tree) {
    match imp.kind {
        ImportKind::Use => println!("@use {:?}", imp.path),
        ImportKind::Forward => println!("@forward {:?}", imp.path),
        ImportKind::Import => println!("@import {:?}", imp.path),
        ImportKind::LoadCss => println!("meta.load-css({:?})", imp.path),
    }
}
```

### CLI

```
cargo install --path crates/sass-cli

sass-cli parse file.scss     # Print syntax tree
sass-cli check src/           # Check directory for errors
sass-cli lex file.scss        # Dump token stream
```

## Building

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Requires Rust 1.85+ (edition 2024).

## Project structure

```
sass-analyzer/
├── crates/
│   ├── sass-parser/          # Core library
│   │   ├── src/
│   │   │   ├── lexer.rs          # Tokenizer
│   │   │   ├── parser.rs         # Parser infrastructure
│   │   │   ├── grammar/          # Recursive descent grammar
│   │   │   │   ├── selectors.rs
│   │   │   │   ├── declarations.rs
│   │   │   │   ├── expressions.rs    # Pratt parser
│   │   │   │   └── at_rules/         # 9 at-rule modules
│   │   │   ├── bridge.rs         # Events → rowan tree
│   │   │   ├── ast/              # Typed AST wrappers
│   │   │   ├── imports.rs        # Dependency extraction
│   │   │   ├── resolver.rs       # Module path resolution
│   │   │   ├── syntax_kind.rs    # 129 token/node kinds
│   │   │   └── token_set.rs      # [u64; 4] bit set
│   │   ├── tests/            # 554 tests (expect-test snapshots)
│   │   ├── benches/          # divan benchmarks
│   │   └── fuzz/             # 4 libfuzzer targets
│   ├── sass-lsp/             # LSP server (tower-lsp-server)
│   ├── sass-cli/             # Command-line tool
│   └── xtask/                # Codegen from sass.ungram
├── editors/
│   └── code/                 # VS Code extension (TypeScript)
└── test-corpus/              # Real-world SCSS for validation
```

## Key invariants

1. **Lossless round-trip** — `tree.text() == input` for every parse, always
2. **Parser isolation** — the parser depends only on `SyntaxKind` + `TokenSet`, never on rowan
3. **Error locality** — a single syntax error produces at most 3 diagnostics and a small `ERROR` node; surrounding correct syntax is unaffected
4. **Recursion safety** — depth limit (256) enforced via RAII guard at all recursive entry points

## License

MIT
