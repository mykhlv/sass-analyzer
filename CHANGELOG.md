# Changelog

All notable changes to sass-analyzer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Fixed
- LSP: an open document's state, its diagnostics and the cross-file index now always come
  from the same version of its text. A parse of an older version than the one stored no
  longer updates the cross-file index; after a settings change, the re-parse of an edit
  still waiting to be parsed publishes its diagnostics under its own version, not the
  previous one's, and is no longer dropped when the edit was parsed meanwhile; a file
  watcher event for a file opened meanwhile no longer replaces the editor's text with the
  disk's; and a tree re-parsed after being evicted from the cache is no longer stored over
  a newer version of the file

## 0.4.0 — 2026-11-08

### Fixed
- The extension installs on VS Code 1.82 and newer again, including editors built on an older
  VS Code such as openvscode-server. 0.3.4 had raised the minimum to 1.110 without needing any
  newer API.
- Indented Sass: a blank line made of lone `\r` or form-feed line breaks, or of `\r\n`
  followed by a lone `\r`, ends a statement as one made of `\n` does. Only `\n` was
  counted, so in a file with old Mac line endings an operator at the end of a line took
  the rule after a blank line into its value
- LSP: a lone `\r` ends a line in positions, as the protocol defines, so diagnostics,
  folding and every other range land on the right line in a file with old Mac line
  endings; everything was reported on the first line
- Indented Sass: a header goes on across blank lines where it requires more, as in Dart
  Sass: a name after `@mixin`, `=`, `@function`, `@include` or `@extend` on a later line,
  a condition after `@if`, and an argument after a word such as `in` or `as`. Each of
  these reported an error when a blank line came first. A line that does not start what
  the header requires, as a rule's `.z` below `@mixin` or `@use "x" as`, is still left a
  rule of its own, and the error is reported at the end of the header. An operator's
  operand may also follow it after a blank line, as in `$a: 1 +` / (blank) / `2`, when
  that line starts like an expression; before a line that does not, as a rule's `.b` or a
  declaration's `b: c`, the operand is still reported missing and the line below is left
  alone
- Indented Sass: a declaration value ending in `and`, `or` or `not` before a line that is
  a declaration of its own, as `content: not` / `b: c`, blank lines between or not,
  reports the operator's missing operand, as Dart Sass reports the form. It was accepted
  with no diagnostic. `and` or `or` with nothing before it, as in `content: and`, is
  still a plain word
- Indented Sass: a nested property below a value that ends in a comma, as in `font: bold,`
  / `family: x`, is read as one, as in Dart Sass; it was silently taken into the value. A
  variable's value may follow its colon after a blank line, as in `$a:` / (blank) / `b`,
  which reported four errors
- Parser: an unterminated quoted string is now reported as "unterminated string" and ends
  at the end of its line, as in CSS and Dart Sass, in both SCSS and indented Sass. It used
  to swallow the rest of the file without a diagnostic of its own, hiding every symbol and
  error below a single stray quote. An escaped newline still continues a string. Inside a
  map, list or argument list spanning lines, or after a comma the string took, the next
  line is read as the next item, since the string most likely took the comma with it,
  unless that line starts a statement no deeper than the line that opened the bracket, as
  `c: d;` below `b: f(` / `"x`. Otherwise the statement ends at the line break too, as in
  `b: "x;`, `--b: f("x);`, `@include m("x);` or an unknown at-rule's `@foo "x;`, with no
  further diagnostics for the `;` or `)` the string took: the next line parses as a
  statement of its own, so a declaration there keeps its symbol, hover and diagnostics. In
  SCSS, a string that took the `}` closing its block, as in `.a { b: "x }`, or the `{`
  opening a body, as in `@include m("x) {`, has that brace put back where the indentation
  of the next line agrees, so the rules after it keep their nesting. An unclosed `@use` or
  `@import` path still resolves to its module
- Parser: nested selector lists containing an attribute selector or a pseudo-class with
  arguments are no longer misparsed as nested properties — `p:nth-child(2) { }` inside a
  rule was silently turned into a property, and `button:hover, [type="b"] { }` produced a
  false "expected RBRACKET" error
