use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use sass_parser::imports::{ImportKind, collect_imports};
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionParams, Diagnostic, NumberOrString, Position, Range,
    TextEdit, Uri, WorkspaceEdit,
};

use crate::DocumentState;
use crate::ast_helpers;
use crate::convert::text_range_to_lsp;
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

    let is_first_action = actions.is_empty();
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
            is_preferred: Some(is_first_action && actions.is_empty()),
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
    if let Some(resolved_uri) = module_graph.resolve_import(from_uri, &spec) {
        if resolved_uri == *target_uri {
            return Some(spec);
        }
    }

    // Fallback: try without stripping _ (non-partial files)
    let spec2 = normalize_extension_only(&rel);
    if spec2 != spec {
        if let Some(resolved_uri) = module_graph.resolve_import(from_uri, &spec2) {
            if resolved_uri == *target_uri {
                return Some(spec2);
            }
        }
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
}
