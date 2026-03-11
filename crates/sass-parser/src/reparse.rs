use crate::syntax::SyntaxNode;
use crate::syntax_kind::SyntaxKind;
use crate::text_range::{TextRange, TextSize};
use rowan::{GreenNode, NodeOrToken};

/// A single text edit: delete `delete` bytes at `offset`, then insert `insert_len` bytes.
/// The caller must already have applied this edit to produce `new_source`.
pub struct TextEdit {
    pub offset: TextSize,
    pub delete: TextSize,
    pub insert_len: TextSize,
}

/// Try to incrementally reparse after a single edit.
///
/// Returns `Some((green, errors))` if incremental succeeded, `None` if the caller
/// should fall back to a full reparse. Falling back is always safe.
#[allow(clippy::cast_possible_truncation)]
pub fn incremental_reparse(
    old_green: &GreenNode,
    old_errors: &[(String, TextRange)],
    edit: &TextEdit,
    new_source: &str,
) -> Option<(GreenNode, Vec<(String, TextRange)>)> {
    let old_root = SyntaxNode::new_root(old_green.clone());
    let edit_range = TextRange::new(edit.offset, edit.offset + edit.delete);

    let container = find_deepest_container(&old_root, edit_range);
    let container_kind = container.kind();
    if container_kind != SyntaxKind::SOURCE_FILE && container_kind != SyntaxKind::BLOCK {
        return None;
    }

    let children: Vec<rowan::SyntaxElement<crate::syntax::SassLanguage>> =
        container.children_with_tokens().collect();
    if children.is_empty() {
        return None;
    }

    let (first, last) = affected_child_range(&children, edit_range)?;

    // Bail out if the original affected range includes LBRACE/RBRACE.
    if container_kind == SyntaxKind::BLOCK {
        for child in children.iter().take(last + 1).skip(first) {
            if child.kind() == SyntaxKind::LBRACE || child.kind() == SyntaxKind::RBRACE {
                return None;
            }
        }
    }

    // Expand ±1 for safety (trivia attachment, boundary merging),
    // but never expand into LBRACE/RBRACE of a BLOCK.
    let mut exp_first = first;
    let mut exp_last = last;
    if first > 0
        && children[first - 1].kind() != SyntaxKind::LBRACE
        && children[first - 1].kind() != SyntaxKind::RBRACE
    {
        exp_first = first - 1;
    }
    if last + 1 < children.len()
        && children[last + 1].kind() != SyntaxKind::LBRACE
        && children[last + 1].kind() != SyntaxKind::RBRACE
    {
        exp_last = last + 1;
    }

    // Use expanded range if it doesn't cover all children; otherwise keep
    // the original range (expansion would eliminate all reusable siblings).
    let (first, last) = if exp_last - exp_first + 1 >= children.len() {
        (first, last)
    } else {
        (exp_first, exp_last)
    };

    if last - first + 1 >= children.len() {
        return None;
    }

    // Compute region text from new_source.
    let old_start = children[first].text_range().start();
    let old_end = children[last].text_range().end();
    let delta = i64::from(u32::from(edit.insert_len)) - i64::from(u32::from(edit.delete));

    let new_end = i64::from(u32::from(old_end)) + delta;
    let new_end = u32::try_from(new_end).ok().filter(|&e| (e as usize) <= new_source.len())?;
    let new_start = u32::from(old_start);
    if new_start as usize > new_source.len() || new_start > new_end {
        return None;
    }
    let region_text = &new_source[new_start as usize..new_end as usize];

    // Re-lex + re-parse the region.
    let (temp_green, region_errors) = parse_region(region_text, container_kind);

    // Verify round-trip: the region text length must match.
    let temp_root = SyntaxNode::new_root(temp_green.clone());
    if temp_root.text().len() != TextSize::from(region_text.len() as u32) {
        return None;
    }

    // Extract new children from the temporary wrapper node.
    let new_children: Vec<rowan::NodeOrToken<GreenNode, rowan::GreenToken>> = temp_green
        .children()
        .map(rowan::NodeOrToken::to_owned)
        .collect();

    // Splice into the container's green node.
    let path = ancestor_path(&old_root, &container);
    let container_green = get_green_at_path(old_green, &path);
    let new_container = container_green.splice_children(first..=last, new_children);

    // Reconstruct parents up to root.
    let new_root = rebuild_ancestors(old_green, &path, new_container);

    // Merge errors.
    let all_errors = merge_errors(old_errors, &region_errors, old_start, old_end, delta);

    // Final text-length sanity check.
    let result_root = SyntaxNode::new_root(new_root.clone());
    if result_root.text().len() != TextSize::from(new_source.len() as u32) {
        return None;
    }

    Some((new_root, all_errors))
}

/// Find the deepest `SOURCE_FILE` or `BLOCK` node containing the edit range.
fn find_deepest_container(root: &SyntaxNode, edit_range: TextRange) -> SyntaxNode {
    let mut container = root.clone();
    'search: loop {
        for child in container.children() {
            if !node_contains_edit(&child, edit_range) {
                continue;
            }
            if child.kind() == SyntaxKind::BLOCK {
                container = child;
                continue 'search;
            }
            for desc in child.descendants() {
                if desc.kind() == SyntaxKind::BLOCK && node_contains_edit(&desc, edit_range) {
                    container = desc;
                    continue 'search;
                }
            }
            return container;
        }
        return container;
    }
}

