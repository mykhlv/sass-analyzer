//! Tree-shape fingerprints for the baseline files of the corpus and sass-spec suites.
//!
//! Error counts and the round-trip check are both blind to a tree that is wrong but
//! quiet: a declaration absorbing the next one, or a rule swallowed by an unclosed
//! bracket, leaves the error count and the source text unchanged. Recording the shape
//! of each tree beside its error count turns such an absorption into a diff.
//!
//! Included with `#[path]` by the test binaries that run it, rather than through
//! `common/mod.rs`, so that binaries which do not are not left with dead code.

use std::collections::BTreeMap;
use std::fmt::Write;

use sass_parser::syntax::{SyntaxNode, debug_tree};
use sass_parser::syntax_kind::SyntaxKind;

/// Names of the columns of a baseline record, in order.
pub const COLUMNS: [&str; 7] = [
    "corpus",
    "path",
    "errors",
    "rule_sets",
    "declarations",
    "blocks",
    "tree_hash",
];

/// A cheap summary of a tree's structure, recorded per file in the baselines.
///
/// The node counts make a change readable in a diff; the hash is what catches a
/// change the counts cannot see, such as a rule moved under a different parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    pub rule_sets: usize,
    pub declarations: usize,
    pub blocks: usize,
    pub tree_hash: u64,
}

impl Shape {
    pub fn of(tree: &SyntaxNode) -> Self {
        let mut shape = Shape {
            rule_sets: 0,
            declarations: 0,
            blocks: 0,
            tree_hash: fnv1a(debug_tree(tree).as_bytes()),
        };
        for node in tree.descendants() {
            match node.kind() {
                SyntaxKind::RULE_SET => shape.rule_sets += 1,
                SyntaxKind::DECLARATION => shape.declarations += 1,
                SyntaxKind::BLOCK => shape.blocks += 1,
                _ => {}
            }
        }
        shape
    }

    /// One baseline record: `<group>\t<path>\t<errors>\t<shape columns>`.
    pub fn record(&self, group: &str, path: &str, errors: usize) -> String {
        format!(
            "{group}\t{}\t{errors}\t{}\t{}\t{}\t{:016x}",
            path.replace('\\', "/"),
            self.rule_sets,
            self.declarations,
            self.blocks,
            self.tree_hash,
        )
    }
}

/// 64-bit FNV-1a: tiny, and unlike `DefaultHasher` its output is fixed, so the
/// hashes written to a tracked file mean the same thing on every toolchain.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Key of a baseline record: its `<group>\t<path>` prefix.
pub fn record_key(record: &str) -> &str {
    let mut tabs = record.match_indices('\t').map(|(i, _)| i);
    let _ = tabs.next();
    tabs.next().map_or(record, |end| &record[..end])
}

/// The columns of a changed record that differ, as `name old → new`.
fn changed_columns(before: &str, after: &str) -> String {
    let before: Vec<&str> = before.split('\t').collect();
    let after: Vec<&str> = after.split('\t').collect();
    let len = before.len().max(after.len());
    (2..len)
        .filter_map(|i| {
            let old = before.get(i).copied().unwrap_or("-");
            let new = after.get(i).copied().unwrap_or("-");
            (old != new).then(|| {
                let name = COLUMNS.get(i).copied().unwrap_or("column");
                format!("{name} {old} → {new}")
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Describe how the records differ, at most a few lines per category, or `None`
/// when they are the same set.
pub fn describe_record_diff(old: &[&str], new: &[String]) -> Option<String> {
    const SHOWN: usize = 10;
    let old: BTreeMap<&str, &str> = old.iter().map(|r| (record_key(r), *r)).collect();
    let new: BTreeMap<&str, &str> = new.iter().map(|r| (record_key(r), r.as_str())).collect();

    let changed: Vec<String> = new
        .iter()
        .filter_map(|(key, record)| {
            let before = old.get(key)?;
            (before != record).then(|| format!("  {key}: {}", changed_columns(before, record)))
        })
        .collect();
    let missing: Vec<&str> = old
        .keys()
        .filter(|key| !new.contains_key(*key))
        .copied()
        .collect();
    let added: Vec<&str> = new
        .keys()
        .filter(|key| !old.contains_key(*key))
        .copied()
        .collect();
    if changed.is_empty() && missing.is_empty() && added.is_empty() {
        return None;
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} changed, {} no longer parsed, {} new",
        changed.len(),
        missing.len(),
        added.len(),
    );
    for line in changed.iter().take(SHOWN) {
        let _ = writeln!(out, "{line}");
    }
    if changed.len() > SHOWN {
        let _ = writeln!(out, "  ... and {} more changed", changed.len() - SHOWN);
    }
    for key in missing.iter().take(SHOWN) {
        let _ = writeln!(out, "  missing: {key}");
    }
    for key in added.iter().take(SHOWN) {
        let _ = writeln!(out, "  new: {key}");
    }
    Some(out)
}