- Parser: a selector with no `{` after it is reported as "expected `{`" — `.a` at the end
  of a file, or a nested `.b` followed by `color: red;`, was silently accepted as a rule
  with no block. The same applies to a keyframe stop such as `50%` with no block. A
  pseudo-class or attribute selector left unclosed, as in `.a:not(.b`, is now reported
  too, where it previously swallowed every rule after it up to the end of the file. In
  SCSS it ends at the next `{` or `}`. In indented Sass, where line breaks inside a
  bracket are whitespace, a bracket still open at the end of the file, or at a line
  below indented no deeper than its own that heads a block, ends at the first such line;
  one taken by a closer of another kind, as in `.a:not(.b]`, ends with its line. A
  bracket closed on a later line, as in `.a:is(.b,` / `.c,` / `.d)`, is read as before
- Parser: a `(` left unclosed in the prelude of an unknown at-rule, as in `@foo (a`, in
  the parameters of a CSS `@function`, or in the condition of `@media`, `@supports`,
  `@container` or `@scope`, as in `@media (a`, is reported as "expected RPAREN" and ends
  at the next `{` or `}`, or in indented Sass as an unclosed selector bracket does. It
  used to swallow every rule after it into the prelude without a diagnostic, even when a
  `)` further down the file seemed to close it. Only `@supports` may hold braces inside
  its parentheses, as Dart Sass allows: they stay part of the condition when the
  parentheses close and every bracket inside them closes in kind, as in
  `@supports (a {b})` or `@supports (a {b} or (c))`. In `@media`, `@container` and
  `@scope`, as in `@media (a {b})`, the first `{` is reported as "expected RPAREN", as in
  Dart Sass. In `@-moz-document` and `@document`, an unquoted `url-prefix(…)`
  argument, and `domain(…)` in `@-moz-document`, is read as a URL like `url(…)`, so the
  `//` in `url-prefix(http://…)` no longer starts a comment that hid the `)` and the rest
  of its line
- Parser: in SCSS, a `@media` query and a `@supports` condition are read as Dart Sass
  reads them, and end where Dart's do: a media query is conditions in parentheses
  joined by `and` or `or`, `not` and a condition, or a media type, after an optional
  modifier, followed by `and` and conditions, with queries separated by commas; a
  `@supports` condition is `not` and a condition, or conditions joined by one of `and`
  and `or`, each in parentheses, an interpolation or a function such as `selector(…)`.
  Names may be interpolated as in Dart, `#{$a}-#{$b}` or `-#{$x}-b(c)`, and a media
  type's modifier may stand on the line above it, `@media screen` / `div {` being the
  query `screen div`. Where no block follows, one "expected `{`" is reported, unless the
  query reported an error of its own: tokens left on the query's line are skipped up to
  a block, which is then the rule's, as in `@media screen and (a) "x" {`, and a statement
  on the next line keeps its own place, as in `@supports (a)` / `@include m;` or
  `@media screen` / `$v: 1;`. A string left unclosed on the query's line that took the
  block's `{`, as in `@media screen and (a) "x {`, is reported only as that. Rules of
  this kind used to be read into the query without a
  diagnostic, and `@supports (a)` at the end of a file or `.p { @media (a) }` was
  silently accepted. `@container` and `@scope` may go without a block, as in Dart Sass,
  and indented Sass is unchanged
- Parser: a nested `name:value` statement holding something only a selector can, as in
  `div:hover .x` or `a:not(.b)`, is read as a rule missing its `{`, as Dart Sass reads it.
  The first was silently accepted as a declaration, and the second reported two
  unrelated expression errors, or one in indented Sass, where such a line is a selector
  with no body and is legal. The `.name: value`, `#name: value` and `:name: value`
  property hacks parse as declarations, like `*name: value`
- Parser: indented Sass at-rule headers continue correctly after arguments again —
  `@for $i` followed by an indented `from 1 through 10`, `@each $a` followed by `in b, c`,
  and `@function a` followed by `()` regressed in 0.3.4 (indented Sass compliance
  96.82% → 99.71%). Whether an indented line continues the header now depends on the
  directive, so `from`/`to`/`0%` under an indented `@keyframes` open the body as keyframe
  selectors, and a body starting with `:hover` no longer escapes its `@mixin` — the latter
  previously reparented the mixin's contents into the enclosing scope with no diagnostic,
  which misled go-to-definition and semantic diagnostics
