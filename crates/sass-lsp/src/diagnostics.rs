use sass_parser::syntax::{GreenNode, NodeOrToken, SyntaxNode};
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::{TextRange, TextSize};
use tower_lsp_server::ls_types::{DiagnosticSeverity, Uri};

use crate::ast_helpers;
use crate::symbols::{FileSymbols, SymbolKind};
use crate::workspace;

pub(crate) struct SemanticDiagnostic {
    pub(crate) range: TextRange,
    pub(crate) message: String,
    pub(crate) severity: DiagnosticSeverity,
    pub(crate) code: &'static str,
    /// Structured context for code actions (serialized as JSON in LSP diagnostic `data`).
    pub(crate) data: Option<serde_json::Value>,
}

pub(crate) fn check_file(
    uri: &Uri,
    symbols: &FileSymbols,
    module_graph: &workspace::ModuleGraph,
    green: &GreenNode,
    has_parse_errors: bool,
) -> Vec<SemanticDiagnostic> {
    let root = SyntaxNode::new_root(green.clone());
    let mut result = Vec::new();
    let suppress_undefined = has_parse_errors || should_suppress_undefined(uri, module_graph);

    check_missing_imports(uri, module_graph, &mut result);
    check_arg_count(&root, symbols, module_graph, uri, &mut result);
    if !suppress_undefined {
        check_undefined(&root, symbols, module_graph, uri, &mut result);
    }
    if crate::worker::is_sass_file(uri) {
        check_colon_selectors(&root, &mut result);
    }

    result
}

// ── Missing stylesheets ─────────────────────────────────────────────

/// An import of a stylesheet that does not exist, reported as Dart Sass reports it.
/// The undefined-name warnings it would cause stay suppressed, so the one cause gives
/// one diagnostic.
fn check_missing_imports(
    uri: &Uri,
    module_graph: &workspace::ModuleGraph,
    out: &mut Vec<SemanticDiagnostic>,
) {
    for range in module_graph.missing_imports(uri) {
        out.push(SemanticDiagnostic {
            range,
            message: "Can't find stylesheet to import.".to_owned(),
            severity: DiagnosticSeverity::ERROR,
            code: "unresolved-import",
            data: None,
        });
    }
}

// ── Colon rules in indented Sass ────────────────────────────────────

/// A nested rule with no block whose selector reads `name:value`, as `color:red`: the
/// indented syntax reads it as a selector, as Dart Sass does, and the empty rule compiles
/// to nothing, so a declaration with its space after `:` missing is far more likely.
///
/// A `value` that names a pseudo-class or a legacy single-colon pseudo-element, as in
/// `a:hover` or `a:not(.b)`, is a selector written as one and is left alone, and so is
/// `::`. A vendor-prefixed value is taken as a pseudo-class unless `name` is a CSS
/// property, as `-webkit-autofill` after `input` but not `-webkit-box` after `display`.
/// Parentheses alone prove nothing: `width:calc(1px)` is a declaration.
fn check_colon_selectors(root: &SyntaxNode, out: &mut Vec<SemanticDiagnostic>) {
    for rule in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::RULE_SET)
    {
        if rule
            .parent()
            .is_none_or(|parent| parent.kind() != SyntaxKind::BLOCK)
            || rule
                .children()
                .any(|child| child.kind() == SyntaxKind::BLOCK)
        {
            continue;
        }
        let Some(list) = rule.first_child() else {
            continue;
        };
        let mut selectors = list.children();
        let (Some(selector), None) = (selectors.next(), selectors.next()) else {
            continue;
        };
        let parts: Vec<SyntaxNode> = selector.children().collect();
        let Some((pseudo, name_parts)) = parts.split_last() else {
            continue;
        };
        if name_parts.is_empty()
            || !name_parts.iter().all(reads_as_property_name)
            || pseudo.kind() != SyntaxKind::PSEUDO_SELECTOR
            || pseudo
                .first_token()
                .is_none_or(|t| t.kind() != SyntaxKind::COLON)
        {
            continue;
        }
        let selector_text = selector.text().to_string();
        let name_len = usize::from(pseudo.text_range().start() - selector.text_range().start());
        let name = selector_text[..name_len].trim_start();
        if name.contains(char::is_whitespace) || names_pseudo_selector(name, pseudo) {
            continue;
        }
        let text = list.text().to_string();
        let start =
            list.text_range().start() + TextSize::of(&text[..text.len() - text.trim_start().len()]);
        let end = list.text_range().end() - TextSize::of(&text[text.trim_end().len()..]);
        out.push(SemanticDiagnostic {
            range: TextRange::new(start, end),
            message: format!(
                "`{}` is read as a selector, not a declaration: a space is likely missing \
                 after `:`",
                text.trim()
            ),
            severity: DiagnosticSeverity::WARNING,
            code: "colon-selector",
            data: None,
        });
    }
}

