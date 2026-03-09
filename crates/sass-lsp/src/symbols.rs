use sass_parser::syntax::{SyntaxNode, SyntaxToken};
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Function,
    Mixin,
    Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Variable,
    Function,
    Mixin,
    Placeholder,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub selection_range: TextRange,
    pub params: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub name: String,
    pub kind: RefKind,
    pub range: TextRange,
    pub selection_range: TextRange,
}

#[derive(Debug, Clone, Default)]
pub struct FileSymbols {
    pub definitions: Vec<Symbol>,
    pub references: Vec<SymbolRef>,
}

pub fn collect_symbols(root: &SyntaxNode) -> FileSymbols {
    let mut symbols = FileSymbols::default();

    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::VARIABLE_DECL => collect_variable_decl(&node, &mut symbols),
            SyntaxKind::FUNCTION_RULE => collect_function_rule(&node, &mut symbols),
            SyntaxKind::MIXIN_RULE => collect_mixin_rule(&node, &mut symbols),
            SyntaxKind::SIMPLE_SELECTOR => collect_placeholder_selector(&node, &mut symbols),
            SyntaxKind::VARIABLE_REF => collect_variable_ref(&node, &mut symbols),
            SyntaxKind::FUNCTION_CALL => collect_function_call(&node, &mut symbols),
            SyntaxKind::INCLUDE_RULE => collect_include_ref(&node, &mut symbols),
            SyntaxKind::EXTEND_RULE => collect_extend_ref(&node, &mut symbols),
            _ => {}
        }
    }

    symbols.definitions.sort_by_key(|s| s.range.start());
    symbols.references.sort_by_key(|r| r.range.start());
    symbols
}

// ── Definition extractors ───────────────────────────────────────────

fn collect_variable_decl(node: &SyntaxNode, symbols: &mut FileSymbols) {
    let Some((name, sel_range)) = dollar_ident(node) else {
        return;
    };
    symbols.definitions.push(Symbol {
        name,
        kind: SymbolKind::Variable,
        range: node.text_range(),
        selection_range: sel_range,
        params: None,
    });
}

fn collect_function_rule(node: &SyntaxNode, symbols: &mut FileSymbols) {
    let Some(ident) = nth_ident_token(node, 1) else {
        return;
    };
    symbols.definitions.push(Symbol {
        name: ident.text().to_string(),
        kind: SymbolKind::Function,
        range: node.text_range(),
        selection_range: ident.text_range(),
        params: extract_param_text(node),
    });
}

fn collect_mixin_rule(node: &SyntaxNode, symbols: &mut FileSymbols) {
    let Some(ident) = nth_ident_token(node, 1) else {
        return;
    };
    symbols.definitions.push(Symbol {
        name: ident.text().to_string(),
        kind: SymbolKind::Mixin,
        range: node.text_range(),
        selection_range: ident.text_range(),
        params: extract_param_text(node),
    });
}

fn collect_placeholder_selector(node: &SyntaxNode, symbols: &mut FileSymbols) {
    let mut pct_start = None;
    let mut ident_text = None;
    let mut ident_end = None;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::PERCENT => pct_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if pct_start.is_some() => {
                    ident_text = Some(token.text().to_string());
                    ident_end = Some(token.text_range().end());
                    break;
                }
                _ => {}
            }
        }
    }
    let (Some(start), Some(name), Some(end)) = (pct_start, ident_text, ident_end) else {
        return;
    };
    let sel_range = TextRange::new(start, end);
    symbols.definitions.push(Symbol {
        name,
        kind: SymbolKind::Placeholder,
        range: node.text_range(),
        selection_range: sel_range,
        params: None,
    });
}

// ── Reference extractors ────────────────────────────────────────────

