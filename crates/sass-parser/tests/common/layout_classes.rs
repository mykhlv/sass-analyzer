//! The kinds of difference between `parse_sass` and `parse_sass_legacy` that the
//! layout frontend makes on purpose, on inputs that are not on the allow-list: the
//! inputs of a fuzzer, almost none of them Sass. Each kind is a defect of legacy that the
//! layout frontend fixes, or a reading of input that Dart Sass rejects, described by what
//! the layout frontend does; a difference of no kind is a regression to look into.
//!
//! Where it can, a kind is recognised by taking its cause out of the input — closing the
//! comment, renaming the variable — and comparing again, so that an input with one more
//! difference besides is not taken for one of these.
//!
//! Included with `#[path]` by the corpus tests and by the differential fuzz target, which
//! must agree on what is known; it needs `layout_differential` beside it.

use sass_parser::input::Input;
use sass_parser::lexer::IndentedLexer;
use sass_parser::parser::{Layout, Parser};
use sass_parser::syntax_kind::SyntaxKind;

use crate::layout_differential::layout_mismatch;

/// A kind of difference; `Other` is none of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// Spaces or tabs after the last line break open no block: the input ends at column 0.
    TrailingIndentation,
    /// A loud comment that is never closed is reported, and the statement before it ends
    /// there, as at the end of the input. Closed, it makes no difference.
    UnterminatedComment,
    /// Indentation Dart Sass rejects is reported — a dedent to no open block's column,
    /// tabs and spaces mixed, the other kind of indentation than the file's first indented
    /// line — with the blocks as they were.
    IndentationDiagnostics,
    /// A value ending in `and`, `or` or `not` goes on only into a line that could be its
    /// operand — not one that starts like a declaration, nor after `and` or `or` with no
    /// operand before it; legacy took any next line into the value. Respelled, the words
    /// make no difference.
    WordOperatorBeforeAStatement,
    /// A variable named like a header word (`$to`, `$in`) at the end of a line ends the
    /// header there; legacy read the name as the word and took the next line into the
    /// header. Renamed, the variables make no difference.
    VariableNamedLikeAHeaderWord,
    /// An error inside a bracket ends the bracket at the next line no deeper than the
    /// line it opened on, and the lines after a bracket left unclosed count again.
    BracketRecovery,
    /// A line dedented to a column that no open block has is reported, and closes every
    /// block deeper than it, as the blocks follow the columns; legacy kept such a line in
    /// a block a comment line had opened. Dart Sass rejects such input.
    DedentToNoOpenBlock,
    /// The same for a line that holds only a silent comment, which is not reported: it
    /// closes the block a comment line above it opened, where legacy kept it in. Without
    /// the comment-only lines, the frontends agree. Only on input legacy reports errors in.
    CommentLineDedentedToNoOpenBlock,
    /// A closing bracket that does not match the open one (`}` inside `[`), or a `{` or
    /// `}` outside brackets, which the indented syntax has no use for, puts the input's
    /// count of open brackets out of step with the grammar's, so that a line break the
    /// grammar reads inside a bracket can still end a statement. Dart Sass rejects such
    /// input.
    UnbalancedBrackets,
    /// A silent comment reaches across a blank line onto the deeper lines after it, as in
    /// Dart Sass.
    CommentAcrossBlankLine,
    /// A line ending in `*`, `and`, `or` or `not` goes on wherever the grammar reads the
    /// word as an operator; legacy went on only in a statement it took for an expression,
    /// and reported the missing operand at the end of the line.
    OperatorContinuesALine,
    /// A line that starts with `$` and ends with a colon goes on only after a variable's
    /// name, `$name:`; legacy continued any such line.
    StrayColonEndsALine,
    /// A `*` that ends a line ends the statement where the grammar does not read it as
    /// multiplication — in a selector, in an at-rule's prelude; legacy went on after it
    /// in any statement with a colon before it. With such a `*` respelled, the frontends
    /// agree. Only on input one of them reports errors in.
    StarEndsALine,
    /// Whether a line that ends in an operator or a colon goes on is the grammar's
    /// decision — after an operator it reads as one, after a colon only where a name ends
    /// the line — where legacy decided by the shape of the line, and went on where the
    /// layout frontend stops: across a blank line or a comment line, or to the end of the
    /// input, which moves the end of the statement and the errors with it. Without the
    /// line breaks after the input's last operator, or with the last character of such
    /// lines respelled, the frontends agree; the second only on input legacy reports
    /// errors in, as such lines are everywhere in valid Sass, where a difference is a
    /// regression.
    OperatorOrColonEndsALine,
    /// An at-rule that starts in mid-line, after a statement an error cut short, is a
    /// statement of its own: the header of the broken statement ends there, and a deeper
    /// line below opens the at-rule's block, or ends it, rather than going on with that
    /// header, as legacy went on with a variable's. With such at-rules on lines of their
    /// own, the frontends agree. Only on input legacy reports errors in.
    AtRuleAfterABrokenStatement,
    Other,
}