/// A selector part that a declaration could have as (part of) its property: a plain
/// name, or an interpolation.
fn reads_as_property_name(part: &SyntaxNode) -> bool {
    match part.kind() {
        SyntaxKind::INTERPOLATION => true,
        SyntaxKind::SIMPLE_SELECTOR => {
            let mut tokens = part
                .children_with_tokens()
                .filter(|element| !element.kind().is_trivia());
            matches!(
                (tokens.next(), tokens.next()),
                (Some(NodeOrToken::Token(token)), None) if token.kind() == SyntaxKind::IDENT
            )
        }
        _ => false,
    }
}

/// Whether `pseudo`, the `:value` after the selector `name`, is a pseudo selector.
fn names_pseudo_selector(name: &str, pseudo: &SyntaxNode) -> bool {
    let Some(value) = pseudo
        .children_with_tokens()
        .filter(|element| !element.kind().is_trivia())
        .nth(1)
        .and_then(NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::IDENT)
    else {
        return false;
    };
    let value = value.text().to_ascii_lowercase();
    if value.starts_with('-') {
        return crate::css_properties::CSS_PROPERTIES
            .binary_search(&name.to_ascii_lowercase().as_str())
            .is_err();
    }
    PSEUDO_SELECTORS.binary_search(&value.as_str()).is_ok()
}

/// Pseudo-classes, and the pseudo-elements browsers also accept after a single colon.
/// Sorted. The `@page` ones (`:left`, `:right`, `:first`) are left out: in a style
/// rule they read as `float:left` with its space missing.
#[rustfmt::skip]
const PSEUDO_SELECTORS: &[&str] = &[
    "active", "after", "any", "any-link", "autofill", "backdrop", "before", "blank",
    "buffering", "checked", "closed", "current", "default", "defined", "dir", "disabled",
    "empty", "enabled", "first-child", "first-letter", "first-line", "first-of-type",
    "focus", "focus-visible", "focus-within", "fullscreen", "future", "global", "has",
    "host", "host-context", "hover", "in-range", "indeterminate", "invalid", "is", "lang",
    "last-child", "last-of-type", "link", "local", "local-link", "marker", "matches",
    "modal", "muted", "not", "nth-child", "nth-col", "nth-last-child", "nth-last-col",
    "nth-last-of-type", "nth-of-type", "only-child", "only-of-type", "open", "optional",
    "out-of-range", "part", "past", "paused", "picture-in-picture", "placeholder",
    "placeholder-shown", "playing", "popover-open", "read-only", "read-write", "required",
    "root", "scope", "seeking", "selection", "slotted", "stalled", "state", "target",
    "target-within", "user-invalid", "user-valid", "valid", "visited", "volume-locked",
    "where",
];

// ── Argument count checking ─────────────────────────────────────────

fn check_arg_count(
    root: &SyntaxNode,
    symbols: &FileSymbols,
    module_graph: &workspace::ModuleGraph,
    uri: &Uri,
    out: &mut Vec<SemanticDiagnostic>,
) {
    for sym_ref in &symbols.references {
        let kind = match sym_ref.kind {
            SymbolKind::Function => SymbolKind::Function,
            SymbolKind::Mixin => SymbolKind::Mixin,
            _ => continue,
        };

        let namespace = ast_helpers::namespace_of_ref(root, sym_ref.range);
        let Some((_, target)) =
            module_graph.resolve_reference(uri, namespace.as_deref(), &sym_ref.name, kind)
        else {
            continue;
        };

        let Some(param_info) = param_info_from_cst(&target) else {
            continue;
        };

        let Some(arg_count) = count_call_args(root, sym_ref.range, sym_ref.kind) else {
            continue;
        };

        if arg_count < param_info.required {
            out.push(SemanticDiagnostic {
                range: sym_ref.selection_range,
                message: format!(
                    "expected at least {} argument{}, but got {}",
                    param_info.required,
                    if param_info.required == 1 { "" } else { "s" },
                    arg_count,
                ),
                severity: DiagnosticSeverity::ERROR,
                code: "wrong-arg-count",
                data: None,
            });
        } else if !param_info.has_rest && arg_count > param_info.total {
            out.push(SemanticDiagnostic {
                range: sym_ref.selection_range,
                message: format!(
                    "expected at most {} argument{}, but got {}",
                    param_info.total,
                    if param_info.total == 1 { "" } else { "s" },
                    arg_count,
                ),
                severity: DiagnosticSeverity::ERROR,
                code: "wrong-arg-count",
                data: None,
            });
        }
    }
}