fn collect_variable_ref(node: &SyntaxNode, symbols: &mut FileSymbols) {
    // Skip refs that are inside a VARIABLE_DECL (that's the definition itself)
    if node
        .parent()
        .is_some_and(|p| p.kind() == SyntaxKind::VARIABLE_DECL)
    {
        return;
    }
    let Some((name, sel_range)) = dollar_ident(node) else {
        return;
    };
    symbols.references.push(SymbolRef {
        name,
        kind: RefKind::Variable,
        range: node.text_range(),
        selection_range: sel_range,
    });
}

fn collect_function_call(node: &SyntaxNode, symbols: &mut FileSymbols) {
    let Some(ident) = first_ident_token(node) else {
        return;
    };
    symbols.references.push(SymbolRef {
        name: ident.text().to_string(),
        kind: RefKind::Function,
        range: node.text_range(),
        selection_range: ident.text_range(),
    });
}

fn collect_include_ref(node: &SyntaxNode, symbols: &mut FileSymbols) {
    let Some(ident) = nth_ident_token(node, 1) else {
        return;
    };
    symbols.references.push(SymbolRef {
        name: ident.text().to_string(),
        kind: RefKind::Mixin,
        range: node.text_range(),
        selection_range: ident.text_range(),
    });
}

fn collect_extend_ref(node: &SyntaxNode, symbols: &mut FileSymbols) {
    // Only track %placeholder extends (not class/tag extends)
    let mut pct_start = None;
    let mut ident_text = None;
    let mut ident_end = None;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::PERCENT => pct_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if pct_start.is_some() => {
                    ident_text = Some(token.text().to_string());
                    ident_end = Some(token.text_range().end());
                    break;
                }
                _ => {}
            }
        }
    }
    let (Some(start), Some(name), Some(end)) = (pct_start, ident_text, ident_end) else {
        return;
    };
    let sel_range = TextRange::new(start, end);
    symbols.references.push(SymbolRef {
        name,
        kind: RefKind::Placeholder,
        range: node.text_range(),
        selection_range: sel_range,
    });
}

// ── Helpers ─────────────────────────────────────────────────────────

fn first_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::IDENT)
}

fn nth_ident_token(node: &SyntaxNode, n: usize) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|t| t.kind() == SyntaxKind::IDENT)
        .nth(n)
}

/// Extract `$name` → (name without $, combined DOLLAR..IDENT range).
fn dollar_ident(node: &SyntaxNode) -> Option<(String, TextRange)> {
    let mut dollar_start = None;
    for element in node.children_with_tokens() {
        if let Some(token) = element.into_token() {
            match token.kind() {
                SyntaxKind::DOLLAR => dollar_start = Some(token.text_range().start()),
                SyntaxKind::IDENT if dollar_start.is_some() => {
                    let range = TextRange::new(dollar_start.unwrap(), token.text_range().end());
                    return Some((token.text().to_string(), range));
                }
                _ => {}
            }
        }
    }
    None
}