/// The kinds of difference that `source` makes between the two frontends, in the order
/// they were found: empty if they agree, ending in `Other` if what remains is of no kind.
///
/// An input can make several. Those with a cause that can be taken out are taken out one
/// by one, comparing again each time, until the input makes no difference or one of the
/// kinds that only the parse shows accounts for the rest. The broadest, line ends
/// respelled, is tried last.
pub fn classify(source: &str) -> Vec<Class> {
    let mut source = source.to_owned();
    let mut found = Vec::new();
    loop {
        let Some((legacy, layout)) = layout_mismatch(&source) else {
            return found;
        };
        if without_indentation_diagnostics(&layout) == legacy {
            found.push(Class::IndentationDiagnostics);
            return found;
        }
        let (class, next) = if let Some(next) = without_trailing_indentation(&source) {
            (Class::TrailingIndentation, next)
        } else if layout.contains("unterminated comment")
            && !found.contains(&Class::UnterminatedComment)
        {
            (Class::UnterminatedComment, format!("{source}*/"))
        } else if let Some(next) = with_word_operators_respelled(&source) {
            (Class::WordOperatorBeforeAStatement, next)
        } else if let Some(next) = with_header_word_variables_renamed(&source) {
            (Class::VariableNamedLikeAHeaderWord, next)
        } else {
            let class = by_what_the_parse_does(&source, &legacy);
            let rejected = legacy.contains("\nerror ");
            if class != Class::Other {
                found.push(class);
                return found;
            }
            let reported = rejected || layout.contains("\nerror ");
            if let Some(next) = ending_at_its_last_operator(&source) {
                (Class::OperatorOrColonEndsALine, next)
            } else if let Some(next) = with_line_end_stars_respelled(&source).filter(|_| reported) {
                (Class::StarEndsALine, next)
            } else if let Some(next) =
                with_line_end_operators_respelled(&source).filter(|_| rejected)
            {
                (Class::OperatorOrColonEndsALine, next)
            } else if let Some(next) =
                with_at_rules_on_lines_of_their_own(&source).filter(|_| rejected)
            {
                (Class::AtRuleAfterABrokenStatement, next)
            } else if let Some(next) = without_comment_lines(&source).filter(|_| rejected) {
                (Class::CommentLineDedentedToNoOpenBlock, next)
            } else {
                found.push(class);
                return found;
            }
        };
        found.push(class);
        source = next;
    }
}

/// The kinds that only the parse shows, which account for whatever else differs.
fn by_what_the_parse_does(source: &str, legacy: &str) -> Class {
    if recovers_from_bracket_errors(source) {
        Class::BracketRecovery
    } else if layout_reports(source, "inconsistent indentation") {
        Class::DedentToNoOpenBlock
    } else if has_unbalanced_brackets(source) {
        Class::UnbalancedBrackets
    } else if silent_comment_crosses_a_blank_line(source) {
        Class::CommentAcrossBlankLine
    } else if operator_ends_a_line_legacy_ended(source, legacy) {
        Class::OperatorContinuesALine
    } else if stray_colon_ends_a_variable_line(source) {
        Class::StrayColonEndsALine
    } else {
        Class::Other
    }
}

/// The input without the spaces and tabs after its last line break, if it has any.
fn without_trailing_indentation(source: &str) -> Option<String> {
    let trimmed = source.trim_end_matches([' ', '\t']);
    (trimmed.len() < source.len() && trimmed.ends_with(['\n', '\r', '\x0c']))
        .then(|| trimmed.to_owned())
}