struct ParamInfo {
    required: u32,
    total: u32,
    has_rest: bool,
}

fn param_info_from_cst(target: &crate::symbols::Symbol) -> Option<ParamInfo> {
    let params_text = target.params.as_deref()?;
    Some(parse_param_info(params_text))
}

#[allow(clippy::cast_possible_truncation)]
fn parse_param_info(params_text: &str) -> ParamInfo {
    let inner = params_text
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(params_text);

    if inner.trim().is_empty() {
        return ParamInfo {
            required: 0,
            total: 0,
            has_rest: false,
        };
    }

    let mut required = 0u32;
    let mut total = 0u32;
    let mut has_rest = false;
    let mut depth = 0u32;
    let mut in_string: Option<u8> = None;
    let mut segment_start = 0usize;

    let bytes = inner.as_bytes();
    let mut escape_next = false;
    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if let Some(quote) = in_string {
            if b == b'\\' {
                escape_next = true;
            } else if b == quote {
                in_string = None;
            }
            continue;
        }
        match b {
            b'"' | b'\'' => in_string = Some(b),
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                let segment = &inner[segment_start..i];
                classify_param(segment, &mut required, &mut total, &mut has_rest);
                segment_start = i + 1;
            }
            _ => {}
        }
    }
    let last_segment = &inner[segment_start..];
    classify_param(last_segment, &mut required, &mut total, &mut has_rest);

    ParamInfo {
        required,
        total,
        has_rest,
    }
}

fn classify_param(segment: &str, required: &mut u32, total: &mut u32, has_rest: &mut bool) {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return;
    }
    *total += 1;
    if trimmed.ends_with("...") {
        *has_rest = true;
    } else if !trimmed.contains(':') {
        *required += 1;
    }
}

#[allow(clippy::cast_possible_truncation)]
fn count_call_args(root: &SyntaxNode, ref_range: TextRange, kind: SymbolKind) -> Option<u32> {
    let token = root.token_at_offset(ref_range.start()).right_biased()?;

    let expected_parent = match kind {
        SymbolKind::Function => SyntaxKind::FUNCTION_CALL,
        SymbolKind::Mixin => SyntaxKind::INCLUDE_RULE,
        _ => return None,
    };

    let call_node = token
        .parent()?
        .ancestors()
        .find(|n| n.kind() == expected_parent)?;
    let arg_list = call_node
        .children()
        .find(|c| c.kind() == SyntaxKind::ARG_LIST)?;

    Some(
        arg_list
            .children()
            .filter(|c| c.kind() == SyntaxKind::ARG)
            .count() as u32,
    )
}

// ── Undefined reference checking ────────────────────────────────────

#[rustfmt::skip]
const CSS_GLOBAL_FUNCTIONS: &[&str] = &[
    // CSS value functions
    "var", "calc", "env", "min", "max", "clamp", "url", "attr",
    "counter", "counters", "image-set", "element",
    // Gradients
    "linear-gradient", "radial-gradient", "conic-gradient",
    "repeating-linear-gradient", "repeating-radial-gradient", "repeating-conic-gradient",
    // Colors (CSS)
    "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch",
    "color", "color-mix", "light-dark",
    // Transforms
    "translate", "translateX", "translateY", "translateZ", "translate3d",
    "rotate", "rotateX", "rotateY", "rotateZ", "rotate3d",
    "scale", "scaleX", "scaleY", "scaleZ", "scale3d",
    "skew", "skewX", "skewY", "matrix", "matrix3d", "perspective",
    // Filters
    "blur", "brightness", "contrast", "drop-shadow", "grayscale",
    "hue-rotate", "invert", "sepia",
    // Shapes / clip-path
    "polygon", "circle", "ellipse", "inset", "path", "rect",
    // Grid
    "minmax", "fit-content", "repeat",
    // Animation timing
    "cubic-bezier", "steps", "linear",
    // Font
    "format", "local",
    // Math (CSS)
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
    "pow", "sqrt", "hypot", "log", "exp", "mod", "rem", "sign",
    // Other CSS
    "cross-fade", "paint", "symbols", "anchor",
    // Sass global built-ins
    "if",
    "lighten", "darken", "mix", "adjust-hue", "saturate", "desaturate",
    "opacify", "fade-in", "transparentize", "fade-out",
    "red", "green", "blue", "alpha", "opacity",
    "adjust-color", "scale-color", "change-color", "ie-hex-str",
    "str-length", "str-insert", "str-index", "str-slice", "to-upper-case", "to-lower-case",
    "quote", "unquote", "string",
    "length", "nth", "set-nth", "join", "append", "zip", "index", "list-separator",
    "map-get", "map-merge", "map-remove", "map-keys", "map-values", "map-has-key",
    "type-of", "unit", "unitless", "comparable", "call",
    "feature-exists", "variable-exists", "global-variable-exists",
    "function-exists", "mixin-exists", "inspect",
    "unique-id", "random", "percentage", "round", "ceil", "floor", "abs",
    "selector-nest", "selector-append", "selector-replace", "selector-unify",
    "is-superselector", "simple-selectors", "selector-parse",
];

