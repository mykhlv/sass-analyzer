#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SassDoc {
    pub description: Option<String>,
    pub params: Vec<ParamDoc>,
    pub return_doc: Option<TypedDoc>,
    pub examples: Vec<Example>,
    pub see_refs: Vec<String>,
    pub output: Option<String>,
    pub content: Option<String>,
    pub deprecated: Option<String>,
    pub type_annotation: Option<String>,
    pub throws: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDoc {
    pub name: String,
    pub type_annotation: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedDoc {
    pub type_annotation: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    pub description: Option<String>,
    pub code: String,
}

pub fn parse(doc: &str) -> SassDoc {
    let mut result = SassDoc {
        description: None,
        params: Vec::new(),
        return_doc: None,
        examples: Vec::new(),
        see_refs: Vec::new(),
        output: None,
        content: None,
        deprecated: None,
        type_annotation: None,
        throws: Vec::new(),
    };

    let lines: Vec<&str> = doc.lines().collect();
    let mut i = 0;

    // Collect leading description (lines before first @annotation)
    let mut desc_lines = Vec::new();
    while i < lines.len() && !is_sassdoc_annotation(lines[i]) {
        desc_lines.push(lines[i]);
        i += 1;
    }
    let desc = desc_lines.join("\n").trim().to_owned();
    if !desc.is_empty() {
        result.description = Some(desc);
    }

    // Parse annotations
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if let Some(rest) = strip_tag(trimmed, "@param") {
            let mut param = parse_param(rest);
            i += 1;
            collect_continuation(&lines, &mut i, &mut param.description);
            result.params.push(param);
        } else if let Some(rest) =
            strip_tag(trimmed, "@returns").or_else(|| strip_tag(trimmed, "@return"))
        {
            let mut ret = parse_typed_doc(rest);
            i += 1;
            collect_continuation(&lines, &mut i, &mut ret.description);
            result.return_doc = Some(ret);
        } else if let Some(rest) = strip_tag(trimmed, "@example") {
            let desc_text = rest.trim();
            let description = if desc_text.is_empty() {
                None
            } else {
                Some(desc_text.to_owned())
            };
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() && !is_sassdoc_annotation(lines[i]) {
                code_lines.push(lines[i]);
                i += 1;
            }
            let code = dedent_example(&code_lines);
            if !code.is_empty() {
                result.examples.push(Example { description, code });
            }
        } else if let Some(rest) = strip_tag(trimmed, "@see") {
            let see = rest.trim();
            if !see.is_empty() {
                result.see_refs.push(see.to_owned());
            }
            i += 1;
        } else if let Some(rest) = strip_tag(trimmed, "@output") {
            result.output = Some(rest.trim().to_owned());
            i += 1;
        } else if let Some(rest) = strip_tag(trimmed, "@content") {
            result.content = Some(rest.trim().to_owned());
            i += 1;
        } else if let Some(rest) = strip_tag(trimmed, "@deprecated") {
            let msg = rest.trim();
            result.deprecated = Some(if msg.is_empty() {
                "Deprecated".to_owned()
            } else {
                msg.to_owned()
            });
            i += 1;
        } else if let Some(rest) = strip_tag(trimmed, "@type") {
            result.type_annotation = Some(trim_braces(rest.trim()).to_owned());
            i += 1;
        } else if let Some(rest) =
            strip_tag(trimmed, "@throws").or_else(|| strip_tag(trimmed, "@throw"))
        {
            let msg = rest.trim();
            if !msg.is_empty() {
                result.throws.push(msg.to_owned());
            }
            i += 1;
        } else {
            // Unknown annotation — skip
            i += 1;
        }
    }

    result
}

#[rustfmt::skip]
const SASSDOC_TAGS: &[&str] = &[
    "@param", "@return", "@returns", "@example", "@see", "@output",
    "@content", "@deprecated", "@type", "@throw", "@throws",
];

fn is_sassdoc_annotation(line: &str) -> bool {
    let trimmed = line.trim_start();
    SASSDOC_TAGS.iter().any(|tag| starts_with_tag(trimmed, tag))
}

fn starts_with_tag(s: &str, tag: &str) -> bool {
    s.starts_with(tag)
        && s.as_bytes()
            .get(tag.len())
            .is_none_or(|&b| b == b' ' || b == b'\t' || b == b'{')
}