fn node_contains_edit(node: &SyntaxNode, edit_range: TextRange) -> bool {
    let range = node.text_range();
    if edit_range.is_empty() {
        // Strict < for end: an insert at the exact end of a node is "between"
        // this node and the next, not inside it.
        range.start() <= edit_range.start() && edit_range.start() < range.end()
    } else {
        range.start() <= edit_range.start() && edit_range.end() <= range.end()
    }
}

/// Find which children of a container overlap the edit range.
fn affected_child_range(
    children: &[rowan::SyntaxElement<crate::syntax::SassLanguage>],
    edit_range: TextRange,
) -> Option<(usize, usize)> {
    let mut first = None;
    let mut last = None;

    for (i, child) in children.iter().enumerate() {
        if child_overlaps_edit(child.text_range(), edit_range) {
            if first.is_none() {
                first = Some(i);
            }
            last = Some(i);
        }
    }

    if let (Some(f), Some(l)) = (first, last) {
        return Some((f, l));
    }

    // Pure insertion at a boundary between children.
    let offset = edit_range.start();
    for (i, child) in children.iter().enumerate() {
        let range = child.text_range();
        if range.start() <= offset && offset <= range.end() {
            return Some((i, i));
        }
    }

    None
}

fn child_overlaps_edit(child_range: TextRange, edit_range: TextRange) -> bool {
    if edit_range.is_empty() {
        child_range.start() <= edit_range.start() && edit_range.start() < child_range.end()
    } else {
        child_range.start() < edit_range.end() && edit_range.start() < child_range.end()
    }
}

/// Parse a region using the appropriate grammar entry point.
fn parse_region(
    source: &str,
    container_kind: SyntaxKind,
) -> (GreenNode, Vec<(String, TextRange)>) {
    let input = crate::input::Input::from_source(source);
    let mut parser = crate::parser::Parser::new(input, source);
    if container_kind == SyntaxKind::SOURCE_FILE {
        crate::grammar::source_file(&mut parser);
    } else {
        crate::grammar::reparse_block_body(&mut parser);
    }
    let (events, errors, input, src) = parser.finish();
    crate::build_tree(events, &errors, &input, src)
}

/// Compute the path (child indices) from root to a descendant node.
fn ancestor_path(root: &SyntaxNode, target: &SyntaxNode) -> Vec<usize> {
    let mut path = Vec::new();
    let mut node = target.clone();
    while node.text_range() != root.text_range() || node.kind() != root.kind() {
        let Some(parent) = node.parent() else { break };
        path.push(node.index());
        node = parent;
    }
    path.reverse();
    path
}

/// Navigate a `GreenNode` tree using child indices to reach a descendant.
fn get_green_at_path(root: &GreenNode, path: &[usize]) -> GreenNode {
    let mut current = root.clone();
    for &idx in path {
        current = match current.children().nth(idx) {
            Some(NodeOrToken::Node(n)) => n.to_owned(),
            _ => return current,
        };
    }
    current
}

/// Rebuild the tree from the container up to the root, replacing each
/// ancestor's child with the updated version.
fn rebuild_ancestors(root: &GreenNode, path: &[usize], new_leaf: GreenNode) -> GreenNode {
    if path.is_empty() {
        return new_leaf;
    }

    // Navigate to each ancestor, then rebuild bottom-up.
    let mut greens: Vec<GreenNode> = Vec::with_capacity(path.len());
    let mut current = root.clone();
    for &idx in path {
        greens.push(current.clone());
        current = match current.children().nth(idx) {
            Some(NodeOrToken::Node(n)) => n.to_owned(),
            _ => return root.clone(),
        };
    }

    let mut replacement = new_leaf;
    for (green, &idx) in greens.iter().rev().zip(path.iter().rev()) {
        replacement = green.replace_child(idx, NodeOrToken::Node(replacement));
    }
    replacement
}

/// Merge old errors with new region errors.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn merge_errors(
    old_errors: &[(String, TextRange)],
    region_errors: &[(String, TextRange)],
    region_start: TextSize,
    region_old_end: TextSize,
    delta: i64,
) -> Vec<(String, TextRange)> {
    let mut result = Vec::new();

    for (msg, range) in old_errors {
        if range.end() <= region_start {
            result.push((msg.clone(), *range));
        } else if range.start() >= region_old_end {
            let shift = |offset: TextSize| -> TextSize {
                let v = i64::from(u32::from(offset)) + delta;
                TextSize::from(v as u32)
            };
            result.push((msg.clone(), TextRange::new(shift(range.start()), shift(range.end()))));
        }
    }

    for (msg, range) in region_errors {
        let abs_start = range.start() + region_start;
        let abs_end = range.end() + region_start;
        result.push((msg.clone(), TextRange::new(abs_start, abs_end)));
    }

    result
}