fn check_undefined(
    root: &SyntaxNode,
    symbols: &FileSymbols,
    module_graph: &workspace::ModuleGraph,
    uri: &Uri,
    out: &mut Vec<SemanticDiagnostic>,
) {
    for sym_ref in &symbols.references {
        // Skip placeholders — @extend %name can reference dynamically generated selectors
        if sym_ref.kind == SymbolKind::Placeholder {
            continue;
        }

        // Skip known CSS/global functions
        if sym_ref.kind == SymbolKind::Function && is_css_global_function(&sym_ref.name) {
            continue;
        }

        let kind = sym_ref.kind;

        // Skip variables that are parameters, loop variables, or locally declared
        if sym_ref.kind == SymbolKind::Variable
            && is_locally_defined(root, sym_ref.range, &sym_ref.name)
        {
            continue;
        }

        let namespace = ast_helpers::namespace_of_ref(root, sym_ref.range);
        if module_graph
            .resolve_reference(uri, namespace.as_deref(), &sym_ref.name, kind)
            .is_some()
        {
            continue;
        }

        let diagnostic_code = match sym_ref.kind {
            SymbolKind::Variable => "undefined-variable",
            SymbolKind::Function => "undefined-function",
            SymbolKind::Mixin => "undefined-mixin",
            SymbolKind::Placeholder => unreachable!(),
        };

        out.push(SemanticDiagnostic {
            range: sym_ref.selection_range,
            message: format!("undefined {} `{}`", kind_label(sym_ref.kind), sym_ref.name),
            severity: DiagnosticSeverity::WARNING,
            code: diagnostic_code,
            data: Some(serde_json::json!({
                "name": sym_ref.name,
                "kind": kind_label(sym_ref.kind),
            })),
        });
    }
}

fn kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Variable => "variable",
        SymbolKind::Function => "function",
        SymbolKind::Mixin => "mixin",
        SymbolKind::Placeholder => "placeholder",
    }
}

fn is_css_global_function(name: &str) -> bool {
    CSS_GLOBAL_FUNCTIONS.contains(&name)
}

// ── Parameter / loop variable detection ─────────────────────────────

fn is_locally_defined(root: &SyntaxNode, ref_range: TextRange, name: &str) -> bool {
    let Some(token) = root.token_at_offset(ref_range.start()).right_biased() else {
        return false;
    };
    for ancestor in token.parent().into_iter().flat_map(|p| p.ancestors()) {
        match ancestor.kind() {
            SyntaxKind::FUNCTION_RULE | SyntaxKind::MIXIN_RULE => {
                if has_param_named(&ancestor, name) {
                    return true;
                }
            }
            SyntaxKind::EACH_RULE | SyntaxKind::FOR_RULE => {
                if has_loop_var_named(&ancestor, name) {
                    return true;
                }
            }
            _ => {}
        }
        // Check for local $var: declarations in this ancestor that precede the reference
        if has_local_var_before(&ancestor, ref_range.start(), name) {
            return true;
        }
    }
    false
}

