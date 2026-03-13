use rowan::GreenNode;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;
use tower_lsp_server::ls_types::{DiagnosticSeverity, Uri};

use crate::symbols::{FileSymbols, RefKind, SymbolKind};
use crate::workspace;

pub(crate) struct SemanticDiagnostic {
    pub(crate) range: TextRange,
    pub(crate) message: String,
    pub(crate) severity: DiagnosticSeverity,
    pub(crate) code: &'static str,
}

pub(crate) fn check_file(
    uri: &Uri,
    symbols: &FileSymbols,
    module_graph: &workspace::ModuleGraph,
    green: &GreenNode,
) -> Vec<SemanticDiagnostic> {
    let root = SyntaxNode::new_root(green.clone());
    let mut result = Vec::new();
    let suppress_undefined = should_suppress_undefined(uri, module_graph);

    check_arg_count(&root, symbols, module_graph, uri, &mut result);
    if !suppress_undefined {
        check_undefined(&root, symbols, module_graph, uri, &mut result);
    }

    result
}

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
            RefKind::Function => SymbolKind::Function,
            RefKind::Mixin => SymbolKind::Mixin,
            _ => continue,
        };

        let namespace = namespace_of_ref(root, sym_ref.range);
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
fn count_call_args(root: &SyntaxNode, ref_range: TextRange, kind: RefKind) -> Option<u32> {
    let token = root.token_at_offset(ref_range.start()).right_biased()?;

    let expected_parent = match kind {
        RefKind::Function => SyntaxKind::FUNCTION_CALL,
        RefKind::Mixin => SyntaxKind::INCLUDE_RULE,
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
    "polygon", "circle", "ellipse", "inset", "path",
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
        if sym_ref.kind == RefKind::Placeholder {
            continue;
        }

        // Skip known CSS/global functions
        if sym_ref.kind == RefKind::Function && is_css_global_function(&sym_ref.name) {
            continue;
        }

        let kind = match sym_ref.kind {
            RefKind::Variable => SymbolKind::Variable,
            RefKind::Function => SymbolKind::Function,
            RefKind::Mixin => SymbolKind::Mixin,
            RefKind::Placeholder => continue,
        };

        // Skip variables that are parameters of enclosing functions/mixins
        // or loop variables (@each, @for)
        if sym_ref.kind == RefKind::Variable
            && is_param_or_loop_var(root, sym_ref.range, &sym_ref.name)
        {
            continue;
        }

        let namespace = namespace_of_ref(root, sym_ref.range);
        if module_graph
            .resolve_reference(uri, namespace.as_deref(), &sym_ref.name, kind)
            .is_some()
        {
            continue;
        }

        let diagnostic_code = match sym_ref.kind {
            RefKind::Variable => "undefined-variable",
            RefKind::Function => "undefined-function",
            RefKind::Mixin => "undefined-mixin",
            RefKind::Placeholder => unreachable!(),
        };

        out.push(SemanticDiagnostic {
            range: sym_ref.selection_range,
            message: format!("undefined {} `{}`", kind_label(sym_ref.kind), sym_ref.name),
            severity: DiagnosticSeverity::WARNING,
            code: diagnostic_code,
        });
    }
}

fn kind_label(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Variable => "variable",
        RefKind::Function => "function",
        RefKind::Mixin => "mixin",
        RefKind::Placeholder => "placeholder",
    }
}

fn is_css_global_function(name: &str) -> bool {
    CSS_GLOBAL_FUNCTIONS.iter().any(|&f| f == name)
}

// ── Namespace extraction ────────────────────────────────────────────

fn namespace_of_ref(root: &SyntaxNode, ref_range: TextRange) -> Option<String> {
    let token = root.token_at_offset(ref_range.start()).right_biased()?;
    for node in token.parent()?.ancestors() {
        if node.kind() == SyntaxKind::NAMESPACE_REF {
            let ns_ident = node
                .children_with_tokens()
                .filter_map(rowan::NodeOrToken::into_token)
                .find(|t| t.kind() == SyntaxKind::IDENT)?;
            return Some(ns_ident.text().to_string());
        }
        // Don't walk past the direct container
        if matches!(
            node.kind(),
            SyntaxKind::DECLARATION
                | SyntaxKind::RULE_SET
                | SyntaxKind::BLOCK
                | SyntaxKind::SOURCE_FILE
        ) {
            break;
        }
    }
    None
}

// ── Parameter / loop variable detection ─────────────────────────────

fn is_param_or_loop_var(root: &SyntaxNode, ref_range: TextRange, name: &str) -> bool {
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
                        .filter_map(rowan::NodeOrToken::into_token)
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
