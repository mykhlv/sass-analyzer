use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use sass_parser::imports::{ImportKind, collect_imports};
use sass_parser::syntax::{NodeOrToken, SyntaxNode};
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::{TextRange, TextSize};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionParams, Diagnostic, NumberOrString, Position, Range,
    TextEdit, Uri, WorkspaceEdit,
};

use crate::DocumentState;
use crate::ast_helpers;
use crate::convert::{lsp_position_to_offset, text_range_to_lsp};
use crate::symbols::SymbolKind;
use crate::workspace::{self, ModuleGraph, Namespace};

pub(crate) fn handle_code_action(
    documents: &DashMap<Uri, DocumentState>,
    module_graph: &ModuleGraph,
    params: CodeActionParams,
) -> Option<Vec<CodeAction>> {
    let uri = params.text_document.uri;
    let request_range = params.range;
    let doc = documents.get(&uri)?;
    let green = doc.green.clone();
    let text = doc.text.clone();
    let line_index = doc.line_index.clone();
    let symbols = Arc::clone(&doc.symbols);
    drop(doc);

    let root = SyntaxNode::new_root(green);
    let mut actions = Vec::new();

    // Collect existing @use imports, resolving to URIs for robust duplicate detection.
    // Raw source paths (e.g. "./colors") may differ from computed paths ("colors"),
    // so we compare resolved URIs instead.
    let existing_imports = collect_imports(&root);
    let existing_use_uris: HashSet<Uri> = existing_imports
        .iter()
        .filter(|i| i.kind == ImportKind::Use)
        .filter_map(|i| module_graph.resolve_import(&uri, &i.path))
        .collect();

    // Lazily initialized: only fetch when we encounter a relevant diagnostic.
    let mut all_symbols = None;

    // Quick fixes for undefined symbol diagnostics
    for diag in &params.context.diagnostics {
        let code = match &diag.code {
            Some(NumberOrString::String(s)) => s.as_str(),
            _ => continue,
        };
        let kind = match code {
            "undefined-variable" => SymbolKind::Variable,
            "undefined-function" => SymbolKind::Function,
            "undefined-mixin" => SymbolKind::Mixin,
            _ => continue,
        };

        // Read symbol name from structured diagnostic data (#7).
        // Falls back to message parsing for diagnostics from older sessions.
        let name =
            extract_name_from_data(diag).or_else(|| extract_name_from_message(&diag.message));
        let Some(name) = name else {
            continue;
        };

        let syms = all_symbols.get_or_insert_with(|| module_graph.all_symbols());
        auto_import_actions(
            &uri,
            &name,
            kind,
            diag,
            &root,
            syms,
            &existing_use_uris,
            module_graph,
            &text,
            &line_index,
            &mut actions,
        );
    }

    // Remove unused @use (only for @use statements that intersect the request range)
    unused_use_actions(
        &uri,
        &root,
        &text,
        &line_index,
        &symbols,
        &existing_imports,
        request_range,
        &mut actions,
    );

    // Extract refactorings (require non-empty selection)
    extract_variable_action(&uri, &root, &text, &line_index, request_range, &mut actions);
    extract_mixin_action(&uri, &root, &text, &line_index, request_range, &mut actions);

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

/// The text that ends a statement written into `uri`: indented Sass ends a
/// statement at the newline, SCSS needs a `;`.
fn statement_terminator(uri: &Uri) -> &'static str {
    if crate::worker::is_sass_file(uri) {
        ""
    } else {
        ";"
    }
}

/// Read symbol name from the structured `data` field of a diagnostic.
fn extract_name_from_data(diag: &Diagnostic) -> Option<String> {
    let data = diag.data.as_ref()?.as_object()?;
    data.get("name")?.as_str().map(String::from)
}

/// Fallback: extract symbol name from diagnostic message (between backtick delimiters).
/// Safe because backtick is a single ASCII byte, so byte offset +1 never
/// splits a multi-byte character.
fn extract_name_from_message(message: &str) -> Option<String> {
    let start = message.find('`')? + 1;
    let end = message[start..].find('`')? + start;
    Some(message[start..end].to_owned())
}

/// Generate auto-import code actions for an undefined symbol.
#[allow(clippy::too_many_arguments)]
fn auto_import_actions(
    from_uri: &Uri,
    name: &str,
    kind: SymbolKind,
    diagnostic: &Diagnostic,
    root: &SyntaxNode,
    all_symbols: &[(Uri, crate::symbols::Symbol)],
    existing_use_uris: &HashSet<Uri>,
    module_graph: &ModuleGraph,
    text: &str,
    line_index: &sass_parser::line_index::LineIndex,
    actions: &mut Vec<CodeAction>,
) {
    // Symbols store names without $ prefix
    let lookup_name = name.strip_prefix('$').unwrap_or(name);

    let candidates: Vec<_> = all_symbols
        .iter()
        .filter(|(uri, sym)| sym.name == lookup_name && sym.kind == kind && uri != from_uri)
        .collect();

    let first_action_index = actions.len();
    let (insert_pos, needs_newline) = find_use_insertion_point(root, text, line_index);

    for (target_uri, _sym) in candidates {
        let Some(use_path) = compute_use_path(from_uri, target_uri, module_graph) else {
            continue;
        };

        // Skip if this module is already imported
        if existing_use_uris.contains(target_uri) {
            continue;
        }

        let Namespace::Named(namespace) = workspace::default_namespace(&use_path) else {
            // default_namespace never returns Star, but guard anyway
            continue;
        };

        let prefix = if needs_newline { "\n" } else { "" };
        let terminator = statement_terminator(from_uri);
        let use_statement = format!("{prefix}@use \"{use_path}\"{terminator}\n");

        // Qualify the bare reference: $primary → colors.$primary, darken → colors.darken
        let qualified_ref = if kind == SymbolKind::Variable {
            format!("{namespace}.${lookup_name}")
        } else {
            format!("{namespace}.{lookup_name}")
        };

        let mut edits = vec![
            // Insert @use statement
            TextEdit {
                range: Range::new(insert_pos, insert_pos),
                new_text: use_statement,
            },
            // Replace bare reference with namespace-qualified form
            TextEdit {
                range: diagnostic.range,
                new_text: qualified_ref.clone(),
            },
        ];
        // LSP requires TextEdits sorted by range (earliest first) with no overlap.
        edits.sort_by_key(|e| (e.range.start.line, e.range.start.character));

        let mut changes = HashMap::new();
        changes.insert(from_uri.clone(), edits);

        let title = format!("Add `@use \"{use_path}\"` ({qualified_ref})");

        actions.push(CodeAction {
            title,
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                ..WorkspaceEdit::default()
            }),
            is_preferred: Some(actions.len() == first_action_index),
            ..CodeAction::default()
        });
    }
}

/// Compute the `@use` path from one file to another.
///
/// Converts URIs to filesystem paths, computes a relative path,
/// and strips Sass conventions (`_` prefix, `.scss`/`.sass` suffix).
/// Validates via round-trip resolution.
fn compute_use_path(
    from_uri: &Uri,
    target_uri: &Uri,
    module_graph: &ModuleGraph,
) -> Option<String> {
    let from_path = workspace::uri_to_path(from_uri)?;
    let target_path = workspace::uri_to_path(target_uri)?;

    let from_dir = from_path.parent()?;
    let rel = diff_paths(&target_path, from_dir);

    let spec = normalize_sass_path(&rel);

    // Round-trip validation: check that the spec resolves back to target
    if let Some(resolved_uri) = module_graph.resolve_import(from_uri, &spec)
        && resolved_uri == *target_uri
    {
        return Some(spec);
    }

    // Fallback: try without stripping _ (non-partial files)
    let spec2 = normalize_extension_only(&rel);
    if spec2 != spec
        && let Some(resolved_uri) = module_graph.resolve_import(from_uri, &spec2)
        && resolved_uri == *target_uri
    {
        return Some(spec2);
    }

    None
}

/// Normalize a relative path into a Sass `@use` specifier:
/// strip `.scss`/`.sass`/`.css`, strip leading `_` from filename.
///
/// String slicing is safe here because the suffixes (`.scss`, `.sass`, `.css`),
/// the separator (`/`), and the partial prefix (`_`) are all ASCII.
fn normalize_sass_path(rel: &Path) -> String {
    let mut spec = path_to_forward_slash(rel);

    // Strip Sass extensions
    for ext in &[".scss", ".sass", ".css"] {
        if let Some(stripped) = spec.strip_suffix(ext) {
            spec = stripped.to_owned();
            break;
        }
    }

    // Strip leading _ from filename (Sass partial convention)
    if let Some(slash_pos) = spec.rfind('/') {
        if spec[slash_pos + 1..].starts_with('_') {
            spec = format!("{}/{}", &spec[..slash_pos], &spec[slash_pos + 2..]);
        }
    } else if let Some(stripped) = spec.strip_prefix('_') {
        spec = stripped.to_owned();
    }

    spec
}

/// Strip only the extension (not the `_` prefix) — used as a fallback
/// when the partial-stripped path doesn't round-trip.
fn normalize_extension_only(rel: &Path) -> String {
    let mut spec = path_to_forward_slash(rel);
    for ext in &[".scss", ".sass", ".css"] {
        if let Some(stripped) = spec.strip_suffix(ext) {
            spec = stripped.to_owned();
            break;
        }
    }
    spec
}