fn has_local_var_before(
    node: &SyntaxNode,
    before: sass_parser::text_range::TextSize,
    name: &str,
) -> bool {
    for child in node.children() {
        if child.kind() == SyntaxKind::VARIABLE_DECL && child.text_range().start() < before {
            // VARIABLE_DECL = DOLLAR IDENT COLON ...
            let mut saw_dollar = false;
            for tok in child
                .children_with_tokens()
                .filter_map(NodeOrToken::into_token)
            {
                if tok.kind() == SyntaxKind::DOLLAR {
                    saw_dollar = true;
                } else if tok.kind() == SyntaxKind::IDENT && saw_dollar {
                    if tok.text() == name {
                        return true;
                    }
                    break;
                } else {
                    break;
                }
            }
        }
    }
    false
}

fn has_param_named(rule_node: &SyntaxNode, name: &str) -> bool {
    for child in rule_node.children() {
        if child.kind() == SyntaxKind::PARAM_LIST {
            for param in child.children() {
                if param.kind() == SyntaxKind::PARAM {
                    // PARAM = DOLLAR IDENT [COLON default] [DOT_DOT_DOT]
                    for tok in param
                        .children_with_tokens()
                        .filter_map(NodeOrToken::into_token)
                    {
                        if tok.kind() == SyntaxKind::IDENT && tok.text() == name {
                            return true;
                        }
                    }
                }
            }
            return false;
        }
    }
    false
}

fn has_loop_var_named(rule_node: &SyntaxNode, name: &str) -> bool {
    // @each $x, $y in ... or @for $i from ...
    // Loop variables are DOLLAR IDENT at direct children level
    let mut saw_dollar = false;
    for element in rule_node.children_with_tokens() {
        if let Some(tok) = element.into_token() {
            if tok.kind() == SyntaxKind::DOLLAR {
                saw_dollar = true;
            } else if tok.kind() == SyntaxKind::IDENT && saw_dollar {
                if tok.text() == name {
                    return true;
                }
                saw_dollar = false;
            } else {
                saw_dollar = false;
            }
        } else {
            saw_dollar = false;
        }
    }
    false
}

// ── Suppression ─────────────────────────────────────────────────────

fn should_suppress_undefined(uri: &Uri, module_graph: &workspace::ModuleGraph) -> bool {
    module_graph.has_unresolved_imports(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudo_selectors_are_sorted() {
        assert!(PSEUDO_SELECTORS.is_sorted());
        assert!(crate::css_properties::CSS_PROPERTIES.is_sorted());
    }

    #[test]
    fn param_info_no_params() {
        let info = parse_param_info("()");
        assert_eq!(info.required, 0);
        assert_eq!(info.total, 0);
        assert!(!info.has_rest);
    }

    #[test]
    fn param_info_required_only() {
        let info = parse_param_info("($a, $b)");
        assert_eq!(info.required, 2);
        assert_eq!(info.total, 2);
        assert!(!info.has_rest);
    }

    #[test]
    fn param_info_with_defaults() {
        let info = parse_param_info("($a, $b: 1px, $c: red)");
        assert_eq!(info.required, 1);
        assert_eq!(info.total, 3);
        assert!(!info.has_rest);
    }

    #[test]
    fn param_info_with_rest() {
        let info = parse_param_info("($a, $rest...)");
        assert_eq!(info.required, 1);
        assert_eq!(info.total, 2);
        assert!(info.has_rest);
    }

    #[test]
    fn param_info_nested_parens() {
        let info = parse_param_info("($a, $b: map-get($m, key))");
        assert_eq!(info.required, 1);
        assert_eq!(info.total, 2);
        assert!(!info.has_rest);
    }

    #[test]
    fn param_info_all_defaults() {
        let info = parse_param_info("($a: 1, $b: 2)");
        assert_eq!(info.required, 0);
        assert_eq!(info.total, 2);
        assert!(!info.has_rest);
    }

    #[test]
    fn param_info_string_with_comma() {
        let info = parse_param_info(r#"($a, $sep: ",", $b: 1)"#);
        assert_eq!(info.required, 1);
        assert_eq!(info.total, 3);
        assert!(!info.has_rest);
    }

    #[test]
    fn param_info_string_with_escaped_quote() {
        let info = parse_param_info(r#"($a, $s: "say \"hi\"", $b: 1)"#);
        assert_eq!(info.required, 1);
        assert_eq!(info.total, 3);
        assert!(!info.has_rest);
    }

    #[test]
    fn css_global_var_is_recognized() {
        assert!(is_css_global_function("var"));
        assert!(is_css_global_function("calc"));
        assert!(is_css_global_function("min"));
    }

    #[test]
    fn custom_function_not_css_global() {
        assert!(!is_css_global_function("my-func"));
        assert!(!is_css_global_function("darken2"));
    }
}
