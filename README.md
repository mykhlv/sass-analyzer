# sass-analyzer

A hand-written recursive descent SCSS/Sass parser and language server in Rust, built for IDE tooling. Supports both SCSS (`.scss`) and indented Sass (`.sass`) syntax.

## Why

Existing SCSS extensions for VS Code struggle in monorepos: slow startup, high memory usage, incomplete module system support. sass-analyzer is a native Rust alternative — a lossless CST parser and full-featured LSP server that handles large workspaces without breaking a sweat.

## VS Code Extension

- Real-time diagnostics with error recovery
- Semantic highlighting (variables, functions, mixins, parameters, properties, placeholders)
- Go to definition, find references, rename (workspace-wide, cross-file)
- Completions with fuzzy scoring (variables, functions, mixins, CSS properties, built-in modules)
- Hover with SassDoc comments and value previews
- Signature help for functions and `@include`
- Call hierarchy (incoming/outgoing calls for mixins and functions)
- Inlay hints (parameter names in function/mixin calls)
- Code actions (extract variable, extract mixin, auto-import `@use`, remove unused `@use`)
- Document/workspace symbols
- Document links for `@use`, `@forward`, `@import`
- Selection range, document highlight, folding
- File watcher — automatically re-indexes on external file changes

## Performance

Benchmarked on Angular Material (~1.6 MB SCSS, 279 files concatenated):

```
Lex              ██████████████████████████████████████████  200+ MB/s
Parse + tree     ███████████████                             62+ MB/s
Incremental      ⚡ 110x faster than full reparse
```

Benchmarks use [`mimalloc`](https://github.com/microsoft/mimalloc) (a compact, high-performance allocator) because rowan's many small allocations benefit from it. Incremental reparsing via rowan's structural sharing means only the affected subtree is re-parsed on each edit.

## Compatibility

Tested against the [sass-spec](https://github.com/sass/sass-spec) conformance suite:

- **10,939 / 10,963** valid inputs parse without error (**99.78%**)
- Remaining 24 mismatches are edge cases (plain CSS `@import` conditions, exotic color syntax)

**Real-world corpus** — 0 panics, 0 round-trip failures on 668 files:

| Library | Files |
|---------|-------|
| Angular Material | 279 |
| Primer | 113 |
| Foundation | 106 |
| Bootstrap | 97 |
| Bulma | 73 |

## Design

Follows [rust-analyzer](https://rust-analyzer.github.io/book/contributing/architecture.html)'s architecture:

- **Events-based parser** emits `Enter`/`Token`/`Exit`/`Error` events — no tree allocation during parsing
- **rowan green-red trees** (v0.16) provide lossless, immutable CST with cheap cloning and incremental reparsing
- **Selective token cache** in the bridge deduplicates fixed-text tokens via `Arc` sharing
- **Pratt parsing** for expressions with context-aware disambiguation (`/` as division vs separator, `min()`/`max()` as Sass vs CSS)
- **Resilient error recovery** — every grammar production has first/follow token sets; parse errors are localized, and correct syntax after an error parses correctly

```
Source text (.scss)              Source text (.sass)
    │                                │
    ▼                                ▼
  Lexer ──► Input               SassLexer (indent → virtual {/}/;) ──► Input
    │                                │
    └──────────┬─────────────────────┘
               ▼
  Parser ──► Events (Enter/Token/Exit/Error)
               │
               ▼
  Bridge ──► rowan GreenNode tree + diagnostics
               │
               ▼
  Typed AST wrappers (UseRule, FunctionCall, ...)
```

## Parser features

**Full SCSS syntax** — selectors, declarations, nested rules, `&` parent selector, interpolation `#{...}` everywhere (selectors, properties, values, strings, `url()`).

**Indented Sass syntax** (`.sass`) — whitespace-significant syntax without braces or semicolons. A dedicated lexer converts indentation into virtual `{`/`}`/`;` tokens, feeding the same parser — all features work identically for both syntaxes.

**Expressions** — arithmetic, comparison, logical operators, Pratt-parsed with correct precedence. Maps, lists, bracketed lists, function calls with keyword/rest args.

**At-rules** — `@use`/`@forward` (with `as`, `show`/`hide`, `with()`), `@import`, `@mixin`/`@include` (with content blocks), `@function`/`@return`, `@if`/`@else`, `@each`/`@for`/`@while`, `@extend`, `@at-root`, `@media`, `@supports`, `@keyframes`, `@layer`, `@container`, `@property`, `@scope`, CSS at-rules, and generic at-rule fallback.

**Calculations** — `calc()`, `min()`, `max()`, `clamp()` with full CSS calculation context.

**Special functions** — `url()` with unquoted content and interpolation, `element()`, `progid:...()`.

**Module system** — `@use`/`@forward` path resolution, built-in module recognition (`sass:math`, `sass:color`, etc.), `meta.load-css()` dynamic import detection.

**Incremental reparsing** — on each edit, only the affected subtree is re-parsed and spliced back into the old tree via rowan's structural sharing.

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

let (green, errors) = sass_parser::parse_scss(source);
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

let (green, _) = sass_parser::parse_scss(source);
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

sass-cli parse file.scss     # Print syntax tree (SCSS)
sass-cli parse file.sass     # Print syntax tree (indented Sass)
sass-cli check src/           # Check directory for errors (.scss + .sass)
sass-cli lex file.scss        # Dump token stream
```

## Building

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Requires Rust 1.94+ (edition 2024).

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
│   │   │   ├── sass_lexer.rs     # Indented Sass → virtual tokens
│   │   │   ├── bridge.rs         # Events → rowan tree
│   │   │   ├── ast/              # Typed AST wrappers
│   │   │   ├── imports.rs        # Dependency extraction
│   │   │   ├── resolver.rs       # Module path resolution
│   │   │   ├── syntax_kind.rs    # 129 token/node kinds
│   │   │   └── token_set.rs      # [u64; 4] bit set
│   │   ├── tests/            # expect-test snapshots
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
