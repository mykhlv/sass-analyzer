use std::collections::BTreeSet;
use std::io::Write as _;
use std::process::{Command, Stdio};
use std::{fmt, fs};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use ungrammar::{Grammar, Rule};

// ── Intermediate model ──────────────────────────────────────────────

struct AstSrc {
    nodes: Vec<AstNodeSrc>,
    enums: Vec<AstEnumSrc>,
}

struct AstNodeSrc {
    name: String,
    fields: Vec<Field>,
}

struct AstEnumSrc {
    name: String,
    variants: Vec<String>,
}

enum Field {
    Node {
        name: String,
        ty: String,
        cardinality: Cardinality,
    },
}

#[derive(Clone, Copy)]
enum Cardinality {
    Optional,
    Many,
}

// ── Public entry point ──────────────────────────────────────────────

pub fn generate() -> Result<(), Box<dyn std::error::Error>> {
    let project_root = project_root()?;
    let ungram_path = project_root.join("sass.ungram");
    let output_path = project_root.join("crates/sass-parser/src/ast/generated.rs");

    let grammar_text = fs::read_to_string(&ungram_path)?;
    let grammar: Grammar = grammar_text.parse().map_err(|e| format!("{e}"))?;

    let ast = lower(&grammar);
    validate(&ast);

    let code = generate_code(&ast);
    let formatted = reformat(&code)?;

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&output_path, formatted)?;
    eprintln!("wrote {}", output_path.display());

    Ok(())
}

// ── Project root discovery ──────────────────────────────────────────

fn project_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_or_else(
        |_| std::env::current_dir().unwrap(),
        std::path::PathBuf::from,
    );

    // Walk up from manifest dir to find workspace root (contains sass.ungram)
    let mut dir = manifest_dir.as_path();
    loop {
        if dir.join("sass.ungram").exists() {
            return Ok(dir.to_path_buf());
        }
        dir = dir
            .parent()
            .ok_or("could not find project root (no sass.ungram found)")?;
    }
}

// ── Lowering: Grammar → AstSrc ─────────────────────────────────────

fn lower(grammar: &Grammar) -> AstSrc {
    let mut ast = AstSrc {
        nodes: Vec::new(),
        enums: Vec::new(),
    };

    for node in grammar.iter() {
        let name = grammar[node].name.clone();
        let rule = &grammar[node].rule;

        if is_enum(grammar, rule) {
            let variants = extract_variants(grammar, rule);
            ast.enums.push(AstEnumSrc { name, variants });
        } else {
            let fields = extract_fields(grammar, rule);
            ast.nodes.push(AstNodeSrc { name, fields });
        }
    }

    ast
}

/// A rule is an enum if it is a top-level `Alt` where every alternative
/// is a bare `Node` reference (no tokens, no sequences).
fn is_enum(grammar: &Grammar, rule: &Rule) -> bool {
    let Rule::Alt(alts) = rule else {
        return false;
    };
    alts.iter().all(|alt| matches!(alt, Rule::Node(_)))
        && alts.len() >= 2
        && alts.iter().all(|alt| {
            if let Rule::Node(node) = alt {
                // Enum variants should themselves be defined as struct nodes (not other enums)
                // We accept them all at this stage; validation happens later
                let _ = grammar[*node].name.as_str();
                true
            } else {
                false
            }
        })
}

fn extract_variants(grammar: &Grammar, rule: &Rule) -> Vec<String> {
    let Rule::Alt(alts) = rule else {
        return Vec::new();
    };
    alts.iter()
        .filter_map(|alt| {
            if let Rule::Node(node) = alt {
                Some(grammar[*node].name.clone())
            } else {
                None
            }
        })
        .collect()
}

fn extract_fields(grammar: &Grammar, rule: &Rule) -> Vec<Field> {
    let mut fields = Vec::new();
    let mut seen = BTreeSet::new();
    extract_fields_inner(grammar, rule, &mut fields, &mut seen);
    fields
}

fn extract_fields_inner(
    grammar: &Grammar,
    rule: &Rule,
    fields: &mut Vec<Field>,
    seen: &mut BTreeSet<String>,
) {
    match rule {
        Rule::Labeled { label, rule } => {
            // A labeled field: determine the type and cardinality from the inner rule
            if let Some((ty, cardinality)) = resolve_field_type(grammar, rule) {
                let name = label.clone();
                if seen.insert(name.clone()) {
                    fields.push(Field::Node {
                        name,
                        ty,
                        cardinality,
                    });
                }
            }
        }
        Rule::Node(node) => {
            // Unlabeled node reference: generate accessor from type name
            let ty = grammar[*node].name.clone();
            let name = to_lower_snake_case(&ty);
            // Unlabeled single child → Optional
            if seen.insert(name.clone()) {
                fields.push(Field::Node {
                    name,
                    ty,
                    cardinality: Cardinality::Optional,
                });
            }
        }
        Rule::Seq(rules) => {
            for rule in rules {
                extract_fields_inner(grammar, rule, fields, seen);
            }
        }
        Rule::Alt(alts) => {
            for alt in alts {
                extract_fields_inner(grammar, alt, fields, seen);
            }
        }
        Rule::Opt(rule) => {
            extract_fields_inner(grammar, rule, fields, seen);
        }
        Rule::Rep(rule) => {
            // Repetition: if it's a node, generate Many accessor
            if let Rule::Node(node) = rule.as_ref() {
                let ty = grammar[*node].name.clone();
                let name = pluralize(&to_lower_snake_case(&ty));
                if seen.insert(name.clone()) {
                    fields.push(Field::Node {
                        name,
                        ty,
                        cardinality: Cardinality::Many,
                    });
                }
            } else if let Rule::Labeled { label, rule } = rule.as_ref() {
                if let Some((ty, _)) = resolve_field_type(grammar, rule) {
                    let name = label.clone();
                    if seen.insert(name.clone()) {
                        fields.push(Field::Node {
                            name,
                            ty,
                            cardinality: Cardinality::Many,
                        });
                    }
                }
            }
        }
        Rule::Token(_) => {
            // Skip token-only fields (v1: no token accessors)
        }
    }
}