/// Convert a Path to a forward-slash string (handles Windows backslashes).
fn path_to_forward_slash(p: &Path) -> String {
    let s = p.to_string_lossy().into_owned();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s
    }
}

/// Compute the relative path from `base` directory to `target` file.
///
/// Only handles absolute paths with a common root (the normal case for
/// files within a single workspace). Returns `"."` when target equals base (#10).
fn diff_paths(target: &Path, base: &Path) -> PathBuf {
    let mut base_comps = base.components().peekable();
    let mut target_comps = target.components().peekable();

    // Skip common prefix
    while let (Some(b), Some(t)) = (base_comps.peek(), target_comps.peek()) {
        if b == t {
            base_comps.next();
            target_comps.next();
        } else {
            break;
        }
    }

    let mut result = PathBuf::new();
    for comp in base_comps {
        if matches!(comp, Component::Normal(_)) {
            result.push("..");
        }
    }
    for comp in target_comps {
        result.push(comp);
    }

    // target == base → empty PathBuf; return "." (#10)
    if result.as_os_str().is_empty() {
        result.push(".");
    }

    result
}

/// Find where to insert a new `@use` statement.
///
/// Scans ALL top-level children to find the last import rule anywhere,
/// not just the first contiguous block (#12).
///
/// Returns `(position, needs_newline_prefix)`. When the last import ends at
/// EOF without a trailing newline, the caller must prepend `\n` to the
/// inserted text to avoid concatenating on the same line.
fn find_use_insertion_point(
    root: &SyntaxNode,
    text: &str,
    line_index: &sass_parser::line_index::LineIndex,
) -> (Position, bool) {
    let mut last_import_end = None;

    for child in root.children() {
        if matches!(
            child.kind(),
            SyntaxKind::USE_RULE | SyntaxKind::FORWARD_RULE | SyntaxKind::IMPORT_RULE
        ) {
            last_import_end = Some(child.text_range().end());
        }
    }

    if let Some(end) = last_import_end {
        let offset: usize = end.into();
        let needs_newline = offset >= text.len() || text.as_bytes()[offset] != b'\n';
        let range = sass_parser::text_range::TextRange::new(end, end);
        let lsp_pos = text_range_to_lsp(range, line_index, text);
        (Position::new(lsp_pos.start.line + 1, 0), needs_newline)
    } else {
        (Position::new(0, 0), false)
    }
}

/// Find unused `@use` statements and generate removal actions.
///
/// Limitations:
/// - `@use ... as *` is always skipped (can't reliably detect usage).
/// - `show`/`hide` clauses are not considered — a `@use` with `show` that
///   restricts to unused symbols won't be detected if the namespace is used
///   for other symbols (#6).
/// - Side-effect imports (modules producing global CSS) are not distinguished;
///   the action is not associated with a diagnostic, so users must explicitly
///   choose to apply it (#9).
#[allow(clippy::too_many_arguments)]
fn unused_use_actions(
    uri: &Uri,
    root: &SyntaxNode,
    text: &str,
    line_index: &sass_parser::line_index::LineIndex,
    symbols: &crate::symbols::FileSymbols,
    import_refs: &[sass_parser::imports::ImportRef],
    request_range: Range,
    actions: &mut Vec<CodeAction>,
) {
    for import_ref in import_refs {
        if import_ref.kind != ImportKind::Use {
            continue;
        }

        let use_range = text_range_to_lsp(import_ref.range, line_index, text);

        // Only offer removal for @use statements that intersect the request range.
        if use_range.end.line < request_range.start.line
            || use_range.start.line > request_range.end.line
        {
            continue;
        }

        let ns = workspace::extract_namespace(root, import_ref);

        // Can't detect unused for `@use ... as *` — symbols merge into scope
        let ns_name = match &ns {
            Namespace::Named(n) => n.clone(),
            Namespace::Star => continue,
        };

        // A named @use only serves qualified references (ns.$var, ns.func()).
        // Unqualified references never resolve through a named @use.
        let is_used = symbols.references.iter().any(|r| {
            let ref_ns = ast_helpers::namespace_of_ref(root, r.range);
            ref_ns.as_deref() == Some(ns_name.as_str())
        });

        if is_used {
            continue;
        }
        // Delete the full line including newline. If the @use is on the last
        // line without a trailing newline, line+1 is past EOF — LSP editors
        // clamp to EOF, so the deletion still works correctly (#8).
        let delete_range = Range::new(
            Position::new(use_range.start.line, 0),
            Position::new(use_range.end.line + 1, 0),
        );

        let mut changes = HashMap::new();
        changes.insert(
            uri.clone(),
            vec![TextEdit {
                range: delete_range,
                new_text: String::new(),
            }],
        );

        actions.push(CodeAction {
            title: format!("Remove unused `@use \"{}\"`", import_ref.path),
            kind: Some(CodeActionKind::QUICKFIX),
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                ..WorkspaceEdit::default()
            }),
            ..CodeAction::default()
        });
    }
}

// ── Extract to variable ────────────────────────────────────────────

#[rustfmt::skip]
fn is_expression_node(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NUMBER_LITERAL   | SyntaxKind::DIMENSION
        | SyntaxKind::STRING_LITERAL | SyntaxKind::INTERPOLATED_STRING
        | SyntaxKind::COLOR_LITERAL  | SyntaxKind::BOOL_LITERAL
        | SyntaxKind::NULL_LITERAL   | SyntaxKind::VARIABLE_REF
        | SyntaxKind::BINARY_EXPR    | SyntaxKind::UNARY_EXPR
        | SyntaxKind::PAREN_EXPR     | SyntaxKind::FUNCTION_CALL
        | SyntaxKind::SPECIAL_FUNCTION_CALL | SyntaxKind::CALCULATION
        | SyntaxKind::LIST_EXPR      | SyntaxKind::BRACKETED_LIST
        | SyntaxKind::MAP_EXPR       | SyntaxKind::NAMESPACE_REF
    )
}

/// Find the tightest expression node whose content matches the selection.
///
/// Rowan nodes include leading trivia (whitespace) recursively, so a user
/// selecting `#333` yields a byte range that starts after the space, while
/// the `COLOR_LITERAL` node range includes it. We compute the "content
/// start" by finding the first non-whitespace byte in the node text and
/// accept selections that match either the full node range or the trimmed
/// content range.
fn find_expression_at_range(root: &SyntaxNode, range: TextRange) -> Option<SyntaxNode> {
    let elem = root.covering_element(range);
    let start_node = match elem {
        NodeOrToken::Node(n) => n,
        NodeOrToken::Token(t) => t.parent()?,
    };

    for node in start_node.ancestors() {
        if !is_expression_node(node.kind()) {
            continue;
        }
        let nr = node.text_range();
        // Rowan attaches leading trivia to tokens, so the node range starts
        // before visible content. Accept selections matching either the full
        // node range or the content range (first non-trivia token → end).
        let cr = content_range(&node);
        if range != nr && range != cr {
            continue;
        }
        let in_value = node
            .ancestors()
            .any(|a| matches!(a.kind(), SyntaxKind::VALUE | SyntaxKind::VARIABLE_DECL));
        if !in_value {
            return None;
        }
        return Some(node);
    }
    None
}