/// The rendering without the indentation diagnostics, which only the layout frontend
/// reports.
fn without_indentation_diagnostics(layout: &str) -> String {
    const MESSAGES: [&str; 4] = [
        "inconsistent indentation",
        "may not be mixed",
        "expected tabs",
        "expected spaces",
    ];
    let mut out = String::with_capacity(layout.len());
    for line in layout.split_inclusive('\n') {
        if !(line.starts_with("error ") && MESSAGES.iter().any(|m| line.contains(m))) {
            out.push_str(line);
        }
    }
    out
}

/// The input with `and`, `or` and `not` at the end of a line respelled, if it has any; a
/// silent comment after the word on the line does not count.
fn with_word_operators_respelled(source: &str) -> Option<String> {
    let mut changed = false;
    let respelled: String = source
        .split_inclusive('\n')
        .map(|line| {
            let code = line.find("//").map_or(line, |comment| &line[..comment]);
            let body = code.trim_end_matches(['\n', '\r', '\x0c', ' ', '\t']);
            for word in ["and", "or", "not"] {
                if let Some(head) = body.strip_suffix(word)
                    && !head.ends_with(|c: char| c.is_alphanumeric() || "-_$".contains(c))
                {
                    changed = true;
                    return format!("{head}x{}", &line[head.len() + 1..]);
                }
            }
            line.to_owned()
        })
        .collect();
    changed.then_some(respelled)
}

/// The input with an operator or a colon that ends a line respelled, if it has any; a
/// silent comment after it on the line does not count.
fn with_line_end_operators_respelled(source: &str) -> Option<String> {
    const OPERATORS: [char; 11] = ['*', '/', '+', '-', '%', '=', '~', '<', '>', '!', ':'];
    let mut changed = false;
    let respelled: String = source
        .split_inclusive(['\n', '\r', '\x0c'])
        .map(|line| {
            let code = line.find("//").map_or(line, |comment| &line[..comment]);
            let body = code.trim_end_matches(['\n', '\r', '\x0c', ' ', '\t']);
            // Not the `*` of a `/*` or the `/` of a `*/`, as in `/**/*`.
            let comment = (body.ends_with("/*") && !body.ends_with("*/*"))
                || (body.ends_with("*/") && !body.ends_with("/*/"))
                || body.ends_with("/*/");
            if body.ends_with(OPERATORS) && !comment {
                changed = true;
                format!("{}x{}", &body[..body.len() - 1], &line[body.len()..])
            } else {
                line.to_owned()
            }
        })
        .collect();
    changed.then_some(respelled)
}

/// The input without the lines that hold only a silent comment, if it has any.
fn without_comment_lines(source: &str) -> Option<String> {
    let mut changed = false;
    let kept: String = source
        .split_inclusive(['\n', '\r', '\x0c'])
        .filter(|line| {
            let comment = line.trim_start_matches([' ', '\t']).starts_with("//");
            changed |= comment;
            !comment
        })
        .collect();
    changed.then_some(kept)
}

/// The input with a `*` that ends a line respelled, if it has any; not the `*` of a `/*`.
fn with_line_end_stars_respelled(source: &str) -> Option<String> {
    let mut changed = false;
    let respelled: String = source
        .split_inclusive(['\n', '\r', '\x0c'])
        .map(|line| {
            let body = line.trim_end_matches(['\n', '\r', '\x0c', ' ', '\t']);
            if body.ends_with('*') && !body.ends_with("/*") {
                changed = true;
                format!("{}x{}", &body[..body.len() - 1], &line[body.len()..])
            } else {
                line.to_owned()
            }
        })
        .collect();
    changed.then_some(respelled)
}

/// The input with every `@` that does not start its line moved to a line of its own, at
/// the same indentation, if it has any.
fn with_at_rules_on_lines_of_their_own(source: &str) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(source.len());
    for line in source.split_inclusive(['\n', '\r', '\x0c']) {
        let indent = &line[..line.len() - line.trim_start_matches([' ', '\t']).len()];
        let mut rest = line;
        let mut first = true;
        while let Some(at) = rest.find('@') {
            if first && rest[..at].trim_start_matches([' ', '\t']).is_empty() {
                out.push_str(&rest[..=at]);
                rest = &rest[at + 1..];
            } else {
                changed = true;
                out.push_str(&rest[..at]);
                out.push('\n');
                out.push_str(indent);
                out.push('@');
                rest = &rest[at + 1..];
            }
            first = false;
        }
        out.push_str(rest);
    }
    changed.then_some(out)
}