/// Resolve the type and cardinality for a field's inner rule.
fn resolve_field_type(grammar: &Grammar, rule: &Rule) -> Option<(String, Cardinality)> {
    match rule {
        Rule::Node(node) => {
            let ty = grammar[*node].name.clone();
            Some((ty, Cardinality::Optional))
        }
        Rule::Rep(inner) => {
            if let Rule::Node(node) = inner.as_ref() {
                let ty = grammar[*node].name.clone();
                Some((ty, Cardinality::Many))
            } else {
                None
            }
        }
        Rule::Opt(inner) => {
            resolve_field_type(grammar, inner).map(|(ty, _)| (ty, Cardinality::Optional))
        }
        _ => None,
    }
}

// ── Validation ──────────────────────────────────────────────────────

fn validate(ast: &AstSrc) {
    // Verify no duplicate names
    let mut all_names = BTreeSet::new();
    for node in &ast.nodes {
        assert!(
            all_names.insert(&node.name),
            "duplicate node name: {}",
            node.name
        );
    }
    for enum_node in &ast.enums {
        assert!(
            all_names.insert(&enum_node.name),
            "duplicate enum name: {}",
            enum_node.name
        );
    }
}

// ── Code generation ─────────────────────────────────────────────────

fn generate_code(ast: &AstSrc) -> String {
    let nodes = ast.nodes.iter().map(generate_node);
    let enums = ast.enums.iter().map(generate_enum);

    let output = quote! {
        //! Generated by `cargo xtask codegen` from `sass.ungram`.
        //! Do not edit manually.

        #![allow(clippy::match_like_matches_macro)]

        use crate::ast::{support, AstChildren, AstNode};
        use crate::syntax::SyntaxNode;
        use crate::syntax_kind::SyntaxKind;

        #(#nodes)*
        #(#enums)*
    };

    output.to_string()
}

fn generate_node(node: &AstNodeSrc) -> TokenStream {
    let name = format_ident!("{}", node.name);
    let kind = format_ident!("{}", to_upper_snake_case(&node.name));

    let accessors = node.fields.iter().map(|field| {
        let Field::Node {
            name,
            ty,
            cardinality,
        } = field;
        let method = format_ident!("{name}");
        let ty_ident = format_ident!("{ty}");
        match cardinality {
            Cardinality::Optional => quote! {
                pub fn #method(&self) -> Option<#ty_ident> {
                    support::child(&self.syntax)
                }
            },
            Cardinality::Many => quote! {
                pub fn #method(&self) -> AstChildren<#ty_ident> {
                    support::children(&self.syntax)
                }
            },
        }
    });

    quote! {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct #name {
            pub(crate) syntax: SyntaxNode,
        }

        impl AstNode for #name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == SyntaxKind::#kind
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                if Self::can_cast(syntax.kind()) {
                    Some(Self { syntax })
                } else {
                    None
                }
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }

        impl #name {
            #(#accessors)*
        }
    }
}

fn generate_enum(enum_src: &AstEnumSrc) -> TokenStream {
    let name = format_ident!("{}", enum_src.name);
    let variants: Vec<_> = enum_src
        .variants
        .iter()
        .map(|v| format_ident!("{v}"))
        .collect();
    let kinds: Vec<_> = enum_src
        .variants
        .iter()
        .map(|v| format_ident!("{}", to_upper_snake_case(v)))
        .collect();

    let cast_arms = variants.iter().zip(kinds.iter()).map(|(variant, kind)| {
        quote! { SyntaxKind::#kind => #name::#variant(#variant { syntax }) }
    });

    let syntax_arms = variants.iter().map(|variant| {
        quote! { #name::#variant(it) => &it.syntax }
    });

    let can_cast_arms = kinds.iter().map(|kind| {
        quote! { SyntaxKind::#kind }
    });

    quote! {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum #name {
            #(#variants(#variants),)*
        }

        impl AstNode for #name {
            fn can_cast(kind: SyntaxKind) -> bool {
                matches!(kind, #(#can_cast_arms)|*)
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                let res = match syntax.kind() {
                    #(#cast_arms,)*
                    _ => return None,
                };
                Some(res)
            }

            fn syntax(&self) -> &SyntaxNode {
                match self {
                    #(#syntax_arms,)*
                }
            }
        }
    }
}

// ── Formatting ──────────────────────────────────────────────────────

fn reformat(code: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .as_mut()
        .ok_or("failed to open rustfmt stdin")?
        .write_all(code.as_bytes())?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("rustfmt failed: {stderr}").into());
    }

    Ok(String::from_utf8(output.stdout)?)
}

// ── Name conversion helpers ─────────────────────────────────────────

fn to_upper_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_uppercase());
    }
    result
}

fn to_lower_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

fn pluralize(name: &str) -> String {
    if name.ends_with('s') || name.ends_with("sh") || name.ends_with("ch") || name.ends_with('x') {
        format!("{name}es")
    } else if name.ends_with('y') && !name.ends_with("ey") && !name.ends_with("ay") {
        format!("{}ies", &name[..name.len() - 1])
    } else {
        format!("{name}s")
    }
}

// ── Display impls for debugging ─────────────────────────────────────

impl fmt::Display for Cardinality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Optional => write!(f, "Optional"),
            Self::Many => write!(f, "Many"),
        }
    }
}