/// Find the statement-level node before which a variable should be inserted.
/// Returns the `RULE_SET` or other top-level item that contains the expression.
fn find_insertion_ancestor(expr: &SyntaxNode) -> Option<SyntaxNode> {
    for ancestor in expr.ancestors() {
        match ancestor.kind() {
            SyntaxKind::RULE_SET
            | SyntaxKind::MIXIN_RULE
            | SyntaxKind::FUNCTION_RULE
            | SyntaxKind::EACH_RULE
            | SyntaxKind::FOR_RULE
            | SyntaxKind::WHILE_RULE
            | SyntaxKind::IF_RULE => return Some(ancestor),
            // At top level, use the DECLARATION or VARIABLE_DECL itself
            SyntaxKind::DECLARATION | SyntaxKind::VARIABLE_DECL => {
                if ancestor
                    .parent()
                    .is_some_and(|p| p.kind() == SyntaxKind::SOURCE_FILE)
                {
                    return Some(ancestor);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract leading whitespace from the line containing `byte_offset`.
fn indentation_at(text: &str, byte_offset: usize) -> &str {
    let line_start = text[..byte_offset].rfind('\n').map_or(0, |p| p + 1);
    let non_ws = text[line_start..]
        .find(|c: char| !c.is_ascii_whitespace() || c == '\n')
        .unwrap_or(0);
    &text[line_start..line_start + non_ws]
}

/// Byte offset of the first non-trivia token in `node`.
fn first_content_offset(node: &SyntaxNode) -> TextSize {
    node.descendants_with_tokens()
        .find_map(|e| match e {
            NodeOrToken::Token(t) if !t.kind().is_trivia() => Some(t.text_range().start()),
            _ => None,
        })
        .unwrap_or(node.text_range().start())
}

/// Get the byte offset of the start of the line containing a node's first content.
#[allow(clippy::cast_possible_truncation)]
fn content_line_start(text: &str, node: &SyntaxNode) -> TextSize {
    let content_offset: usize = first_content_offset(node).into();
    let line_start = text[..content_offset].rfind('\n').map_or(0, |p| p + 1);
    TextSize::from(line_start as u32)
}

/// Get the indentation of a node by looking at its first non-trivia content.
fn node_indentation<'a>(text: &'a str, node: &SyntaxNode) -> &'a str {
    indentation_at(text, usize::from(first_content_offset(node)))
}

fn extract_variable_action(
    uri: &Uri,
    root: &SyntaxNode,
    text: &str,
    line_index: &sass_parser::line_index::LineIndex,
    request_range: Range,
    actions: &mut Vec<CodeAction>,
) {
    // Require non-empty selection
    if request_range.start == request_range.end {
        return;
    }

    let Some(start) = lsp_position_to_offset(text, line_index, request_range.start) else {
        return;
    };
    let Some(end) = lsp_position_to_offset(text, line_index, request_range.end) else {
        return;
    };
    let sel_range = TextRange::new(start, end);

    let Some(expr) = find_expression_at_range(root, sel_range) else {
        return;
    };

    // Use the selection range for replacement and value text (avoids leading trivia)
    let sel_start: usize = sel_range.start().into();
    let sel_end: usize = sel_range.end().into();
    let expr_text = &text[sel_start..sel_end];
    let Some(insert_before) = find_insertion_ancestor(&expr) else {
        return;
    };

    let indent = node_indentation(text, &insert_before);
    let insert_line_start = content_line_start(text, &insert_before);

    let terminator = statement_terminator(uri);
    let var_decl = format!("{indent}$new-variable: {expr_text}{terminator}\n");
    let insert_pos = text_range_to_lsp(
        TextRange::new(insert_line_start, insert_line_start),
        line_index,
        text,
    );

    let sel_lsp_range = text_range_to_lsp(sel_range, line_index, text);

    let mut edits = vec![
        TextEdit {
            range: Range::new(insert_pos.start, insert_pos.start),
            new_text: var_decl,
        },
        TextEdit {
            range: sel_lsp_range,
            new_text: "$new-variable".to_owned(),
        },
    ];
    edits.sort_by_key(|e| (e.range.start.line, e.range.start.character));

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    actions.push(CodeAction {
        title: "Extract to variable".to_owned(),
        kind: Some(CodeActionKind::REFACTOR_EXTRACT),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }),
        ..CodeAction::default()
    });
}

// ── Extract to mixin ───────────────────────────────────────────────

/// The range of a node excluding leading trivia (first non-trivia token → node end).
fn content_range(node: &SyntaxNode) -> TextRange {
    TextRange::new(first_content_offset(node), node.text_range().end())
}

/// Statements that cannot move into a mixin: `@content` and `@return` would
/// change what they refer to, and a mixin may not declare a mixin or function.
fn blocks_mixin_extraction(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::CONTENT_RULE
            | SyntaxKind::RETURN_RULE
            | SyntaxKind::MIXIN_RULE
            | SyntaxKind::FUNCTION_RULE
    )
}

/// The block whose statements `range` selects, and the source text to move out
/// of it.
///
/// Once trimmed of surrounding whitespace, the selection must lie inside one
/// block's braces and cover each statement it touches completely; otherwise
/// there is no action. Comments wholly inside the selection move along with the
/// statements, a comment the selection cuts through stays put. The moved text
/// is a single verbatim range, so whatever sits between the first and the last
/// statement — nested rules, at-rules, comments — moves with them.
#[allow(clippy::cast_possible_truncation)]
fn find_statements_at_range(
    root: &SyntaxNode,
    text: &str,
    range: TextRange,
) -> Option<(SyntaxNode, TextRange)> {
    let selected = &text[usize::from(range.start())..usize::from(range.end())];
    let trimmed = selected.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lead = selected.len() - selected.trim_start().len();
    let start = range.start() + TextSize::from(lead as u32);
    let sel = TextRange::at(start, TextSize::from(trimmed.len() as u32));

    let start_node = match root.covering_element(sel) {
        NodeOrToken::Node(n) => n,
        NodeOrToken::Token(t) => t.parent()?,
    };
    let block = start_node
        .ancestors()
        .find(|a| a.kind() == SyntaxKind::BLOCK)?;

    let brace = |kind| {
        block
            .children_with_tokens()
            .find(|e| e.kind() == kind)
            .map(|e| e.text_range())
    };
    let inner_start = brace(SyntaxKind::LBRACE).map_or(block.text_range().start(), TextRange::end);
    let inner_end = brace(SyntaxKind::RBRACE).map_or(block.text_range().end(), TextRange::start);
    if sel.start() < inner_start || sel.end() > inner_end {
        return None;
    }

    let mut moved: Option<TextRange> = None;
    for child in block.children_with_tokens() {
        let r = match &child {
            NodeOrToken::Node(n) => content_range(n),
            NodeOrToken::Token(t) if t.kind().is_trivia() => continue,
            NodeOrToken::Token(t) => t.text_range(),
        };
        if r.end() <= sel.start() || r.start() >= sel.end() {
            continue;
        }
        let NodeOrToken::Node(n) = &child else {
            return None;
        };
        if !sel.contains_range(r) || n.descendants().any(|d| blocks_mixin_extraction(d.kind())) {
            return None;
        }
        moved = Some(moved.map_or(r, |m| m.cover(r)));
    }
    let mut moved = moved?;

    for token in block
        .descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
    {
        let is_comment = matches!(
            token.kind(),
            SyntaxKind::SINGLE_LINE_COMMENT | SyntaxKind::MULTI_LINE_COMMENT
        );
        if is_comment && sel.contains_range(token.text_range()) {
            moved = moved.cover(token.text_range());
        }
    }
    Some((block, moved))
}

fn extract_mixin_action(
    uri: &Uri,
    root: &SyntaxNode,
    text: &str,
    line_index: &sass_parser::line_index::LineIndex,
    request_range: Range,
    actions: &mut Vec<CodeAction>,
) {
    if request_range.start == request_range.end {
        return;
    }

    let Some(start) = lsp_position_to_offset(text, line_index, request_range.start) else {
        return;
    };
    let Some(end) = lsp_position_to_offset(text, line_index, request_range.end) else {
        return;
    };
    let sel_range = TextRange::new(start, end);

    let Some((block, moved)) = find_statements_at_range(root, text, sel_range) else {
        return;
    };

    let Some(rule_set) = mixin_insertion_anchor(&block) else {
        return;
    };

    let rule_indent = node_indentation(text, &rule_set);
    let body = mixin_body(text, &block, moved, rule_indent);

    let (mixin_text, include_text) = if crate::worker::is_sass_file(uri) {
        indented_mixin_texts(root, &block, &body, rule_indent)
    } else {
        (
            format!("{rule_indent}@mixin new-mixin {{\n{body}\n{rule_indent}}}\n\n"),
            "@include new-mixin;".to_owned(),
        )
    };

    let replace_lsp = text_range_to_lsp(moved, line_index, text);
    let insert_line = content_line_start(text, &rule_set);
    let insert_range = TextRange::new(insert_line, insert_line);
    let insert_lsp = text_range_to_lsp(insert_range, line_index, text);

    let mut edits = vec![
        TextEdit {
            range: Range::new(insert_lsp.start, insert_lsp.start),
            new_text: mixin_text,
        },
        TextEdit {
            range: replace_lsp,
            new_text: include_text,
        },
    ];
    edits.sort_by_key(|e| (e.range.start.line, e.range.start.character));

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    actions.push(CodeAction {
        title: "Extract to mixin".to_owned(),
        kind: Some(CodeActionKind::REFACTOR_EXTRACT),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }),
        ..CodeAction::default()
    });
}

/// The statement the new mixin is inserted before, or `None` where no mixin can
/// be included at all (a function body).
///
/// Dart Sass rejects a mixin declared inside another mixin, a control directive
/// or an `@include` content block, so the mixin goes before the outermost of
/// those. Elsewhere it goes before the nearest style rule, which keeps it local
/// to the parent rule.
fn mixin_insertion_anchor(block: &SyntaxNode) -> Option<SyntaxNode> {
    if block
        .ancestors()
        .any(|a| a.kind() == SyntaxKind::FUNCTION_RULE)
    {
        return None;
    }
    block
        .ancestors()
        .filter(|a| {
            matches!(
                a.kind(),
                SyntaxKind::MIXIN_RULE
                    | SyntaxKind::INCLUDE_RULE
                    | SyntaxKind::EACH_RULE
                    | SyntaxKind::FOR_RULE
                    | SyntaxKind::WHILE_RULE
                    | SyntaxKind::IF_RULE
            )
        })
        .last()
        .or_else(|| block.ancestors().find(|a| a.kind() == SyntaxKind::RULE_SET))
}