/// The input with the variables named like a header word renamed, if it has any.
fn with_header_word_variables_renamed(source: &str) -> Option<String> {
    const WORDS: [&str; 13] = [
        "from", "through", "to", "in", "and", "or", "not", "if", "using", "as", "with", "show",
        "hide",
    ];
    let mut renamed = source.to_owned();
    for word in WORDS {
        renamed = renamed.replace(&format!("${word}"), "$x");
    }
    (renamed != source).then_some(renamed)
}

/// The input without the line breaks and indentation after it, if it ends in an operator
/// or a colon: whether the statement ends before them or after is all that differs.
fn ending_at_its_last_operator(source: &str) -> Option<String> {
    let trimmed = source.trim_end_matches(['\n', '\r', '\x0c', ' ', '\t']);
    (trimmed.len() < source.len() && trimmed.ends_with(['*', '/', '+', '-', '%', '=', ':']))
        .then(|| trimmed.to_owned())
}

fn layout_reports(source: &str, message: &str) -> bool {
    sass_parser::parse_sass(source)
        .1
        .iter()
        .any(|(found, _)| found.contains(message))
}

fn recovers_from_bracket_errors(source: &str) -> bool {
    let input = Input::from_source_indented(source);
    let mut parser = Parser::with_layout(input, source, Layout::Indented);
    sass_parser::grammar::source_file(&mut parser);
    let recovered = parser.recovered_from_bracket_errors();
    let _ = parser.finish();
    recovered
}

/// A closer that does not match the innermost open bracket, or a `{` or `}` outside
/// brackets, among the tokens of the input.
fn has_unbalanced_brackets(source: &str) -> bool {
    let input = Input::from_source_indented(source);
    let mut open = Vec::new();
    for pos in 0..input.len() {
        let kind = input.kind(pos);
        match kind {
            SyntaxKind::LPAREN | SyntaxKind::LBRACKET | SyntaxKind::HASH_LBRACE => {
                open.push(kind);
            }
            SyntaxKind::LBRACE => {
                if open.is_empty() {
                    return true;
                }
                open.push(kind);
            }
            SyntaxKind::RPAREN | SyntaxKind::RBRACKET | SyntaxKind::RBRACE => {
                let matching = match open.pop() {
                    Some(SyntaxKind::LPAREN) => SyntaxKind::RPAREN,
                    Some(SyntaxKind::LBRACKET) => SyntaxKind::RBRACKET,
                    Some(SyntaxKind::HASH_LBRACE | SyntaxKind::LBRACE) => SyntaxKind::RBRACE,
                    _ => return true,
                };
                if kind != matching {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn silent_comment_crosses_a_blank_line(source: &str) -> bool {
    let mut legacy = IndentedLexer::new(source);
    let mut layout = IndentedLexer::crossing_blank_lines(source);
    loop {
        let token = legacy.next_token();
        if token != layout.next_token() {
            return true;
        }
        if token.0 == SyntaxKind::EOF {
            return false;
        }
    }
}

/// Legacy reported a missing operand right after one of these operators at the end of a
/// line, where the layout frontend continued the line.
fn operator_ends_a_line_legacy_ended(source: &str, legacy: &str) -> bool {
    legacy
        .lines()
        .filter(|line| line.ends_with(": expected expression"))
        .filter_map(|line| {
            let range = line.strip_prefix("error ")?.split(':').next()?;
            let (start, _) = range.split_once("..")?;
            start.parse::<usize>().ok()
        })
        .any(|offset| {
            let before = source
                .get(..offset)
                .unwrap_or("")
                .trim_end_matches([' ', '\t']);
            ["*", "and", "or", "not"]
                .iter()
                .any(|operator| before.ends_with(operator))
        })
}

fn stray_colon_ends_a_variable_line(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_matches([' ', '\t', '\r', '\x0c']);
        line.strip_prefix('$')
            .and_then(|rest| rest.strip_suffix(':'))
            .is_some_and(|name| {
                !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            })
    })
}