/// Extract the text of the `PARAM_LIST` node (e.g. `($size, $color: red)`).
fn extract_param_text(node: &SyntaxNode) -> Option<String> {
    node.children()
        .find(|c| c.kind() == SyntaxKind::PARAM_LIST)
        .map(|pl| pl.text().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_symbols(input: &str) -> FileSymbols {
        let (green, _errors) = sass_parser::parse(input);
        let root = SyntaxNode::new_root(green);
        collect_symbols(&root)
    }

    #[test]
    fn variable_declaration() {
        let s = parse_symbols("$color: red;");
        assert_eq!(s.definitions.len(), 1);
        let def = &s.definitions[0];
        assert_eq!(def.name, "color");
        assert_eq!(def.kind, SymbolKind::Variable);
        assert!(def.params.is_none());
    }

    #[test]
    fn variable_declaration_and_reference() {
        let s = parse_symbols("$color: red;\n.btn { color: $color; }");
        assert_eq!(s.definitions.len(), 1);
        assert_eq!(s.definitions[0].name, "color");
        assert_eq!(s.references.len(), 1);
        assert_eq!(s.references[0].name, "color");
        assert_eq!(s.references[0].kind, RefKind::Variable);
    }

    #[test]
    fn function_definition() {
        let s = parse_symbols("@function double($n) { @return $n * 2; }");
        assert_eq!(s.definitions.len(), 1);
        let def = &s.definitions[0];
        assert_eq!(def.name, "double");
        assert_eq!(def.kind, SymbolKind::Function);
        assert_eq!(def.params.as_deref(), Some("($n)"));
    }

    #[test]
    fn mixin_definition_with_params() {
        let s = parse_symbols("@mixin size($w, $h: 100px) { width: $w; height: $h; }");
        assert_eq!(s.definitions.len(), 1);
        let def = &s.definitions[0];
        assert_eq!(def.name, "size");
        assert_eq!(def.kind, SymbolKind::Mixin);
        assert_eq!(def.params.as_deref(), Some("($w, $h: 100px)"));
    }

    #[test]
    fn include_reference() {
        let s = parse_symbols(".btn { @include size(10px, 20px); }");
        assert_eq!(s.references.len(), 1);
        assert_eq!(s.references[0].name, "size");
        assert_eq!(s.references[0].kind, RefKind::Mixin);
    }

    #[test]
    fn placeholder_definition_and_extend() {
        let s = parse_symbols("%base { display: block; }\n.btn { @extend %base; }");
        assert_eq!(s.definitions.len(), 1);
        assert_eq!(s.definitions[0].name, "base");
        assert_eq!(s.definitions[0].kind, SymbolKind::Placeholder);

        assert_eq!(s.references.len(), 1);
        assert_eq!(s.references[0].name, "base");
        assert_eq!(s.references[0].kind, RefKind::Placeholder);
    }

    #[test]
    fn function_call_reference() {
        let s = parse_symbols(".btn { color: darken(red, 10%); }");
        let func_refs: Vec<_> = s
            .references
            .iter()
            .filter(|r| r.kind == RefKind::Function)
            .collect();
        assert_eq!(func_refs.len(), 1);
        assert_eq!(func_refs[0].name, "darken");
    }

    #[test]
    fn mixed_symbols() {
        let input = "\
$primary: blue;
@mixin btn($size) { font-size: $size; }
@function lighten-color($c) { @return $c; }
%clearfix { &::after { clear: both; } }
.card {
  color: $primary;
  @include btn(16px);
  @extend %clearfix;
  background: lighten-color(red);
}
";
        let s = parse_symbols(input);
        // Definitions: $primary, btn, lighten-color, %clearfix
        assert_eq!(s.definitions.len(), 4);
        assert_eq!(s.definitions[0].kind, SymbolKind::Variable);
        assert_eq!(s.definitions[1].kind, SymbolKind::Mixin);
        assert_eq!(s.definitions[2].kind, SymbolKind::Function);
        assert_eq!(s.definitions[3].kind, SymbolKind::Placeholder);

        // References: $primary, @include btn, @extend %clearfix, lighten-color()
        // Plus $size ref inside mixin, $c ref inside function
        let var_refs: Vec<_> = s
            .references
            .iter()
            .filter(|r| r.kind == RefKind::Variable)
            .collect();
        let mixin_refs: Vec<_> = s
            .references
            .iter()
            .filter(|r| r.kind == RefKind::Mixin)
            .collect();
        let func_refs: Vec<_> = s
            .references
            .iter()
            .filter(|r| r.kind == RefKind::Function)
            .collect();
        let placeholder_refs: Vec<_> = s
            .references
            .iter()
            .filter(|r| r.kind == RefKind::Placeholder)
            .collect();

        assert!(var_refs.len() >= 2, "at least $primary + $size refs");
        assert_eq!(mixin_refs.len(), 1);
        assert!(func_refs.len() >= 1);
        assert_eq!(placeholder_refs.len(), 1);
    }
}