- Parser: a calculation nested past the parser's depth limit (about 250 levels of unclosed
  `calc(`, `min(` or `(`) no longer hangs the parser until it runs out of memory; it
  reports "nesting too deep" and goes on. A 1 KB file could exhaust tens of gigabytes
- LSP: indented `.sass` files are no longer re-parsed with the SCSS parser after the tree
  cache evicts them, which silently produced a file with no declarations
- LSP: find-references and rename no longer re-parse every file in the workspace, which
  also thrashed the tree cache on large projects
- LSP: code actions write indented syntax into indented `.sass` files. "Extract to mixin"
  inserted a braced `@mixin new-mixin { … }`, which neither our parser nor Dart Sass
  accepts there; it now writes an indented mixin body using the file's own indentation
  unit, keeps nested and continuation lines at their relative depth, and uses the `=`/`+`
  shorthands when the file already does. "Extract to variable" and the auto-import quick
  fix no longer end the inserted `$new-variable: …` and `@use "…"` with a semicolon
- LSP: folding in indented `.sass` files now hides a block's last body line — it was kept
  visible as if it held a closing `}`, so a block with a single body line could not be
  folded at all. The same applies to a `/*` comment that ends at a dedent
- LSP: "Extract to mixin" in `.scss` files no longer deletes what sits between the
  selected declarations. The whole selected range was replaced by `@include new-mixin;`
  but only the declarations were copied into the mixin, so a nested rule, `@include`,
  `@media`, `@extend`, nested property or comment in between was silently lost. Every
  selected statement now moves into the mixin with its nesting intact; a selection that
  cuts through a statement or crosses a block's `}` offers no action. The new mixin is
  also no longer declared where Dart Sass rejects it — inside another mixin, a control
  directive or an `@include` content block — and the action is not offered in a
  `@function`, where `@include` is not allowed
- LSP: releasing the resolver and module-graph locks before recursing removes a potential
  deadlock between indexing and a concurrent configuration change
- LSP: `${workspaceFolder}` in `loadPaths`, `importAliases` and `prependImports` is now
  resolved when settings change, not only at startup
- LSP: secondary folders of a multi-root workspace stay readable after a settings change
- LSP: changing `loadPaths` re-resolves imports in open files without requiring an edit
- LSP: a stale parse can no longer overwrite a newer document state
- LSP: degenerate configuration values (`maxFileSize: 0`, `maxCachedTrees: 0`) are clamped
- Indented Sass: `@use "x" as *` no longer swallows the following line. It parsed only
  when a blank line happened to follow, so the failure looked intermittent. Found by
  testing against hundreds of real-world `.sass` files from Bulma, Vuetify and Quasar —
  the indented syntax previously had no real-world test coverage at all
- Parser: `@layer` accepts an interpolated name — `@layer vuetify-#{$name}`, `@layer #{$n}`,
  and interpolation combined with the dot-separated and comma-separated forms. Only a bare
  identifier was accepted, so any interpolated layer name reported "expected SEMICOLON" at
  the `#{`
- Indented Sass: an identifier that happens to match a keyword the lexer uses for line
  continuation no longer glues the next line onto the current statement. The check fired
  on the bare token text, with no regard for the directive or for whether a directive was
  involved at all, so `margin: 1in` (the IDENT `in`), `content: from`, and any at-rule
  ending in a variable like `$from` or `$to` all swallowed the line below them. This was
  usually **silent** — `margin: 1in` followed by `color: red` produced one declaration
  instead of two, with no diagnostic — and became visible only when the merged result
  could not parse, as in `@if $from` leaving its `@else` without a matching `@if`. These
  keywords now continue only where they can legitimately occur: in the header of a
  directive that takes them or, for the boolean operators `and`/`or`/`not`, inside a value
  as well
- Indented Sass: `@keyframes` frames written as `from`/`to` now open a body of their own.
  They previously produced a flat tree in which each frame's declarations were siblings of
  the frame selector rather than its children, so a `from` frame's properties appeared to
  belong to the `@keyframes` rule itself. Percentage frames (`0%`, `100%`) were already
  correct, which is why the two forms disagreed. Same root cause as the entry above
