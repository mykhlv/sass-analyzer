# Changelog

All notable changes to sass-analyzer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.5.0 — 2026-11-12

### Added
- LSP: a `@use`, `@forward` or `@import` of a stylesheet that does not exist, as a
  misspelled path or one that needs a `loadPaths` entry, is reported as an error on its
  URL, with Dart Sass's message "Can't find stylesheet to import.", and so is an unknown
  `sass:` module. Nothing was reported: the only trace was that every undefined-name
  warning in the file went silent, which it still does, so the one cause gives one
  diagnostic. URLs the server does not resolve itself are not reported: `pkg:` and other
  schemes, the `~` prefix of bundlers, and a URL in an installed package that the
  server cannot follow. Neither are plain-CSS `@import`s, which Dart Sass leaves to the
  browser: a URL ending in `.css`, starting with `http://`, `https://` or `//`, written
  as `url(...)`, or followed by a media query or `supports(...)`. A `@use` or `@forward`
  of a `.css` URL loads that file, so a missing one is reported. The
  `sass-analyzer.reportUnresolvedImports` setting turns the report off, for a project
  whose own importer loads what the server cannot

### Fixed
- Parser: a rule or at-rule missing its `{`, where the tokens skipped before the block
  hold a quoted string with an operator in its interpolation, as in `.a ) "#{$b + 1}" {}`
  or `@media (a) x "#{$b + 1}" {}`, no longer crashes the parser.
- LSP: an open document's state, its diagnostics and the cross-file index now always come
  from the same version of its text. A parse of an older version than the one stored no
  longer updates the cross-file index; after a settings change, the re-parse of an edit
  still waiting to be parsed publishes its diagnostics under its own version, not the
  previous one's, and is no longer dropped when the edit was parsed meanwhile; a file
  watcher event for a file opened meanwhile no longer replaces the editor's text with the
  disk's; and a tree re-parsed after being evicted from the cache is no longer stored over
  a newer version of the file
- LSP: imports resolve to the files Dart Sass loads: a `.css` file for a URL with no
  extension, as `@import "normalize.css/normalize"`, and a directory's `index.css`; an
  import-only file such as `_theme.import.scss` for an `@import`, ahead of the plain
  file; and a URL with `%` escapes, as `my%20file`. A bare package name loads the
  stylesheet its `package.json` names in `exports` (the `sass` or `style` condition) or
  in its `sass` or `style` field, as bundlers do. Go to definition and document links
  reached none of these, and their members were not found
- LSP: a `.css` URL, as in `@use "reset.css"`, resolves to its file, or to the partial
  `_reset.css`, so go to definition and document links reach it. As in Dart Sass, the
  extension is matched case-sensitively: `@import "a.CSS"` is not plain CSS, and
  `@use "a.SCSS"` does not load `a.scss`
- LSP: a URL ending in `/`, as `@use "theme/" as t`, loads the directory's `index`
  file, as in Dart Sass; it loaded `theme.scss` beside the directory, and reported
  nothing when only that existed
- LSP: a file that appears, created on disk or opened in the editor, re-checks the open
  files whose imports did not resolve, and deleting a file re-resolves the imports of
  the open files that use it. A new file was not seen by its importers until they were
  edited, and a deleted one was still treated as loaded. A change to a file already
  known re-checks only the files that import it
- LSP: a `.css` file created or deleted on disk re-checks the open files as a new or
  deleted stylesheet does, so creating `theme.css` clears the error of `@use "theme"`
  without editing the importer. A watcher event alone does not index a CSS file as a Sass
  module, whatever the case of its extension: a change to `Foo.CSS` was read as SCSS
- LSP: "Check Workspace" no longer holds a lock on the open documents while it
  publishes their diagnostics, which could hang the server if a file was opened at
  that moment
- LSP: the auto-import, extract-to-variable and extract-to-mixin code actions write
  their lines with the document's own line breaks. In a file with `\r\n` line endings
  they inserted bare `\n` breaks, and auto-import added a stray empty line after an
  existing `@use`
- Indented Sass: extract to mixin is no longer offered for lines whose indentation does
  not continue the first selected line's, as tabs below a line indented with spaces. It
  moved such a line flush with the mixin's body, which could take a nested declaration
  out of its rule
- Indented Sass: the warning that a nested `name:value` rule is likely a declaration
  missing its space after `:` is no longer given for a pseudo selector, as an empty
  `a:hover`, `li:first-child`, `a:not(.b)` or `input:-webkit-autofill`, nor for those of
  CSS Modules and Vue, as `:export` or `div:deep(.x)`, `:active-view-transition`,
  `:heading` or `:xr-overlay`. It is now given for an interpolated name, as
  `margin-#{$side}:auto`, and for a CSS property's name before a pseudo-class's, as
  `cursor:default`, while `input:default` stays silent
- Parser: a mistake inside a bracket — an argument list, a parameter list, the
  configuration of `@use` or `@forward ... with (...)`, a parenthesized list or map, a
  bracketed list or an interpolation — is one error, and what could not be read up to
  the bracket's closer is skipped with it. The statement went on from the inside of the
  bracket instead, which reported up to a dozen errors for one, as for
  `@use "a" with (b.$c: d)` or `f((a, b: c)...)`. A parameter or a configured variable
  missing its `$` no longer takes the ones after it with it
- Parser: an expression nested too deep is one error, "nesting too deep". The rest of
  the operand that went too deep is skipped with it, and the brackets left open around
  it end there without errors of their own: 1000 unclosed `[` gave over 1700 errors
- Parser: `@include m using $a { ... }`, missing the parentheses around `$a`, is one
  error, a missing `(` as in Dart Sass, and the content block still parses as the
  rule's. It was taken for a declaration, with up to nine errors