fn strip_tag<'a>(s: &'a str, tag: &str) -> Option<&'a str> {
    if starts_with_tag(s, tag) {
        Some(&s[tag.len()..])
    } else {
        None
    }
}

fn collect_continuation(lines: &[&str], i: &mut usize, desc: &mut Option<String>) {
    while *i < lines.len() && !is_sassdoc_annotation(lines[*i]) {
        let cont = lines[*i].trim();
        if cont.is_empty() {
            break;
        }
        match desc {
            Some(d) => {
                d.push(' ');
                d.push_str(cont);
            }
            None => *desc = Some(cont.to_owned()),
        }
        *i += 1;
    }
}

fn parse_param(rest: &str) -> ParamDoc {
    let rest = rest.trim_start();
    let (type_annotation, rest) = extract_braced_type(rest);
    let rest = rest.trim_start();

    // Extract $name
    let (name, rest) = if let Some(stripped) = rest.strip_prefix('$') {
        let end = stripped
            .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .unwrap_or(stripped.len());
        (stripped[..end].to_owned(), &stripped[end..])
    } else {
        // No $ prefix — try bare name
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .unwrap_or(rest.len());
        if end > 0 {
            (rest[..end].to_owned(), &rest[end..])
        } else {
            return ParamDoc {
                name: String::new(),
                type_annotation,
                description: None,
            };
        }
    };

    // Skip separator (` - `, `: `, whitespace)
    let rest = rest.trim_start();
    let rest = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix('-'))
        .unwrap_or(rest)
        .trim_start();

    let description = if rest.is_empty() {
        None
    } else {
        Some(rest.to_owned())
    };

    ParamDoc {
        name,
        type_annotation,
        description,
    }
}

fn parse_typed_doc(rest: &str) -> TypedDoc {
    let rest = rest.trim_start();
    let (type_annotation, rest) = extract_braced_type(rest);
    let rest = rest.trim_start();
    let rest = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix('-'))
        .unwrap_or(rest)
        .trim_start();

    let description = if rest.is_empty() {
        None
    } else {
        Some(rest.to_owned())
    };

    TypedDoc {
        type_annotation,
        description,
    }
}

fn extract_braced_type(s: &str) -> (Option<String>, &str) {
    if !s.starts_with('{') {
        return (None, s);
    }
    // Find matching close brace (handle nested braces)
    let mut depth = 0u32;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let ty = s[1..i].trim();
                    return (Some(ty.to_owned()), &s[i + 1..]);
                }
            }
            _ => {}
        }
    }
    // No closing brace found — treat as no type
    (None, s)
}