- Indented Sass: a bare `@else` no longer swallows a variable declaration indented under
  it. `@else` followed by `$x: …` pulled the first declaration into its header — silently,
  since a header can hold an expression — and then errored on the second, so a two-line
  block reported a cascade that pointed at the wrong line. A bare `@else` is a complete
  header, taking an optional `if …` and nothing else, so an indented `$` under it opens
  the body. The other expression directives (`@if`, `@for`, `@return` …) are unaffected —
  they are genuinely incomplete after their bare name
- Indented Sass: a bare `@at-root` now opens a body instead of pulling the next line into
  its header. `@at-root` followed by an indented `@include` reported "expected selector",
  because the header was continued to give the invalid `@at-root @include …`. This is
  Vuetify's density-variant pattern and was the second-largest source of failures in the
  real-world indented-Sass corpus: 26 errors across 19 files. Followed by an indented
  *selector* it produced no error but the wrong shape — the selector form `@at-root .bar`
  rather than a block containing `.bar`, which compiles to the same CSS and so went
  unnoticed; it is now the block form the indented syntax calls for
- Indented Sass: a selector ending in a universal selector — `> *`, `& ~ *`, `.a *` — no
  longer swallows the block indented under it. `*` at the end of a line was always read as
  a multiplication operator, so the whole nested block was parsed as part of the selector
  and produced a cascade of errors. It is now treated as an operator only inside a value
  or an expression-taking directive header. This was the largest source of failures in the
  real-world indented-Sass corpus: 63 errors across 12 files, in Vuetify and Quasar
- Indented Sass: an at-rule whose indented body holds only comments — a `@mixin`,
  `@include`, `@media` or `@if` with a `//` or `/* */` line under it and nothing else — no
  longer swallows the statements that follow it. The body stayed open past the dedent, so
  the next rule and everything after it were **silently** moved inside the at-rule, and the
  rule itself lost its block. It surfaced only as a misleading "`@else` without preceding
  `@if`" when an `@else` followed. A comment line between a header and its indented
  continuation (`@for $i`, then a comment, then `from 1 through 3`) is still skipped, as
  Dart Sass does
- Indented Sass: nested properties written as `font:` with `family: fantasy` and
  `size: 30em` indented under it now parse as a nested property. A colon at the end of a
  line always glued the next line on, so the block **silently** became a rule set with the
  selector `font:family:fantasy`, and only the last child stayed a declaration. As in Dart
  Sass, a colon at the end of a line now continues only a variable declaration
  (`$a:` or `ns.$a:` with the value on the next line); after a property name it opens the
  nested block. The same change stops `font:` with no value from swallowing the
  declaration on the line below it
- Parser: a nested property with a value and children is no longer **silently** read as a
  rule set when the value is an identifier or a function call — `font: bold { family: y; }`,
  `background: url("x.png") { repeat: no-repeat; }` and
  `color: rgba(0, 0, 0, .5) { fallback: red; }` all produced a selector such as
  `font:bold`. As in Dart Sass, a colon followed by a space, or by anything other than an
  identifier, now always starts a declaration; only `name:ident` with nothing in between
  (`a:hover`, `font:bold`) can be a selector. This resolves the known issue recorded for
  the nested-selector fix above, and a custom property whose value contains braces
  (`--c: a {b}`) is now a custom property rather than a rule set
- Indented Sass: a selector that combines a pseudo-class with a trailing universal selector
  no longer swallows the block indented under it — `.a:hover > *`, `.a:not(.b) *`, and
  `.a:focus,` with `.b *` on the next line of the same selector list. The pseudo-class
  colon was taken for the start of a value, so the trailing `*` read as multiplication and
  the declaration below was **silently** absorbed into the selector as a pseudo-class
  (`color: red` became `:red`). `.a:hover .not` absorbed its block the same way. As in Dart
  Sass, a colon followed directly by an identifier (`a:hover`, `a:-webkit-x`, `a:#{$p}`) is
  now a selector colon. A value written without a space after the colon, such as
  `color:red *` with its operand on the next line, consequently no longer continues — Dart
  Sass reads that line as a selector as well
- Indented Sass: a loud comment that starts a line now ends where Dart Sass ends it — at
  the first following line indented no deeper than the comment — and needs no closing
  `*/`, as the Sass documentation shows. An unclosed `/* …` ran to the end of the file, so
  every rule after it was **silently** swallowed into the comment: sass-spec's `loud.hrx`
  produced 2 comments instead of 8. A `*/` on a line not indented under its comment now
  reports an error, as in Dart Sass