/// The `moved` text of `block`, re-indented one unit under `rule_indent`.
///
/// The text moves verbatim; only indentation changes. Each line keeps its
/// indentation relative to the first one, which keeps nested blocks and
/// continuation lines attached to the line that owns them — and in indented
/// Sass, where indentation alone opens a block, keeps the meaning intact.
fn mixin_body(text: &str, block: &SyntaxNode, moved: TextRange, rule_indent: &str) -> String {
    let start = usize::from(moved.start());
    let first_indent = indentation_at(text, start);
    // The unit is the step from the block's header to its body. The rule the
    // mixin is inserted before can sit several levels above the block (a
    // declaration under `@media` inside a rule), so it cannot serve as the base.
    let header_indent = block.parent().map_or("", |h| node_indentation(text, &h));
    let unit = first_indent
        .strip_prefix(header_indent)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| detect_indent_unit(text));
    let body_indent = format!("{rule_indent}{unit}");

    text[start..usize::from(moved.end())]
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if line.trim().is_empty() {
                return String::new();
            }
            let relative = if i == 0 {
                line
            } else {
                line.strip_prefix(first_indent)
                    .unwrap_or_else(|| line.trim_start())
            };
            format!("{body_indent}{relative}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The mixin definition and the `@include` that replaces the selection, for
/// indented Sass.
///
/// The `=`/`+` shorthands are used only when the file already uses them. The
/// long forms are always valid and are what a reader expects by default; the
/// shorthands are a style a file opts into, so the edit follows that choice
/// rather than introducing it. The `+` form is never written into a nested
/// property's block, where Dart Sass accepts only declarations and at-rules.
fn indented_mixin_texts(
    root: &SyntaxNode,
    block: &SyntaxNode,
    body: &str,
    rule_indent: &str,
) -> (String, String) {
    let header = if uses_shorthand(root, SyntaxKind::MIXIN_RULE, "=") {
        "=new-mixin"
    } else {
        "@mixin new-mixin"
    };
    let in_nested_property = block
        .parent()
        .is_some_and(|p| p.kind() == SyntaxKind::NESTED_PROPERTY);
    let include = if !in_nested_property && uses_shorthand(root, SyntaxKind::INCLUDE_RULE, "+") {
        "+new-mixin"
    } else {
        "@include new-mixin"
    };
    (
        format!("{rule_indent}{header}\n{body}\n\n"),
        include.to_owned(),
    )
}

/// Whether any `kind` node in the file is written with the indented shorthand
/// `sigil` (`=` for `@mixin`, `+` for `@include`). The parser keeps the sigil as
/// the node's first IDENT, where the long form has the directive name.
fn uses_shorthand(root: &SyntaxNode, kind: SyntaxKind, sigil: &str) -> bool {
    root.descendants()
        .filter(|n| n.kind() == kind)
        .any(|n| ast_helpers::ident_text_range_of(&n).is_some_and(|(t, _)| t == sigil))
}

/// The leading whitespace of the first indented line, or two spaces for a file
/// with no indentation at all.
fn detect_indent_unit(text: &str) -> &str {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| &line[..line.len() - line.trim_start().len()])
        .find(|ws| !ws.is_empty())
        .unwrap_or("  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_name_from_data ──────────────────────────────────────

    #[test]
    fn extract_name_from_diagnostic_data() {
        let diag = Diagnostic {
            data: Some(serde_json::json!({"name": "primary", "kind": "variable"})),
            ..Diagnostic::default()
        };
        assert_eq!(extract_name_from_data(&diag), Some("primary".to_owned()));
    }

    #[test]
    fn extract_name_from_data_missing() {
        let diag = Diagnostic::default();
        assert_eq!(extract_name_from_data(&diag), None);
    }

    // ── extract_name_from_message (fallback) ────────────────────────

    #[test]
    fn extract_name_from_undefined_variable_message() {
        // diagnostics.rs emits names without $: `undefined variable \`primary\``
        assert_eq!(
            extract_name_from_message("undefined variable `primary`"),
            Some("primary".to_owned())
        );
    }

    #[test]
    fn extract_name_from_undefined_function_message() {
        assert_eq!(
            extract_name_from_message("undefined function `darken`"),
            Some("darken".to_owned())
        );
    }

    #[test]
    fn extract_name_from_undefined_mixin_message() {
        assert_eq!(
            extract_name_from_message("undefined mixin `flex-center`"),
            Some("flex-center".to_owned())
        );
    }

    // ── find_use_insertion_point ─────────────────────────────────────

    #[test]
    fn insertion_point_empty_file() {
        let (green, _) = sass_parser::parse_scss("");
        let root = SyntaxNode::new_root(green);
        let text = "";
        let li = sass_parser::line_index::LineIndex::new(text);
        let (pos, nl) = find_use_insertion_point(&root, text, &li);
        assert_eq!(pos, Position::new(0, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_after_existing_use() {
        let src = "@use \"colors\";\n.btn { color: red; }\n";
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(1, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_after_multiple_uses() {
        let src = "@use \"colors\";\n@use \"mixins\";\n.btn { }\n";
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(2, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_after_forward() {
        let src = "@forward \"colors\";\n@use \"mixins\";\n.btn { }\n";
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(2, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_no_imports_with_rules() {
        let src = ".btn { color: red; }\n";
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(0, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_use_after_rule_finds_last_use() {
        // Non-standard layout: @use after a rule (#12)
        let src = "@use \"a\";\n.btn { }\n@use \"b\";\n";
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(3, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_no_trailing_newline_needs_prefix() {
        let src = "@use \"colors\";";
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(1, 0));
        assert!(nl);
    }

    // ── diff_paths ──────────────────────────────────────────────────

    #[test]
    fn diff_paths_same_directory() {
        let result = diff_paths(Path::new("/a/b/c.scss"), Path::new("/a/b"));
        assert_eq!(result, PathBuf::from("c.scss"));
    }

    #[test]
    fn diff_paths_parent_directory() {
        let result = diff_paths(Path::new("/a/c.scss"), Path::new("/a/b"));
        assert_eq!(result, PathBuf::from("../c.scss"));
    }

    #[test]
    fn diff_paths_nested() {
        let result = diff_paths(Path::new("/a/b/d/e.scss"), Path::new("/a/b/c"));
        assert_eq!(result, PathBuf::from("../d/e.scss"));
    }

    #[test]
    fn diff_paths_identical_returns_dot() {
        let result = diff_paths(Path::new("/a/b"), Path::new("/a/b"));
        assert_eq!(result, PathBuf::from("."));
    }

    // ── normalize_sass_path ─────────────────────────────────────────

    #[test]
    fn normalize_strips_scss_and_underscore() {
        assert_eq!(normalize_sass_path(Path::new("_colors.scss")), "colors");
        assert_eq!(
            normalize_sass_path(Path::new("utils/_mixins.scss")),
            "utils/mixins"
        );
        assert_eq!(normalize_sass_path(Path::new("styles.scss")), "styles");
    }

    #[test]
    fn normalize_strips_sass_extension() {
        assert_eq!(normalize_sass_path(Path::new("_base.sass")), "base");
    }

    #[test]
    fn normalize_strips_css_extension() {
        assert_eq!(normalize_sass_path(Path::new("vendor.css")), "vendor");
    }

    // ── extract_name edge cases ───────────────────────────────────────

    #[test]
    fn extract_name_from_malformed_data() {
        // data present but wrong shape — should fall back gracefully
        let diag = Diagnostic {
            data: Some(serde_json::json!({"wrong_key": 42})),
            ..Diagnostic::default()
        };
        assert_eq!(extract_name_from_data(&diag), None);

        let diag2 = Diagnostic {
            data: Some(serde_json::json!("not an object")),
            ..Diagnostic::default()
        };
        assert_eq!(extract_name_from_data(&diag2), None);
    }

    // ── extract variable tests ───────────────────────────────────────

    fn run_extract_variable(
        src: &str,
        sel_start: (u32, u32),
        sel_end: (u32, u32),
    ) -> Vec<TextEdit> {
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let uri: Uri = "file:///test.scss".parse().unwrap();
        let range = Range::new(
            Position::new(sel_start.0, sel_start.1),
            Position::new(sel_end.0, sel_end.1),
        );
        let mut actions = Vec::new();
        extract_variable_action(&uri, &root, src, &li, range, &mut actions);
        actions
            .into_iter()
            .flat_map(|a| a.edit.unwrap().changes.unwrap().into_values().flatten())
            .collect()
    }

    #[test]
    fn extract_var_color_literal() {
        let src = ".btn { color: #333; }\n";
        // Select #333 → bytes 14..18, line 0 col 14..18
        let edits = run_extract_variable(src, (0, 14), (0, 18));
        assert_eq!(edits.len(), 2);
        // First edit: insert variable before .btn
        assert_eq!(edits[0].new_text, "$new-variable: #333;\n");
        assert_eq!(edits[0].range.start, Position::new(0, 0));
        // Second edit: replace #333
        assert_eq!(edits[1].new_text, "$new-variable");
        assert_eq!(edits[1].range.start, Position::new(0, 14));
    }

    #[test]
    fn extract_var_dimension() {
        let src = ".btn { width: 100px; }\n";
        // Select 100px → bytes 14..19
        let edits = run_extract_variable(src, (0, 14), (0, 19));
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "$new-variable: 100px;\n");
        assert_eq!(edits[1].new_text, "$new-variable");
    }

    #[test]
    fn extract_var_function_call() {
        let src = ".btn { color: darken(red, 10%); }\n";
        // darken(red, 10%) → bytes 14..30
        let edits = run_extract_variable(src, (0, 14), (0, 30));
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "$new-variable: darken(red, 10%);\n");
    }

    #[test]
    fn extract_var_binary_expr() {
        let src = ".btn { width: $base * 2; }\n";
        // $base * 2 → bytes 14..23
        let edits = run_extract_variable(src, (0, 14), (0, 23));
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "$new-variable: $base * 2;\n");
    }

    #[test]
    fn extract_var_nested_rule() {
        let src = ".parent {\n  .child {\n    color: #fff;\n  }\n}\n";
        // #fff → line 2, col 11..15
        let edits = run_extract_variable(src, (2, 11), (2, 15));
        assert_eq!(edits.len(), 2);
        // Should insert before .child rule with 2-space indent
        assert_eq!(edits[0].new_text, "  $new-variable: #fff;\n");
        assert_eq!(edits[0].range.start, Position::new(1, 0));
    }

    #[test]
    fn extract_var_top_level_decl() {
        let src = "$color: #333;\n";
        // #333 → bytes 8..12
        let edits = run_extract_variable(src, (0, 8), (0, 12));
        // Top-level $color decl, insert before it
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "$new-variable: #333;\n");
        assert_eq!(edits[0].range.start, Position::new(0, 0));
    }

    #[test]
    fn extract_var_zero_width_no_action() {
        let src = ".btn { color: #333; }\n";
        let edits = run_extract_variable(src, (0, 14), (0, 14));
        assert!(edits.is_empty());
    }

    #[test]
    fn extract_var_partial_token_no_action() {
        let src = ".btn { color: #333; }\n";
        // Select just "33" inside #333
        let edits = run_extract_variable(src, (0, 15), (0, 17));
        assert!(edits.is_empty());
    }

    #[test]
    fn extract_var_selector_no_action() {
        let src = ".btn { color: red; }\n";
        // Select ".btn" (selector, not in VALUE)
        let edits = run_extract_variable(src, (0, 0), (0, 4));
        assert!(edits.is_empty());
    }

    #[test]
    fn extract_var_string_literal() {
        let src = ".btn { content: 'hello'; }\n";
        // 'hello' → bytes 16..23
        let edits = run_extract_variable(src, (0, 16), (0, 23));
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "$new-variable: 'hello';\n");
    }

    // ── extract mixin tests ─────────────────────────────────────────

    fn run_extract_mixin(src: &str, sel_start: (u32, u32), sel_end: (u32, u32)) -> Vec<TextEdit> {
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let uri: Uri = "file:///test.scss".parse().unwrap();
        let range = Range::new(
            Position::new(sel_start.0, sel_start.1),
            Position::new(sel_end.0, sel_end.1),
        );
        let mut actions = Vec::new();
        extract_mixin_action(&uri, &root, src, &li, range, &mut actions);
        actions
            .into_iter()
            .flat_map(|a| a.edit.unwrap().changes.unwrap().into_values().flatten())
            .collect()
    }

    #[test]
    fn extract_mixin_single_decl() {
        let src = ".btn {\n  display: flex;\n}\n";
        // Select "display: flex;" → line 1, col 2..16
        let edits = run_extract_mixin(src, (1, 2), (1, 16));
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("@mixin new-mixin"));
        assert!(edits[0].new_text.contains("display: flex;"));
        assert!(edits[1].new_text.contains("@include new-mixin;"));
    }

    #[test]
    fn extract_mixin_multiple_decls() {
        let src = ".btn {\n  display: flex;\n  align-items: center;\n}\n";
        // Select both declarations → line 1 col 2 to line 2 col 22
        let edits = run_extract_mixin(src, (1, 2), (2, 22));
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("display: flex;"));
        assert!(edits[0].new_text.contains("align-items: center;"));
    }

    #[test]
    fn extract_mixin_zero_width_no_action() {
        let src = ".btn {\n  display: flex;\n}\n";
        let edits = run_extract_mixin(src, (1, 5), (1, 5));
        assert!(edits.is_empty());
    }

    #[test]
    fn extract_mixin_no_rule_set_no_action() {
        // Top-level declaration (no enclosing rule set)
        let src = "$color: red;\n";
        let edits = run_extract_mixin(src, (0, 0), (0, 12));
        assert!(edits.is_empty());
    }

    #[test]
    fn extract_mixin_nested_rule() {
        let src = ".parent {\n  .child {\n    display: flex;\n  }\n}\n";
        // Select "display: flex;" inside .child
        let edits = run_extract_mixin(src, (2, 4), (2, 18));
        assert_eq!(edits.len(), 2);
        // Mixin should be inserted before .child
        assert!(edits[0].new_text.contains("@mixin new-mixin"));
    }

    #[test]
    fn extract_mixin_with_important() {
        let src = ".btn {\n  color: red !important;\n}\n";
        let edits = run_extract_mixin(src, (1, 2), (1, 27));
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("color: red !important;"));
    }

    #[test]
    fn extract_mixin_indentation() {
        let src = ".parent {\n  .child {\n    color: red;\n  }\n}\n";
        let edits = run_extract_mixin(src, (2, 4), (2, 15));
        assert_eq!(edits.len(), 2);
        // Mixin should have .child's indentation level
        assert!(edits[0].new_text.starts_with("  @mixin"));
    }

    #[test]
    fn extract_mixin_selection_not_on_decl_no_action() {
        let src = ".btn {\n  display: flex;\n}\n";
        // Select the opening brace area — no declarations
        let edits = run_extract_mixin(src, (0, 5), (0, 6));
        assert!(edits.is_empty());
    }

    #[test]
    fn extract_mixin_variable_decl_in_block() {
        let src = ".btn {\n  $size: 10px;\n  width: $size;\n}\n";
        // Select $size declaration
        let edits = run_extract_mixin(src, (1, 2), (1, 14));
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("$size: 10px;"));
    }

    // ── round-trip / integration tests ──────────────────────────────

    /// Apply sorted, non-overlapping LSP `TextEdits` to source text (last-to-first).
    fn apply_edits(src: &str, edits: &[TextEdit]) -> String {
        let mut result = src.to_owned();
        // Apply in reverse order to preserve offsets
        let li = sass_parser::line_index::LineIndex::new(src);
        let mut byte_edits: Vec<_> = edits
            .iter()
            .map(|e| {
                let start = lsp_position_to_offset(src, &li, e.range.start).unwrap();
                let end = lsp_position_to_offset(src, &li, e.range.end).unwrap();
                (usize::from(start), usize::from(end), &e.new_text)
            })
            .collect();
        byte_edits.sort_by(|a, b| b.0.cmp(&a.0));
        for (start, end, new_text) in byte_edits {
            result.replace_range(start..end, new_text);
        }
        result
    }

    #[test]
    fn extract_var_round_trip() {
        let src = ".btn {\n  color: #333;\n}\n";
        let edits = run_extract_variable(src, (1, 9), (1, 13));
        assert_eq!(edits.len(), 2);
        let result = apply_edits(src, &edits);
        // Variable is inserted before .btn (enclosing rule set)
        assert_eq!(
            result,
            "$new-variable: #333;\n.btn {\n  color: $new-variable;\n}\n"
        );
        let (_, errors) = sass_parser::parse_scss(&result);
        assert!(errors.is_empty(), "Parse errors: {errors:?}");
    }

    #[test]
    fn extract_mixin_round_trip() {
        let src = ".btn {\n  display: flex;\n  color: red;\n}\n";
        // Select both declarations: line 1 col 2 to line 2 col 13 (inclusive of `;`)
        let edits = run_extract_mixin(src, (1, 2), (2, 13));
        assert_eq!(edits.len(), 2);
        let result = apply_edits(src, &edits);
        // Verify result parses without errors
        let (_, errors) = sass_parser::parse_scss(&result);
        assert!(errors.is_empty(), "Parse errors in:\n{result}\n{errors:?}");
        assert!(result.contains("@mixin new-mixin"));
        assert!(result.contains("@include new-mixin;"));
    }

    #[test]
    fn extract_var_inside_mixin_body() {
        let src = "@mixin box {\n  width: 100px;\n}\n";
        // Select "100px" — line 1, col 9..14
        let edits = run_extract_variable(src, (1, 9), (1, 14));
        assert_eq!(edits.len(), 2);
        // Should insert before @mixin rule
        assert_eq!(edits[0].new_text, "$new-variable: 100px;\n");
        assert_eq!(edits[0].range.start, Position::new(0, 0));
    }

    #[test]
    fn extract_mixin_inside_mixin_body() {
        // A mixin may not declare another one, so the new mixin goes before
        // `@mixin outer`, not before `.inner`.
        check_scss_mixin(
            "@mixin outer {\n  .inner {\n    color: red;\n  }\n}\n",
            "color: red;",
            &expect_test::expect![[r#"
                @mixin new-mixin {
                  color: red;
                }

                @mixin outer {
                  .inner {
                    @include new-mixin;
                  }
                }
            "#]],
        );
    }

    #[test]
    fn extract_mixin_inside_control_directive_and_content_block() {
        check_scss_mixin(
            ".a {\n  @each $k in x, y {\n    .#{$k} { color: red; }\n  }\n}\n",
            "color: red;",
            &expect_test::expect![[r#"
                .a {
                  @mixin new-mixin {
                    color: red;
                  }

                  @each $k in x, y {
                    .#{$k} { @include new-mixin; }
                  }
                }
            "#]],
        );
        check_scss_mixin(
            "@include wrap {\n  .b {\n    color: red;\n  }\n}\n",
            "color: red;",
            &expect_test::expect![[r#"
                @mixin new-mixin {
                  color: red;
                }

                @include wrap {
                  .b {
                    @include new-mixin;
                  }
                }
            "#]],
        );
    }

    #[test]
    fn extract_mixin_inside_function_body_no_action() {
        // `@include` is not allowed in a function.
        let src = "@function foo() {\n  $x: 1;\n  @return $x;\n}\n";
        assert!(run_extract_mixin(src, (1, 2), (1, 8)).is_empty());
    }

    #[test]
    fn extract_var_multiline_expression() {
        let src = ".btn {\n  background: linear-gradient(\n    red,\n    blue\n  );\n}\n";
        // Select the function call "linear-gradient(\n    red,\n    blue\n  )"
        // line 1 col 14 to line 4 col 3
        let edits = run_extract_variable(src, (1, 14), (4, 3));
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("linear-gradient("));
        let result = apply_edits(src, &edits);
        let (_, errors) = sass_parser::parse_scss(&result);
        assert!(errors.is_empty(), "Parse errors in:\n{result}\n{errors:?}");
    }

    #[test]
    fn extract_mixin_tab_indentation() {
        let src = ".btn {\n\tdisplay: flex;\n}\n";
        let edits = run_extract_mixin(src, (1, 1), (1, 15));
        assert_eq!(edits.len(), 2);
        // Mixin body should use tab indent, not 2 spaces
        assert!(
            edits[0].new_text.contains('\t'),
            "Expected tab indent in mixin body"
        );
    }

    // ── normalize_extension_only ────────────────────────────────────

    #[test]
    fn normalize_extension_only_strips_scss() {
        assert_eq!(
            normalize_extension_only(Path::new("_colors.scss")),
            "_colors"
        );
    }

    #[test]
    fn normalize_extension_only_strips_sass() {
        assert_eq!(normalize_extension_only(Path::new("_base.sass")), "_base");
    }

    #[test]
    fn normalize_extension_only_strips_css() {
        assert_eq!(normalize_extension_only(Path::new("vendor.css")), "vendor");
    }

    #[test]
    fn normalize_extension_only_preserves_underscore() {
        assert_eq!(
            normalize_extension_only(Path::new("utils/_mixins.scss")),
            "utils/_mixins"
        );
    }

    #[test]
    fn normalize_extension_only_no_extension() {
        assert_eq!(normalize_extension_only(Path::new("module")), "module");
    }

    // ── path_to_forward_slash ───────────────────────────────────────

    #[test]
    fn forward_slash_unix_path_unchanged() {
        assert_eq!(
            path_to_forward_slash(Path::new("utils/mixins")),
            "utils/mixins"
        );
    }

    #[test]
    fn forward_slash_single_component() {
        assert_eq!(path_to_forward_slash(Path::new("colors")), "colors");
    }

    // ── unused_use_actions ──────────────────────────────────────────

    fn run_unused_use(
        src: &str,
        request_line_start: u32,
        request_line_end: u32,
    ) -> Vec<CodeAction> {
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let uri: Uri = "file:///test.scss".parse().unwrap();
        let file_symbols = Arc::new(crate::symbols::collect_symbols(&root));
        let import_refs = sass_parser::imports::collect_imports(&root);
        let request_range = Range::new(
            Position::new(request_line_start, 0),
            Position::new(request_line_end, 999),
        );
        let mut actions = Vec::new();
        unused_use_actions(
            &uri,
            &root,
            src,
            &li,
            &file_symbols,
            &import_refs,
            request_range,
            &mut actions,
        );
        actions
    }

    #[test]
    fn unused_use_offers_removal() {
        let src = "@use \"colors\";\n.btn { color: red; }\n";
        let actions = run_unused_use(src, 0, 0);
        assert_eq!(actions.len(), 1);
        assert!(actions[0].title.contains("Remove unused"));
        assert!(actions[0].title.contains("colors"));
    }

    #[test]
    fn unused_use_outside_request_range_no_action() {
        let src = "@use \"colors\";\n.btn { color: red; }\n";
        // Request range is on the .btn rule, not the @use line
        let actions = run_unused_use(src, 1, 1);
        assert!(actions.is_empty());
    }

    #[test]
    fn multiple_unused_uses_both_offered() {
        let src = "@use \"colors\";\n@use \"mixins\";\n.btn { color: red; }\n";
        let actions = run_unused_use(src, 0, 1);
        assert_eq!(actions.len(), 2);
        let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
        assert!(titles.iter().any(|t| t.contains("colors")));
        assert!(titles.iter().any(|t| t.contains("mixins")));
    }

    #[test]
    fn unused_use_star_skipped() {
        let src = "@use \"colors\" as *;\n.btn { color: red; }\n";
        let actions = run_unused_use(src, 0, 0);
        assert!(actions.is_empty());
    }

    // ── indentation_at ──────────────────────────────────────────────

    #[test]
    fn indentation_at_first_line() {
        assert_eq!(indentation_at("  .btn { }", 2), "  ");
    }

    #[test]
    fn indentation_at_second_line() {
        assert_eq!(indentation_at(".parent {\n    .child { }\n}", 14), "    ");
    }

    #[test]
    fn indentation_at_no_indent() {
        assert_eq!(indentation_at(".btn { }", 0), "");
    }

    // ── is_expression_node ──────────────────────────────────────────

    #[test]
    fn is_expression_node_known_kinds() {
        assert!(is_expression_node(SyntaxKind::NUMBER_LITERAL));
        assert!(is_expression_node(SyntaxKind::STRING_LITERAL));
        assert!(is_expression_node(SyntaxKind::BINARY_EXPR));
        assert!(is_expression_node(SyntaxKind::FUNCTION_CALL));
        assert!(is_expression_node(SyntaxKind::VARIABLE_REF));
        assert!(is_expression_node(SyntaxKind::MAP_EXPR));
    }

    #[test]
    fn is_expression_node_non_expr_kinds() {
        assert!(!is_expression_node(SyntaxKind::RULE_SET));
        assert!(!is_expression_node(SyntaxKind::DECLARATION));
        assert!(!is_expression_node(SyntaxKind::BLOCK));
        assert!(!is_expression_node(SyntaxKind::SOURCE_FILE));
    }

    // ── extract_name_from_message edge cases ────────────────────────

    #[test]
    fn extract_name_no_backticks_returns_none() {
        assert_eq!(extract_name_from_message("no backticks here"), None);
    }

    #[test]
    fn extract_name_single_backtick_returns_none() {
        assert_eq!(extract_name_from_message("only one ` backtick"), None);
    }

    #[test]
    fn extract_name_empty_between_backticks() {
        assert_eq!(extract_name_from_message("empty ``"), Some(String::new()));
    }

    // ── insertion point with @import ────────────────────────────────

    #[test]
    fn insertion_point_after_import_rule() {
        let src = "@import \"legacy\";\n.btn { }\n";
        let (green, _) = sass_parser::parse_scss(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(1, 0));
        assert!(!nl);
    }

    // ── extract variable: custom property ───────────────────────────

    #[test]
    fn extract_var_custom_property_value() {
        let src = ".btn { --my-color: #ff0; }\n";
        // Select #ff0 → bytes 19..23
        // Custom property values are inside CUSTOM_PROPERTY_DECL, not VALUE —
        // extract variable is not supported for this node type.
        let edits = run_extract_variable(src, (0, 19), (0, 23));
        assert!(edits.is_empty());
    }

    // ── extract mixin: custom property decl ─────────────────────────

    #[test]
    fn extract_mixin_custom_property_decl() {
        let src = ".btn {\n  --gap: 10px;\n}\n";
        let edits = run_extract_mixin(src, (1, 2), (1, 14));
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("--gap: 10px;"));
    }

    fn check_scss_mixin(src: &str, selected: &str, expect: &expect_test::Expect) {
        let start = src.find(selected).expect("selection not found in source");
        #[allow(clippy::cast_possible_truncation)]
        let sel = TextRange::new(
            TextSize::from(start as u32),
            TextSize::from((start + selected.len()) as u32),
        );
        let li = sass_parser::line_index::LineIndex::new(src);
        let range = text_range_to_lsp(sel, &li, src);
        let edits = run_extract_mixin(
            src,
            (range.start.line, range.start.character),
            (range.end.line, range.end.character),
        );
        assert!(!edits.is_empty(), "no code action offered");
        let result = apply_edits(src, &edits);
        let (_, errors) = sass_parser::parse_scss(&result);
        assert!(errors.is_empty(), "parse errors in:\n{result}\n{errors:?}");
        expect.assert_eq(&result);
    }

    #[test]
    fn extract_mixin_moves_nested_rule_between_decls() {
        check_scss_mixin(
            ".a {\n  color: red;\n  .b { x: y; }\n  margin: 0;\n}\n",
            "color: red;\n  .b { x: y; }\n  margin: 0;",
            &expect_test::expect![[r#"
                @mixin new-mixin {
                  color: red;
                  .b { x: y; }
                  margin: 0;
                }

                .a {
                  @include new-mixin;
                }
            "#]],
        );
    }

    #[test]
    fn extract_mixin_moves_every_statement_kind_and_keeps_nesting() {
        check_scss_mixin(
            ".a {\n  color: red;\n  &:hover {\n    x: y;\n  }\n  /* note */\n  @include other;\n  @media print { display: none; }\n\n  font: {\n    family: x;\n  }\n  @extend %ph;\n  margin: 0;\n}\n",
            "color: red;\n  &:hover {\n    x: y;\n  }\n  /* note */\n  @include other;\n  @media print { display: none; }\n\n  font: {\n    family: x;\n  }\n  @extend %ph;\n  margin: 0;",
            &expect_test::expect![[r#"
                @mixin new-mixin {
                  color: red;
                  &:hover {
                    x: y;
                  }
                  /* note */
                  @include other;
                  @media print { display: none; }

                  font: {
                    family: x;
                  }
                  @extend %ph;
                  margin: 0;
                }

                .a {
                  @include new-mixin;
                }
            "#]],
        );
    }

    #[test]
    fn extract_mixin_whole_line_selection_with_comments() {
        // Comments wholly inside the selection move; whitespace around it is
        // ignored, so selecting full lines works.
        check_scss_mixin(
            ".a {\n  // heading\n  color: red; // trailing\n  margin: 0;\n}\n",
            "  // heading\n  color: red; // trailing\n",
            &expect_test::expect![[r#"
                @mixin new-mixin {
                  // heading
                  color: red; // trailing
                }

                .a {
                  @include new-mixin;
                  margin: 0;
                }
            "#]],
        );
    }

    #[test]
    fn extract_mixin_comment_cut_by_selection_stays() {
        check_scss_mixin(
            ".a {\n  /* a long note */ color: red;\n}\n",
            "note */ color: red;",
            &expect_test::expect![[r#"
                @mixin new-mixin {
                  color: red;
                }

                .a {
                  /* a long note */ @include new-mixin;
                }
            "#]],
        );
    }

    #[test]
    fn extract_mixin_single_line_block() {
        check_scss_mixin(
            ".a { color: red; .b { x: y; } margin: 0 }\n",
            "color: red; .b { x: y; }",
            &expect_test::expect![[r#"
                @mixin new-mixin {
                  color: red; .b { x: y; }
                }

                .a { @include new-mixin; margin: 0 }
            "#]],
        );
    }

    #[test]
    fn extract_mixin_partial_statement_no_action() {
        let src = ".a {\n  color: red;\n  .b { x: y; }\n  margin: 0;\n}\n";
        // Ends inside `.b { … }`.
        assert!(run_extract_mixin(src, (1, 2), (2, 8)).is_empty());
        // Starts inside `color: red;`.
        assert!(run_extract_mixin(src, (1, 4), (3, 12)).is_empty());
    }

    #[test]
    fn extract_mixin_selection_crossing_closing_brace_no_action() {
        let src = ".a {\n  .b {\n    x: y;\n  }\n  margin: 0;\n}\n.c { }\n";
        // From inside `.b` past its `}`.
        assert!(run_extract_mixin(src, (2, 4), (4, 12)).is_empty());
        // From inside `.a` past its `}`.
        assert!(run_extract_mixin(src, (4, 2), (6, 3)).is_empty());
        // The braces themselves.
        assert!(run_extract_mixin(src, (0, 3), (5, 1)).is_empty());
    }

    #[test]
    fn extract_mixin_content_or_nested_mixin_no_action() {
        let src = "@mixin m {\n  color: red;\n  @content;\n}\n";
        assert!(run_extract_mixin(src, (1, 2), (2, 11)).is_empty());
        let src = ".a {\n  color: red;\n  .b { @content; }\n}\n";
        assert!(run_extract_mixin(src, (1, 2), (2, 18)).is_empty());
    }

    #[test]
    fn extract_mixin_comment_only_selection_no_action() {
        let src = ".a {\n  // just a note\n  color: red;\n}\n";
        assert!(run_extract_mixin(src, (1, 2), (1, 16)).is_empty());
    }

    // ── indented Sass ───────────────────────────────────────────────

    type ExtractAction = fn(
        &Uri,
        &SyntaxNode,
        &str,
        &sass_parser::line_index::LineIndex,
        Range,
        &mut Vec<CodeAction>,
    );

    /// Run `action` on a `.sass` document with `selected` (its first occurrence)
    /// as the selection, and apply the resulting edits. Asserts that both the
    /// input and the result parse as indented Sass with no errors.
    fn apply_in_sass(action: ExtractAction, src: &str, selected: &str) -> Option<String> {
        let (green, errors) = sass_parser::parse_sass(src);
        assert!(errors.is_empty(), "input has parse errors: {errors:?}");
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let uri: Uri = "file:///test.sass".parse().unwrap();
        let start = src.find(selected).expect("selection not found in source");
        #[allow(clippy::cast_possible_truncation)]
        let sel = TextRange::new(
            TextSize::from(start as u32),
            TextSize::from((start + selected.len()) as u32),
        );
        let mut actions = Vec::new();
        action(
            &uri,
            &root,
            src,
            &li,
            text_range_to_lsp(sel, &li, src),
            &mut actions,
        );
        let edits: Vec<TextEdit> = actions
            .into_iter()
            .flat_map(|a| a.edit.unwrap().changes.unwrap().into_values().flatten())
            .collect();
        if edits.is_empty() {
            return None;
        }
        let result = apply_edits(src, &edits);
        let (_, errors) = sass_parser::parse_sass(&result);
        assert!(errors.is_empty(), "parse errors in:\n{result}\n{errors:?}");
        Some(result)
    }

    fn check_sass(action: ExtractAction, src: &str, selected: &str, expect: &expect_test::Expect) {
        let result = apply_in_sass(action, src, selected).expect("no code action offered");
        expect.assert_eq(&result);
    }

    #[test]
    fn statement_terminator_follows_the_extension() {
        let sass: Uri = "file:///a/_b.sass".parse().unwrap();
        let scss: Uri = "file:///a/_b.scss".parse().unwrap();
        assert_eq!(statement_terminator(&sass), "");
        assert_eq!(statement_terminator(&scss), ";");
    }

    #[test]
    fn sass_extract_var_in_rule() {
        check_sass(
            extract_variable_action,
            ".btn\n  color: #333\n",
            "#333",
            &expect_test::expect![[r"
                $new-variable: #333
                .btn
                  color: $new-variable
            "]],
        );
    }

    #[test]
    fn sass_extract_var_nested_rule_keeps_indentation() {
        check_sass(
            extract_variable_action,
            ".parent\n    .child\n        color: #fff\n",
            "#fff",
            &expect_test::expect![[r"
                .parent
                    $new-variable: #fff
                    .child
                        color: $new-variable
            "]],
        );
    }

    #[test]
    fn sass_extract_var_top_level_decl() {
        check_sass(
            extract_variable_action,
            "$color: #333\n",
            "#333",
            &expect_test::expect![[r"
                $new-variable: #333
                $color: $new-variable
            "]],
        );
    }

    #[test]
    fn sass_extract_var_in_tab_indented_mixin() {
        let result = apply_in_sass(
            extract_variable_action,
            "@mixin box\n\twidth: 100px * 2\n",
            "100px * 2",
        );
        assert_eq!(
            result.as_deref(),
            Some("$new-variable: 100px * 2\n@mixin box\n\twidth: $new-variable\n")
        );
    }

    #[test]
    fn sass_extract_var_multiline_expression() {
        check_sass(
            extract_variable_action,
            ".btn\n  background: linear-gradient(\n    red,\n    blue\n  )\n",
            "linear-gradient(\n    red,\n    blue\n  )",
            &expect_test::expect![[r"
                $new-variable: linear-gradient(
                    red,
                    blue
                  )
                .btn
                  background: $new-variable
            "]],
        );
    }

    #[test]
    fn sass_extract_var_selector_no_action() {
        let src = ".btn\n  color: red\n";
        assert_eq!(apply_in_sass(extract_variable_action, src, ".btn"), None);
    }

    #[test]
    fn sass_extract_mixin_single_decl() {
        check_sass(
            extract_mixin_action,
            ".btn\n  display: flex\n",
            "display: flex",
            &expect_test::expect![[r"
                @mixin new-mixin
                  display: flex

                .btn
                  @include new-mixin
            "]],
        );
    }

    #[test]
    fn sass_extract_mixin_nested_rule_four_space_unit() {
        check_sass(
            extract_mixin_action,
            ".parent\n    .child\n        display: flex\n        color: red\n    width: 1px\n",
            "display: flex\n        color: red",
            &expect_test::expect![[r"
                .parent
                    @mixin new-mixin
                        display: flex
                        color: red

                    .child
                        @include new-mixin
                    width: 1px
            "]],
        );
    }

    #[test]
    fn sass_extract_mixin_tab_unit() {
        let result = apply_in_sass(
            extract_mixin_action,
            ".btn\n\tdisplay: flex\n\tcolor: red\n",
            "display: flex\n\tcolor: red",
        );
        assert_eq!(
            result.as_deref(),
            Some("@mixin new-mixin\n\tdisplay: flex\n\tcolor: red\n\n.btn\n\t@include new-mixin\n")
        );
    }

    #[test]
    fn sass_extract_mixin_unit_comes_from_the_enclosing_block() {
        // The mixin goes before `.a`, two levels above the declaration. Its body
        // is still one unit deep, not two.
        check_sass(
            extract_mixin_action,
            ".a\n  @media print\n    color: red\n",
            "color: red",
            &expect_test::expect![[r"
                @mixin new-mixin
                  color: red

                .a
                  @media print
                    @include new-mixin
            "]],
        );
    }

    #[test]
    fn sass_extract_mixin_keeps_relative_indentation() {
        // Continuation lines of a multi-line value and anything between the
        // selected declarations move with them, one level under the mixin.
        check_sass(
            extract_mixin_action,
            ".a\n  color: red\n  // keep me\n  font: 12px\n    family: x\n  background: linear-gradient(\n    red,\n    blue\n  )\n  width: 1px\n",
            "color: red\n  // keep me\n  font: 12px\n    family: x\n  background: linear-gradient(\n    red,\n    blue\n  )",
            &expect_test::expect![[r"
                @mixin new-mixin
                  color: red
                  // keep me
                  font: 12px
                    family: x
                  background: linear-gradient(
                    red,
                    blue
                  )

                .a
                  @include new-mixin
                  width: 1px
            "]],
        );
    }

    #[test]
    fn sass_extract_mixin_follows_shorthand_style() {
        check_sass(
            extract_mixin_action,
            "=box\n  width: 1px\n.a\n  +box\n  color: red\n",
            "color: red",
            &expect_test::expect![[r"
                =box
                  width: 1px
                =new-mixin
                  color: red

                .a
                  +box
                  +new-mixin
            "]],
        );
    }

    #[test]
    fn sass_extract_mixin_shorthands_are_detected_independently() {
        check_sass(
            extract_mixin_action,
            "@mixin box\n  width: 1px\n.a\n  +box\n  color: red\n",
            "color: red",
            &expect_test::expect![[r"
                @mixin box
                  width: 1px
                @mixin new-mixin
                  color: red

                .a
                  +box
                  +new-mixin
            "]],
        );
    }

    #[test]
    fn sass_extract_mixin_moves_nested_rule_between_decls() {
        check_sass(
            extract_mixin_action,
            ".a\n  color: red\n  &:hover\n    x: y\n  +other\n  margin: 0\n  width: 1px\n",
            "color: red\n  &:hover\n    x: y\n  +other\n  margin: 0",
            &expect_test::expect![[r#"
                @mixin new-mixin
                  color: red
                  &:hover
                    x: y
                  +other
                  margin: 0

                .a
                  +new-mixin
                  width: 1px
            "#]],
        );
    }

    #[test]
    fn sass_extract_mixin_nested_property_uses_long_include() {
        // Dart Sass reads `+new-mixin` in a nested property's block as a
        // malformed declaration.
        check_sass(
            extract_mixin_action,
            ".a\n  +b\n  font:\n    family: x\n    size: 1px\n",
            "family: x",
            &expect_test::expect![[r#"
                @mixin new-mixin
                  family: x

                .a
                  +b
                  font:
                    @include new-mixin
                    size: 1px
            "#]],
        );
    }

    #[test]
    fn sass_extract_mixin_partial_statement_no_action() {
        let src = ".a\n  color: red\n  .b\n    x: y\n  margin: 0\n";
        assert_eq!(
            apply_in_sass(extract_mixin_action, src, "color: red\n  .b"),
            None
        );
    }

    #[test]
    fn sass_extract_mixin_selection_not_on_decl_no_action() {
        let src = ".btn\n  display: flex\n";
        assert_eq!(apply_in_sass(extract_mixin_action, src, ".btn"), None);
    }

    const INVARIANT_SCSS: &[&str] = &[
        "@use \"sass:math\";\n\n%reset { margin: 0; }\n\n@mixin other($x: 1px) {\n  padding: $x;\n  @content;\n}\n\n.card {\n  $gap: 8px;\n  display: flex; // layout\n  gap: $gap;\n  &:hover {\n    color: red;\n    .icon & { opacity: 0.5; }\n  }\n\n  /* spacing */\n  @include other(2px) {\n    border: 0;\n  }\n  @media (min-width: 600px) {\n    gap: math.div($gap, 2);\n  }\n  font: {\n    family: sans-serif;\n    size: 12px;\n  }\n  @extend %reset;\n  --accent: #{$gap};\n  margin: 0 auto !important;\n}\n",
        "@mixin box {\n  .inner {\n    width: 1px;\n    @if $a == 1 { height: 2px; } @else { height: 3px; }\n    @each $k in a, b {\n      .#{$k} { color: $k; }\n    }\n  }\n}\n.nav { ul { li { a { color: blue; text-decoration: none; } } } }\n",
    ];

    const INVARIANT_SASS: &[&str] = &[
        "@use \"sass:math\"\n\n%reset\n  margin: 0\n\n=other($x: 1px)\n  padding: $x\n  @content\n\n.card\n  $gap: 8px\n  display: flex\n  // layout\n  gap: $gap\n  &:hover\n    color: red\n    .icon &\n      opacity: 0.5\n\n  /* spacing */\n  +other(2px)\n    border: 0\n  @media (min-width: 600px)\n    gap: math.div($gap, 2)\n  font:\n    family: sans-serif\n    size: 12px\n  @extend %reset\n  margin: 0 auto !important\n",
        "@mixin box\n    .inner\n        width: 1px\n        @if $a == 1\n            height: 2px\n        @else\n            height: 3px\n        background: linear-gradient(\n            red,\n            blue\n        )\n.nav\n    ul\n        li\n            color: blue\n",
    ];

    /// Byte offsets a user might plausibly start or end a selection at: every
    /// token boundary, the middle of every longer token and every line end.
    fn selection_offsets(root: &SyntaxNode, src: &str) -> Vec<usize> {
        let mut offsets: Vec<usize> = root
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .flat_map(|t| {
                let r = t.text_range();
                let (start, end) = (usize::from(r.start()), usize::from(r.end()));
                let mid = (start..=start + (end - start) / 2)
                    .rev()
                    .find(|&i| src.is_char_boundary(i))
                    .unwrap_or(start);
                [start, mid, end]
            })
            .chain(src.match_indices('\n').map(|(i, _)| i))
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        offsets
    }

    fn sorted_visible_chars(parts: &[&str]) -> Vec<char> {
        let mut chars: Vec<char> = parts
            .iter()
            .flat_map(|p| p.chars())
            .filter(|c| !c.is_whitespace())
            .collect();
        chars.sort_unstable();
        chars
    }

    /// For every selection over a few realistic documents, an offered "Extract
    /// to mixin" must keep every visible character of the document — adding
    /// only the mixin's header and the `@include` — and leave a document that
    /// parses without errors.
    #[test]
    fn extract_mixin_never_loses_text() {
        type Parse = fn(&str) -> (sass_parser::syntax::GreenNode, Vec<(String, TextRange)>);
        let cases = INVARIANT_SCSS
            .iter()
            .map(|s| (*s, "file:///t.scss", sass_parser::parse_scss as Parse))
            .chain(
                INVARIANT_SASS
                    .iter()
                    .map(|s| (*s, "file:///t.sass", sass_parser::parse_sass as Parse)),
            );

        let (mut tried, mut offered) = (0usize, 0usize);
        for (src, uri, parse) in cases {
            let (green, errors) = parse(src);
            assert!(
                errors.is_empty(),
                "input has parse errors: {errors:?}\n{src}"
            );
            let root = SyntaxNode::new_root(green);
            let li = sass_parser::line_index::LineIndex::new(src);
            let uri: Uri = uri.parse().unwrap();
            let offsets = selection_offsets(&root, src);

            for (i, &start) in offsets.iter().enumerate() {
                for &end in &offsets[i + 1..] {
                    tried += 1;
                    #[allow(clippy::cast_possible_truncation)]
                    let sel =
                        TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
                    let mut actions = Vec::new();
                    let range = text_range_to_lsp(sel, &li, src);
                    extract_mixin_action(&uri, &root, src, &li, range, &mut actions);
                    for action in actions {
                        offered += 1;
                        let edits: Vec<TextEdit> = action
                            .edit
                            .unwrap()
                            .changes
                            .unwrap()
                            .into_values()
                            .flatten()
                            .collect();
                        let result = apply_edits(src, &edits);
                        let added: Vec<&str> = edits
                            .iter()
                            .flat_map(|e| e.new_text.lines())
                            .filter(|l| l.contains("new-mixin"))
                            .collect();
                        let mut scaffold = added.join("");
                        if !crate::worker::is_sass_file(&uri) {
                            scaffold.push('}');
                        }
                        let context =
                            format!("selection {:?}\n--- result ---\n{result}", &src[start..end]);
                        assert_eq!(
                            sorted_visible_chars(&[&result]),
                            sorted_visible_chars(&[src, &scaffold]),
                            "visible text changed for {context}"
                        );
                        let (_, errors) = parse(&result);
                        assert!(errors.is_empty(), "parse errors {errors:?} for {context}");
                    }
                }
            }
        }
        assert!(tried > 50_000, "only {tried} selections tried");
        assert!(offered > 500, "only {offered} actions offered");
    }

    #[test]
    fn detect_indent_unit_first_indented_line() {
        assert_eq!(detect_indent_unit("// c\n\n.a\n   b: c\n  d: e\n"), "   ");
        assert_eq!(detect_indent_unit(".a\n\tb: c\n"), "\t");
        assert_eq!(detect_indent_unit("$a: 1\n"), "  ");
    }
}