- Parser: a `@use ... as` or `@forward ... as` clause with no identifier, as
  `@forward "a" as 1-*`, is one error, and the clauses after it still parse. The rest of
  the clause was left to the next statement, with up to three more errors
- Parser: a map entry missing its `:`, as `two` in `(a: 1, two, b: 2)`, is one error,
  not a second one for a value it never had
- Parser: a rule missing its `{` where a string left unclosed starts, as the rest of
  `'line1` / `line2';` read on the line after a line break that ended the string, reports
  the string only, not a missing `{` at the same place as well
- Parser: a `:`, `)` or `]` in a declaration's value, as in `c: d)` or `b: c:d`, is
  reported, as Dart Sass does, and the rest of the value is skipped with it. Such
  values were accepted silently, in both syntaxes. In the braced syntax, `name:` after a
  value is the next declaration, missing the `;` before it: `color: red font-size: 14px`
  reports the `;` and parses both declarations. Colons that values hold stay valid:
  `progid:` filters, `url(a:b)`, strings, escapes, interpolation, custom properties
- Parser: a `.` in a declaration's value that starts no number is reported, as Dart Sass
  does. In the indented syntax, a word operator or `+` at the end of a line took a
  selector line below, `content: not` / `.c`, for its operand, and the lines beneath it
  for a nested property, with no error. A line below that is no operand, as `&:hover`,
  `b: c` or `$b: 2`, now reports the one error Dart Sass gives there
- SCSS: a variable's value that reports an error, as a map missing its commas and
  left unclosed, `$m: (` / `a: 1` / `b: 2` / `.c { ... }`, takes the rest of its line
  with that one error. The rest was left to the next statement, with up to three more
  errors, and the rule on the next line was read into it. The indented syntax does the
  same, `$m: (` / `a: 1` / `b: 2` / `.c`, which reported the line's end missing as well
- Indented Sass: a file with many brackets left unclosed, line after line, parses in time
  linear in its size. Each line searched on to the end of the file for the next line
  start, and each erroring bracket for the next line at its column, so the time grew with
  the square of the file's length
- Parser: an `@page` selector list, as `@page toc, index:blank` or `@page :left, :right`,
  is no longer an error. Only the first selector was read, and the rest was a rule missing
  its selector; a prelude such as `@page a + b` was read as a rule after the at-rule, with
  no error
- Parser: `!important` inside brackets, as in `(a: 1 !important)`, `(1, 2 !important)`,
  `[a !important b]` or `f(a !important b)`, is an item of the value, as in Dart Sass, not
  an error. Only a positional argument ending with it was accepted
- Parser: a function named `URL` or `Url`, which Dart Sass accepts with a deprecation
  warning, is no longer an error: its name was read as the start of `url(...)`, and its
  parameters as the URL. `@function url(...)`, as well as `and`, `or`, `not`,
  `expression` and `element`, is one error on the name, "invalid function name", as in
  Dart Sass, and `@function type(...)` reports that the name is reserved for the
  plain-CSS function. `url(...)` in values is read as before
- Parser: `and` and `or` where an operand starts are identifiers, as in Dart Sass:
  `$a: and;`, `f(and, or)`, `$a: and and and` and `@if not and` are no longer errors.
  After an operand they stay operators, and one missing its right operand, as
  `$a: 1 and;`, is still reported. In the indented syntax, an `@if` or `@while` line ending
  with such an operand, `@if and`, no longer reads the lines beneath it as more of its
  condition
- Parser: a variable's value may end with `!important`, as `$a: 1 !important;` or
  `$a: 1 ! important;`, which Dart Sass accepts; that was three errors. `!important` is
  matched in any case, `!IMPORTANT`. A `!` followed by a space and another word, as
  `$a: 1 ! default` or `b: c ! d`, is one error, "expected `important`", as in Dart Sass;
  it was accepted
- Indented Sass: `b: c !` / blank line / `important` is the declaration's `!important`,
  as in Dart Sass; it read `important` as a rule of its own. A `!` ending a line before a
  line that is not `important`, as `$a: 1 !` / `default`, is one error after the `!`
- LSP: a `.css` file that an import loads, as `@use "theme"` finding `theme.css`, is
  plain CSS, as in Dart Sass, which rejects Sass variables, mixins and functions there.
  It was read as SCSS, so a `$v` defined in it was a member of the module, with a
  definition and a workspace symbol. It now exports no members and adds no symbols; go to
  definition and document links on the import still reach the file
- Parser: `@extend .b ! optional`, or `!` and `optional` on lines of their own, is one
  error after the `!`, as in Dart Sass, which takes the flag only when it is joined to its
  `!`. The braced syntax accepted it
- Parser: an `@for` missing a part where its header ends, as `@for $i from {` or, in the
  indented syntax, `@for $i from` / blank line / `.z`, is one error, as in Dart Sass; the
  parts after it were reported missing too, three errors at one place. In the indented
  syntax, `@for $i` goes on across blank lines only to a word, its `from`, so a rule
  below stays a rule
- Indented Sass: a written `;` that ends no statement is one error per line, as in Dart
  Sass: after a selector, `.x;` or `b:c;`, "expected newline", and the lines beneath it
  are still the rule's block; a second `;`, `b: c;;`, or a second statement after one,
  `b: c; d: e`, reports that multiple statements on one line are not supported in the
  indented syntax, Dart Sass's message. Such `;` were accepted. A `;` that ends a
  statement, or sits in a string or `url()`, stays valid

### Security
- VS Code extension: `brace-expansion`, bundled through `vscode-languageclient`, is
  updated to 2.1.7 for three denial-of-service advisories (GHSA-q2hr-2g5m-vwhr,
  GHSA-qhr7-859c-m2p7, GHSA-6j4f-fj2g-mc7p)

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