- Parser: a `//` comment ends at any line break, `\r` and form feed included, as in CSS and
  Dart Sass. In a file with lone `\r` line endings it ran to the end of the file, in both
  syntaxes. In indented Sass, a `//` comment also continues over every indented line
  separated by `\r\n` line breaks, where it stopped after the first
- Indented Sass: a quote or an unclosed `/*` on a line that continues a `//` comment no
  longer swallows the rest of the file, and an unclosed `url(` there no longer breaks the
  lines below. The continuation line was lexed as code and only then marked as comment, so
  `it's` opened a string that ran to the next `'` anywhere below; as in Dart Sass, such a
  line now ends where the line ends
- Indented Sass: an unclosed bracket no longer turns off line handling for the rest of the
  file. After `b: fn(1` every later line break was read as plain whitespace, so all the
  rules below joined that one declaration and lost their own declarations, symbols and
  folding. The missing `)` is now reported once, and parsing resumes at the next line
  indented no deeper than the one the bracket opened on
- Indented Sass: inconsistent indentation is now reported, as Dart Sass reports it.
  A line indented with spaces in a file indented with tabs (or the reverse), and a line
  that mixes tabs and spaces in its indentation, were accepted silently; a dedent to a
  column that no enclosing block uses closed the deeper blocks without a word. Each
  now gets one diagnostic: "expected tabs, was spaces", "tabs and spaces may not be
  mixed", "inconsistent indentation, expected 4 spaces"
- Indented Sass: whitespace indented deeper than the last line at the very end of a file
  no longer changes the last statement. A trailing `    ` after `b: c` opened an empty
  block, **silently** turning the declaration into a nested property
- Indented Sass: a `//` comment continues across a blank line onto the more deeply
  indented lines after it, as in Dart Sass. It stopped at the blank line, so the next
  indented line was **silently** parsed as a rule of its own
- Indented Sass: an unclosed `/*` after other content on its line (`.a /* x`) is now
  reported as an unterminated comment. It was **silently** taken as a comment running to
  the end of the file, swallowing every rule below it
- Indented Sass: a variable named like a header keyword — `$to`, `$in`, `$and`, `$if` —
  at the end of an `@for`, `@each`, `@if` or `@return` line no longer pulls the line below
  into the header. `@for $i from 1 through $to` **silently** took its indented body into
  the header, leaving the loop with no body. The keyword fix above missed these headers,
  which take the bare word themselves
- Indented Sass: an at-rule line ending in `*`, such as `@x:*`, ends at its line break.
  The `*` read as multiplication, so the rule on the next line was **silently** taken into
  the at-rule's prelude
- Indented Sass: a declaration value ending in a bare `and`, `or` or `not`
  (`content: not`) no longer swallows the declaration on the line below. The two were
  **silently** merged into one; they are now separate declarations, and an indented line
  under `b: c and` is a nested declaration, as it would be without the `and`

### Documentation
- Corrected the 0.3.3 indented-Sass compliance figure: it reported 100% (346/346), the
  measured value against the pinned sass-spec corpus was 345/346

### Changed
- Indented Sass: where a statement ends and a block opens is now decided by the parser from
  the line layout, rather than guessed by the lexer. Real-world files parse to the same
  trees as before; the fixes above are the deliberate differences
- Indented Sass: a statement goes on across a line break only where Dart Sass reads it
  on — after an operator, inside brackets, and where the statement cannot end, as after
  `@mixin`, `@for $i`, `@for $i from 1`, `@each $a`, `@function f`, `@use "a" as` or
  `$a`. A line indented beneath a header that may end there is the rule's block, and a
  comma at the end of a line ends the value or the at-rule's prelude there, as a trailing
  comma. These were accepted as continued headers and are now the one error Dart Sass
  reports: `@include foo` / `($x)`, `@use "x"` / `as y`, `@media screen and` /
  `(min-width: 1px)`, `@media screen,` / `print`, `@container card` / `(width > 400px)`,
  `@scope (.a) to` / `(.b)`, `@at-root` / `(without: media)`, `@else` / `if $b`, `@if $x`
  / `and $y`, `@return 1` / `(2)`, `@extend .a,` / `.b`, `@import "a",` / `"b"`,
  `$a: 1,` / `2`, `--x: a,` / `b` and `b: c,` / `d` indented deeper. Where Dart Sass
  accepts the form but reads the next line as a statement of its own, so does the parser:
  `b: c,` / `d` at the same column, `@layer a,` / `b` and `@foo a,` / `b` make `b` a rule.
  Lines indented beneath a statement that takes no block, such as a variable, `@use` or
  `@return`, are one "nothing may be indented beneath …" error, and a line that starts no
  statement, or is left over after one, is one error rather than one per token
