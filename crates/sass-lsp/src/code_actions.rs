use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use sass_parser::imports::{ImportKind, collect_imports};
use sass_parser::syntax::SyntaxNode;
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
        let use_statement = format!("{prefix}@use \"{use_path}\";\n");

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
        rowan::NodeOrToken::Node(n) => n,
        rowan::NodeOrToken::Token(t) => t.parent()?,
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
            rowan::NodeOrToken::Token(t) if !t.kind().is_trivia() => Some(t.text_range().start()),
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

    let var_decl = format!("{indent}$new-variable: {expr_text};\n");
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

fn is_declaration_node(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::DECLARATION | SyntaxKind::VARIABLE_DECL | SyntaxKind::CUSTOM_PROPERTY_DECL
    )
}

fn find_declarations_at_range(
    root: &SyntaxNode,
    range: TextRange,
) -> Option<(SyntaxNode, Vec<SyntaxNode>)> {
    let elem = root.covering_element(range);
    let start_node = match elem {
        rowan::NodeOrToken::Node(n) => n,
        rowan::NodeOrToken::Token(t) => t.parent()?,
    };

    // Find enclosing BLOCK
    let block = start_node
        .ancestors()
        .find(|a| a.kind() == SyntaxKind::BLOCK)?;

    // Use content ranges (skipping leading trivia) for containment checks,
    // since users select visible text, not the trivia rowan attaches to nodes.
    let decls: Vec<SyntaxNode> = block
        .children()
        .filter(|c| is_declaration_node(c.kind()))
        .filter(|c| {
            let cr = content_range(c);
            cr.start() >= range.start() && cr.end() <= range.end()
        })
        .collect();

    if decls.is_empty() {
        return None;
    }

    // Reject if selection partially overlaps any declaration
    for child in block.children() {
        if !is_declaration_node(child.kind()) {
            continue;
        }
        let cr = content_range(&child);
        let overlaps = cr.start() < range.end() && cr.end() > range.start();
        let fully_contained = cr.start() >= range.start() && cr.end() <= range.end();
        if overlaps && !fully_contained {
            return None;
        }
    }

    Some((block, decls))
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

    let Some((block, decls)) = find_declarations_at_range(root, sel_range) else {
        return;
    };

    // Find the enclosing rule/at-rule to insert the mixin before
    let Some(rule_set) = block.ancestors().find(|a| {
        matches!(
            a.kind(),
            SyntaxKind::RULE_SET
                | SyntaxKind::MIXIN_RULE
                | SyntaxKind::FUNCTION_RULE
                | SyntaxKind::EACH_RULE
                | SyntaxKind::FOR_RULE
                | SyntaxKind::WHILE_RULE
                | SyntaxKind::IF_RULE
        )
    }) else {
        return;
    };

    let rule_indent = node_indentation(text, &rule_set);

    // Infer indent step from the first declaration's extra indent over the rule set.
    // Falls back to 2 spaces if the declaration is at the same level (shouldn't happen).
    let first_decl_indent = node_indentation(text, decls.first().unwrap());
    let indent_step = first_decl_indent
        .strip_prefix(rule_indent)
        .filter(|s| !s.is_empty())
        .unwrap_or("  ");
    let body_indent = format!("{rule_indent}{indent_step}");
    let mut body_lines = Vec::new();
    for decl in &decls {
        let decl_text = decl.text().to_string();
        // Trim each line's existing indent and re-indent with body_indent
        for line in decl_text.lines() {
            let trimmed = line.trim_start();
            if !trimmed.is_empty() {
                body_lines.push(format!("{body_indent}{trimmed}"));
            }
        }
    }
    let body = body_lines.join("\n");

    let mixin_text = format!("{rule_indent}@mixin new-mixin {{\n{body}\n{rule_indent}}}\n\n");

    let first_decl = decls.first().unwrap();
    let last_decl = decls.last().unwrap();
    let first_content_start = content_range(first_decl).start();
    let last_end = last_decl.text_range().end();

    let replace_range = TextRange::new(first_content_start, last_end);
    let replace_lsp = text_range_to_lsp(replace_range, line_index, text);
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
            new_text: "@include new-mixin;".to_owned(),
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
        let (green, _) = sass_parser::parse("");
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
        let (green, _) = sass_parser::parse(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(1, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_after_multiple_uses() {
        let src = "@use \"colors\";\n@use \"mixins\";\n.btn { }\n";
        let (green, _) = sass_parser::parse(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(2, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_after_forward() {
        let src = "@forward \"colors\";\n@use \"mixins\";\n.btn { }\n";
        let (green, _) = sass_parser::parse(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(2, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_no_imports_with_rules() {
        let src = ".btn { color: red; }\n";
        let (green, _) = sass_parser::parse(src);
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
        let (green, _) = sass_parser::parse(src);
        let root = SyntaxNode::new_root(green);
        let li = sass_parser::line_index::LineIndex::new(src);
        let (pos, nl) = find_use_insertion_point(&root, src, &li);
        assert_eq!(pos, Position::new(3, 0));
        assert!(!nl);
    }

    #[test]
    fn insertion_point_no_trailing_newline_needs_prefix() {
        let src = "@use \"colors\";";
        let (green, _) = sass_parser::parse(src);
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
        let (green, _) = sass_parser::parse(src);
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
        let (green, _) = sass_parser::parse(src);
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

    /// Apply sorted, non-overlapping LSP TextEdits to source text (last-to-first).
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
        let (_, errors) = sass_parser::parse(&result);
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
        let (_, errors) = sass_parser::parse(&result);
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
        let src = "@mixin outer {\n  .inner {\n    color: red;\n  }\n}\n";
        // Select "color: red;" inside .inner which is inside @mixin outer
        let edits = run_extract_mixin(src, (2, 4), (2, 15));
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("@mixin new-mixin"));
        let result = apply_edits(src, &edits);
        let (_, errors) = sass_parser::parse(&result);
        assert!(errors.is_empty(), "Parse errors in:\n{result}\n{errors:?}");
    }

    #[test]
    fn extract_mixin_inside_function_body() {
        // Functions can't have rule sets, so this should produce no action
        let src = "@function foo() {\n  $x: 1;\n  @return $x;\n}\n";
        let edits = run_extract_mixin(src, (1, 2), (1, 8));
        // VARIABLE_DECL is inside @function block but no RULE_SET ancestor —
        // the enclosing rule is FUNCTION_RULE, so mixin is inserted before it
        assert_eq!(edits.len(), 2);
        assert!(edits[0].new_text.contains("@mixin new-mixin"));
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
        let (_, errors) = sass_parser::parse(&result);
        assert!(errors.is_empty(), "Parse errors in:\n{result}\n{errors:?}");
    }

    #[test]
    fn extract_mixin_tab_indentation() {
        let src = ".btn {\n\tdisplay: flex;\n}\n";
        let edits = run_extract_mixin(src, (1, 1), (1, 15));
        assert_eq!(edits.len(), 2);
        // Mixin body should use tab indent, not 2 spaces
        assert!(
            edits[0].new_text.contains("\t"),
            "Expected tab indent in mixin body"
        );
    }
}