fn dedent_example(lines: &[&str]) -> String {
    // Skip leading/trailing empty lines
    let first_non_empty = lines.iter().position(|l| !l.trim().is_empty());
    let last_non_empty = lines.iter().rposition(|l| !l.trim().is_empty());

    let (Some(first), Some(last)) = (first_non_empty, last_non_empty) else {
        return String::new();
    };

    let lines = &lines[first..=last];

    // Find minimum indentation
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|l| {
            if l.len() >= min_indent {
                &l[min_indent..]
            } else {
                l.trim()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn trim_braces(s: &str) -> &str {
    s.strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap_or(s)
}

// ── Hover formatting ────────────────────────────────────────────────

pub fn format_markdown(doc: &SassDoc) -> String {
    let mut parts = Vec::new();

    if let Some(desc) = &doc.description {
        parts.push(desc.clone());
    }

    if let Some(dep) = &doc.deprecated {
        parts.push(format!("**@deprecated** {dep}"));
    }

    if !doc.params.is_empty() {
        let mut param_lines = Vec::new();
        for p in &doc.params {
            if p.name.is_empty() {
                continue;
            }
            let mut line = format!("- `${}`", p.name);
            if let Some(ty) = &p.type_annotation {
                line.push_str(&format!(" `{{{ty}}}`"));
            }
            if let Some(desc) = &p.description {
                line.push_str(&format!(" — {desc}"));
            }
            param_lines.push(line);
        }
        if !param_lines.is_empty() {
            parts.push(format!("**Parameters:**\n{}", param_lines.join("\n")));
        }
    }

    if let Some(ret) = &doc.return_doc {
        let mut line = String::from("**@return**");
        if let Some(ty) = &ret.type_annotation {
            line.push_str(&format!(" `{{{ty}}}`"));
        }
        if let Some(desc) = &ret.description {
            line.push_str(&format!(" — {desc}"));
        }
        parts.push(line);
    }

    if let Some(ty) = &doc.type_annotation {
        parts.push(format!("**@type** `{{{ty}}}`"));
    }

    if let Some(output) = &doc.output {
        parts.push(format!("**@output** {output}"));
    }

    if let Some(content) = &doc.content {
        parts.push(format!("**@content** {content}"));
    }

    for throw in &doc.throws {
        parts.push(format!("**@throw** {throw}"));
    }

    for example in &doc.examples {
        let header = if let Some(desc) = &example.description {
            format!("**@example** {desc}")
        } else {
            "**@example**".to_owned()
        };
        parts.push(format!("{header}\n```scss\n{}\n```", example.code));
    }

    for see in &doc.see_refs {
        parts.push(format!("**@see** {see}"));
    }

    parts.join("\n\n")
}

pub fn has_annotations(doc: &str) -> bool {
    doc.lines().any(is_sassdoc_annotation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_description() {
        let doc = parse("Doubles a number");
        assert_eq!(doc.description.as_deref(), Some("Doubles a number"));
        assert!(doc.params.is_empty());
        assert!(doc.return_doc.is_none());
    }

    #[test]
    fn parse_param_with_type_and_description() {
        let doc = parse("@param {Number} $n - The number to double");
        assert_eq!(doc.params.len(), 1);
        assert_eq!(doc.params[0].name, "n");
        assert_eq!(doc.params[0].type_annotation.as_deref(), Some("Number"));
        assert_eq!(
            doc.params[0].description.as_deref(),
            Some("The number to double")
        );
    }

    #[test]
    fn parse_param_without_type() {
        let doc = parse("@param $color - The base color");
        assert_eq!(doc.params[0].name, "color");
        assert!(doc.params[0].type_annotation.is_none());
        assert_eq!(doc.params[0].description.as_deref(), Some("The base color"));
    }

    #[test]
    fn parse_param_without_description() {
        let doc = parse("@param {String} $name");
        assert_eq!(doc.params[0].name, "name");
        assert_eq!(doc.params[0].type_annotation.as_deref(), Some("String"));
        assert!(doc.params[0].description.is_none());
    }

    #[test]
    fn parse_return_with_type() {
        let doc = parse("@return {Color} The adjusted color");
        let ret = doc.return_doc.unwrap();
        assert_eq!(ret.type_annotation.as_deref(), Some("Color"));
        assert_eq!(ret.description.as_deref(), Some("The adjusted color"));
    }

    #[test]
    fn parse_returns_alias() {
        let doc = parse("@returns {Number}");
        let ret = doc.return_doc.unwrap();
        assert_eq!(ret.type_annotation.as_deref(), Some("Number"));
    }

    #[test]
    fn parse_example() {
        let doc = parse("@example\n  .foo {\n    color: red;\n  }");
        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].description.is_none());
        assert_eq!(doc.examples[0].code, ".foo {\n  color: red;\n}");
    }

    #[test]
    fn parse_example_with_description() {
        let doc = parse("@example scss - Basic usage\n  color: red;");
        assert_eq!(
            doc.examples[0].description.as_deref(),
            Some("scss - Basic usage")
        );
        assert_eq!(doc.examples[0].code, "color: red;");
    }

    #[test]
    fn parse_deprecated() {
        let doc = parse("@deprecated Use new-mixin instead");
        assert_eq!(doc.deprecated.as_deref(), Some("Use new-mixin instead"));
    }

    #[test]
    fn parse_deprecated_no_message() {
        let doc = parse("@deprecated");
        assert_eq!(doc.deprecated.as_deref(), Some("Deprecated"));
    }

    #[test]
    fn parse_type_annotation() {
        let doc = parse("@type {Color}");
        assert_eq!(doc.type_annotation.as_deref(), Some("Color"));
    }

    #[test]
    fn parse_type_annotation_no_braces() {
        let doc = parse("@type Color");
        assert_eq!(doc.type_annotation.as_deref(), Some("Color"));
    }

    #[test]
    fn parse_see() {
        let doc = parse("@see other-function");
        assert_eq!(doc.see_refs, vec!["other-function"]);
    }

    #[test]
    fn parse_throw() {
        let doc = parse("@throw Error if $n is negative");
        assert_eq!(doc.throws, vec!["Error if $n is negative"]);
    }

    #[test]
    fn parse_throws_alias() {
        let doc = parse("@throws Error if $n is negative");
        assert_eq!(doc.throws, vec!["Error if $n is negative"]);
    }

    #[test]
    fn parse_output() {
        let doc = parse("@output Responsive breakpoint styles");
        assert_eq!(doc.output.as_deref(), Some("Responsive breakpoint styles"));
    }

    #[test]
    fn parse_content() {
        let doc = parse("@content Styles to include inside the media query");
        assert_eq!(
            doc.content.as_deref(),
            Some("Styles to include inside the media query")
        );
    }

    #[test]
    fn parse_full_sassdoc() {
        let doc = parse(
            "Adjusts the lightness of a color.\n\
             @param {Color} $color - The base color\n\
             @param {Number} $amount - Amount to adjust\n\
             @return {Color} The adjusted color\n\
             @example\n\
               .foo { color: adjust($red, 10%); }\n\
             @see darken\n\
             @see lighten",
        );
        assert_eq!(
            doc.description.as_deref(),
            Some("Adjusts the lightness of a color.")
        );
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "color");
        assert_eq!(doc.params[1].name, "amount");
        assert!(doc.return_doc.is_some());
        assert_eq!(doc.examples.len(), 1);
        assert_eq!(doc.see_refs.len(), 2);
    }

    #[test]
    fn format_markdown_full() {
        let doc = parse(
            "Doubles a number.\n\
             @param {Number} $n - The number\n\
             @return {Number} The doubled value",
        );
        let md = format_markdown(&doc);
        assert!(md.contains("Doubles a number."));
        assert!(md.contains("**Parameters:**"));
        assert!(md.contains("`$n`"));
        assert!(md.contains("`{Number}`"));
        assert!(md.contains("**@return**"));
    }

    #[test]
    fn has_annotations_detects_param() {
        assert!(has_annotations("Some description\n@param $x"));
        assert!(!has_annotations("Just a description"));
    }

    // ── Edge cases from review ──────────────────────────────────────

    #[test]
    fn parse_empty_input() {
        let doc = parse("");
        assert!(doc.description.is_none());
        assert!(doc.params.is_empty());
        assert!(doc.return_doc.is_none());
        assert!(doc.examples.is_empty());
    }

    #[test]
    fn nested_braces_in_type() {
        let doc = parse("@param {Map<String, List<Number>>} $map - A complex map");
        assert_eq!(
            doc.params[0].type_annotation.as_deref(),
            Some("Map<String, List<Number>>")
        );
        assert_eq!(doc.params[0].name, "map");
    }

    #[test]
    fn example_with_no_code_is_dropped() {
        let doc = parse("@example scss - Usage\n@param $x");
        assert!(doc.examples.is_empty());
        assert_eq!(doc.params.len(), 1);
    }

    #[test]
    fn multiple_return_last_wins() {
        let doc = parse("@return {Number} first\n@return {String} second");
        let ret = doc.return_doc.unwrap();
        assert_eq!(ret.type_annotation.as_deref(), Some("String"));
        assert_eq!(ret.description.as_deref(), Some("second"));
    }

    #[test]
    fn tag_prefix_not_matched_as_annotation() {
        // "@parameters" should not match "@param"
        let doc = parse("@parameters are listed below");
        assert!(doc.params.is_empty());
        assert_eq!(
            doc.description.as_deref(),
            Some("@parameters are listed below")
        );
    }

    #[test]
    fn multiline_param_description() {
        let doc = parse("@param {Number} $n - The number\n  to double (must be positive)");
        assert_eq!(
            doc.params[0].description.as_deref(),
            Some("The number to double (must be positive)")
        );
    }

    #[test]
    fn multiline_return_description() {
        let doc = parse("@return {Color} The adjusted\n  color value");
        let ret = doc.return_doc.unwrap();
        assert_eq!(ret.description.as_deref(), Some("The adjusted color value"));
    }

    #[test]
    fn param_without_name_skipped_in_markdown() {
        let doc = parse("@param {Number}");
        assert_eq!(doc.params.len(), 1);
        assert!(doc.params[0].name.is_empty());
        let md = format_markdown(&doc);
        assert!(!md.contains("**Parameters:**"));
    }

    #[test]
    fn has_annotations_rejects_false_prefix() {
        assert!(!has_annotations("@parameterize something"));
        assert!(!has_annotations("@returning value"));
        assert!(!has_annotations("@seeAlso other"));
    }
}