- Indented Sass: a nested `name:value` with no space after the colon, as `color:red`, is
  read as a selector, as Dart Sass reads it — `a:hover` and `color:red` look the same to
  it. It was read as a declaration. Where nothing but a declaration may stand, in a
  nested property's block, and for a custom property such as `--x:red`, it still is,
  and a colon followed by anything that does not start a name, as in `margin:0` or
  `color:#fff`, still makes a declaration. Since the empty rule compiles to nothing, the
  language server warns that a space is likely missing after the `:`
- Dependencies refreshed: 47 transitive Rust crates within semver, `rowan` 0.16 → 0.17,
  TypeScript 6 → 7, `@types/node` 25 → 26, esbuild 0.27 → 0.28, and `tokio` to 1.54 for
  RUSTSEC-2026-0386
- `@types/vscode` is now pinned with a tilde (`~1.82.0`) so it tracks the supported
  `engines.vscode` floor instead of drifting above it on every dependency update

### Known issues
- In indented Sass, text indented under a closed `/* … */` is accepted without the error
  Dart Sass reports. It does not occur in the Bulma, Vuetify and Quasar corpora

## 0.3.4 — 2026-03-31

### Fixed
- Indented Sass: at-rule body detection after params — `@function foo($bg)\n  $x: 1` now correctly starts the body block instead of continuing the header

### Changed
- Upgrade TypeScript to 6.0, update tsconfig (module: nodenext)
- Update VS Code extension devDependencies (@types/vscode, @types/node, esbuild)
- Bump minimum VS Code engine to 1.80

## 0.3.3 — 2026-03-21

### Fixed
- Parser: indented Sass (.sass) compliance improved from 97.11% to 99.71% (345/346 valid inputs):
  - Unterminated block comments (`/* ...` without `*/`) now valid in indented Sass
  - Unquoted `@import` paths (`@import other.css`) for legacy Sass syntax
  - Standalone `//` comments extend to subsequent indented continuation lines
  - Silent comment continuation breaks on blank lines
  - Functional selectors with escaped colon (`\:nth-child(2n+1)`)
  - Sole combinator tokens (`*`, `~`, `+`, `>`) on their own line treated as selectors, not operators
- Parser: sass-spec compliance improved from 99.78% to 99.98% (10,961/10,963 valid inputs). Remaining 2 are pathological nesting tests (also TODO in Dart Sass). Fixed 22 edge cases:
  - Space-separated map keys (`(bold 20px: value)`) — reworked `paren_or_map()` fallback
  - CSS `@function` bodies parsed with raw CSS values (not Sass expressions)
  - Interpolated function names (`foo#{$bar}(arg)`, `#{$name}(arg)`) parsed as calls
  - Non-ASCII identifiers per CSS Syntax Level 3 (`U+00B7`, `U+00C0–U+FFFF`)
  - `url()` with nested parentheses (`url(u())`)
  - Interpolated at-rule names with keyframe blocks (`@#{anim} { from { } }`)
  - Extra tokens after keyframes selector (`from foo { }`)
  - Escape normalization edge cases
  - `calc(min(1 2 3...))` / `calc(max(1 2 3...))` — spread operator now detected as Sass signal
  - Consecutive interpolations as map keys (`#{re}#{d}: value`)
  - `@extend` with interpolation no longer emits false "descendant combinator" error
  - `@include ... using` keyword is now case-insensitive (e.g. `UsInG`)
  - IE star-hack properties (`*width: expression(...)`)
  - Declarations inside `@keyframes` blocks (`blah: blee;`)
  - Extra tokens between `@keyframes` name and opening brace
  - `!important` with trailing content in declaration values (`foo bar !important hux`)
  - Bare `/` as expression atom (`(1, / 2)`, `$path: /images`)

## 0.3.2 — 2026-03-17

### Fixed
- Diagnostics no longer persist after accepting a completion (e.g. `ns.$variable` inside mixin arguments)
- Namespace completions (`ns.`) now insert only the member name instead of duplicating the prefix
- `@use`/`@forward` path completions now navigate into subdirectories and resolve import aliases (e.g. `@sass/sub/`)

### Added
- Completion trigger on `'`, `/` for better `@use`/`@forward` path suggestions
- `.sass` files now appear in `@use`/`@forward` path completions

## 0.3.1 — 2026-03-15

### Fixed
- LSP: first Cmd+Click / hover after opening a file no longer fails (eagerly parse on `didOpen` instead of waiting for debounced worker)

## 0.3.0 — 2026-03-15

### Added
- Indented Sass syntax (`.sass`) support: parse `.sass` files using whitespace-significant syntax (no braces/semicolons). All LSP features (hover, completions, go-to-definition, rename, references, diagnostics, etc.) work with `.sass` files. Includes CLI support (`sass-cli parse/check`) and VS Code extension activation for the `sass` language
- Open VSX Registry publishing in release workflow (available in Cursor, Windsurf, VSCodium, Gitpod)

### Changed
- MSRV updated from 1.85 to 1.94

### Fixed
- Parser: sass-spec compatibility improved from 99.13% to 99.78% (95 → 24 false negatives). Fixes include: trailing commas in selectors, `@elseif` (deprecated no-space form), `@extend` with comma-separated selectors, `@at-root` query with quoted strings, keyframe flexibility (interpolated selectors, variable declarations, nested at-rules, anonymous names), interpolated at-rule names, CSS `@function` declarations, `@import` with media query commas, calc with interpolation/variables/space-separated values, `alpha()`/`progid:`/`expression()` special functions, `#{}` in ID selectors and placeholders, `-#{}` in property names and selectors, `!important` in mixin arguments, `@each` with space-separated lists
- README inaccuracies corrected

## 0.2.0 — 2026-03-14

### Added
- SassDoc support: `/// @param`, `/// @return`, `/// @example`, `/// @deprecated`, `/// @type`, `/// @see`, `/// @output`, `/// @content`, `/// @throw` annotations are parsed and rendered as structured markdown in hover, completions, and signature help. Parameter descriptions appear inline in signature help popups
- Call hierarchy: navigate incoming/outgoing calls for functions and mixins — "who calls this?" and "what does this call?" (right-click → Show Call Hierarchy). Supports same-file and cross-file resolution, groups callers by enclosing function/mixin
- Inlay hints: parameter name hints for positional arguments in function calls and `@include` directives (e.g., `add(⁣$a:⁣ 1px, ⁣$b:⁣ 2px)`). Skips keyword arguments, single-parameter calls, and rest parameters
- CSS value completions: after typing `display: `, offers `flex`, `grid`, `block`, `none`, etc. Covers ~80 CSS properties with keyword values plus global keywords (`inherit`, `initial`, `unset`, `revert`, `revert-layer`). Sass variables and functions also appear in value position. AST-based context refinement correctly handles map entries and multi-line values
- Code actions (quick fixes): auto-import `@use` for undefined variables, functions, and mixins — inserts the `@use` statement and qualifies the reference with the namespace. Remove unused `@use` statements
- Code actions (refactoring): extract selection to variable (`$new-variable`), extract declarations to mixin (`@mixin new-mixin`)
- Semantic diagnostics: wrong argument count (ERROR) for functions and mixins, undefined variable/function/mixin warnings (WARNING) with false-positive suppression for `@import` files and CSS global functions
- Diagnostic cascade: when a dependency file changes, diagnostics are re-published for all open files that import it
- Selection ranges: smart expand/shrink selection (Shift+Alt+→/←) that follows the AST structure — token → node → parent → root
- Document highlights: highlight all occurrences of the symbol under the cursor (variables, functions, mixins, placeholders) with read/write distinction
- Folding ranges: collapsible regions for rule blocks, at-rules, multi-line comments, consecutive `//` comment groups, and `// #region` / `// #endregion` markers
- File watcher support: non-open SCSS/Sass files modified, created, or deleted on disk are now detected and re-indexed automatically
- Check Workspace command: run diagnostics on all SCSS files in the workspace (Command Palette → "sass-analyzer: Check Workspace"), with progress reporting
- Go to definition on `@use`/`@forward`/`@import` paths: Cmd+Click on the import string navigates directly to the target file
- Parser: value-and-block syntax (`margin: 10px { top: 20px; }`) — nested property with both a value and sub-declarations
- `rect()` CSS function recognized as valid (no false "undefined function" diagnostic)

### Removed
- Color provider (decorators and picker) — removed to avoid duplicate color squares with VS Code's built-in CSS color support

## 0.1.1 — 2026-03-12

### Added
- Cross-platform CI (Ubuntu, Windows, macOS) with cargo-deny license/vulnerability auditing
- CONTRIBUTING.md with development setup and coding guidelines
- Crate-level documentation with quick start example
- Lexer error recovery tests and non-ASCII / UTF-16 LSP tests
- `parse_file` example binary for quick syntax tree inspection
- GitHub issue templates (bug report, feature request)
- Dependabot for Cargo, npm, and GitHub Actions dependencies
- `rust-toolchain.toml` pinning stable toolchain
- Linux ARM64 (`aarch64-unknown-linux-gnu`) release target
- `#![warn(missing_docs)]` with doc comments on all public API items in `sass-parser`
- Expanded extension README with feature overview, monorepo example, and tuning settings

### Changed
- Runtime constants (`maxFileSize`, `debounceMs`, `maxCachedTrees`, `maxCachedSources`) are now configurable via `initializationOptions` and `workspace/didChangeConfiguration`
- LRU eviction for source text in module graph to cap memory usage
- Completion handler avoids cloning full document text
- `@use` path completion runs on a blocking thread to avoid stalling the LSP
- Symbols shared via `Arc<FileSymbols>` instead of deep cloning per request
- Dependency indexing capped at 10,000 files per workspace with TOCTOU guard

### Fixed
- Correct UTF-16 column offsets for non-ASCII content in diagnostics, go-to-definition, hover, references, rename, and document symbols
- Windows: extension now finds the bundled `sass-lsp.exe` binary
- `path_to_uri` no longer panics on paths with special characters
- Silent failure when LSP worker channel drops (now logs error)
- Path traversal protection in module resolver (reject `..` escaping workspace roots)
- Files exceeding size limit now logged instead of silently skipped
- Extension now forwards `didChangeConfiguration` to the server (settings changes take effect without restart)
- Extension checks server binary exists before starting, shows actionable error instead of cryptic crash
- Extension awaits `client.start()` so startup errors surface properly
- Defensive string slicing in `@use`/`@forward` path extraction prevents panic on malformed tokens
- `byte_to_lsp_pos` clamps out-of-range offsets to avoid panic on stale parse trees
- `didChangeConfiguration` now rebuilds module resolver, `loadPaths`, `importAliases`, and `prependImports` (previously only numeric settings updated live)
- `merge_errors` clamps negative shifted offsets to zero instead of wrapping to corrupt ranges
- Lexer now handles scientific notation in numbers (`1e3`, `2.5e-2`, `1E+3`)
- Signature help parameter offsets now use UTF-16 code units (fixes highlight for non-ASCII parameter names)
- `path_to_uri` fallback no longer produces `file:////path` (4 slashes) on Unix
- Extension now declares `extensionKind: ["workspace"]` for SSH/WSL/Container remote dev

## 0.1.0 — 2026-03-08

Initial release.

- Real-time diagnostics with error recovery
- Semantic highlighting for variables, functions, mixins, parameters, properties, placeholders
- Go to definition for variables, functions, mixins, placeholders
- Find all references
- Rename symbol (workspace-wide)
- Completions with fuzzy scoring (variables, functions, mixins, CSS properties, built-in modules)
- Hover with doc comments and value previews
- Signature help for functions and mixins
- Document symbols (Outline view)
- Workspace symbols (Ctrl+T)
- Document links for `@use`, `@forward`, `@import`
- Multi-file module graph with `@use` / `@forward` resolution
- Incremental text sync and incremental reparsing
- Import alias and load path configuration for monorepos
- Platform-specific binaries: Linux x64, Windows x64, macOS x64, macOS ARM64
