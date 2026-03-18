use super::*;

use crate::completion::fuzzy_score;

// ── Workspace symbol tests ──────────────────────────────────────

#[tokio::test]
async fn initialize_reports_workspace_symbol_capability() {
    let (mut reader, mut writer) = spawn_server();
    let resp = do_initialize(&mut reader, &mut writer).await;
    let caps = &resp["result"]["capabilities"];
    assert_eq!(caps["workspaceSymbolProvider"], true);
}

#[tokio::test]
async fn workspace_symbol_returns_matching_symbols() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$primary: blue;\n@mixin btn($size) { }\n@function scale($n) { @return $n; }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // Search for "btn"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 70,
            "method": "workspace/symbol",
            "params": { "query": "btn" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "btn");
}

#[tokio::test]
async fn workspace_symbol_empty_query_returns_all() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$a: 1;\n$b: 2;";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws_all.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 71,
            "method": "workspace/symbol",
            "params": { "query": "" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 2, "empty query should return all symbols");
}

#[tokio::test]
async fn workspace_symbol_fuzzy_matching() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "@mixin responsive-grid { }\n@mixin simple { }";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws_fuzz.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    // "rg" should fuzzy-match "responsive-grid" but not "simple"
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 72,
            "method": "workspace/symbol",
            "params": { "query": "rg" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    let result = resp["result"].as_array().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "responsive-grid");
}

#[tokio::test]
async fn workspace_symbol_no_match_returns_null() {
    let (mut reader, mut writer) = spawn_server();
    do_initialize(&mut reader, &mut writer).await;

    let scss = "$x: 1;";
    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///ws_none.scss",
                    "languageId": "scss",
                    "version": 1,
                    "text": scss
                }
            }
        }),
    )
    .await;
    let _diag = recv_msg(&mut reader, &mut writer).await;

    send_msg(
        &mut writer,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 73,
            "method": "workspace/symbol",
            "params": { "query": "zzz" }
        }),
    )
    .await;

    let resp = recv_msg(&mut reader, &mut writer).await;
    assert!(resp["result"].is_null(), "no match should return null");
}

#[test]
fn fuzzy_score_basics() {
    // Exact match → highest score.
    assert_eq!(fuzzy_score("color", "color"), Some(1000));
    // Prefix match → 500+.
    assert!(fuzzy_score("color-primary", "color").unwrap() >= 500);
    // Word boundary match → 200+ (r and g match starts of "responsive" and "grid").
    let rg_score = fuzzy_score("responsive-grid", "rg").unwrap();
    assert!(
        rg_score >= 200,
        "word boundary should score 200+, got {rg_score}"
    );
    // Subsequence match → >0.
    assert!(fuzzy_score("primary", "pry").unwrap() > 0);
    // No match → None.
    assert_eq!(fuzzy_score("simple", "rg"), None);
    // Empty query → matches everything.
    assert_eq!(fuzzy_score("anything", ""), Some(0));
}

#[test]
fn fuzzy_score_ranking() {
    let exact = fuzzy_score("color", "color").unwrap();
    let prefix = fuzzy_score("color-primary", "color").unwrap();
    let boundary = fuzzy_score("responsive-grid", "rg").unwrap();
    let subseq = fuzzy_score("primary", "pry").unwrap();
    assert!(exact > prefix, "exact > prefix");
    assert!(prefix > boundary, "prefix > boundary");
    assert!(boundary > subseq, "boundary > subsequence");
}

#[test]
fn fuzzy_score_camel_case_boundary() {
    // camelCase boundary: "bc" matches "B" from "border" and "C" from "Color"
    let score = fuzzy_score("borderColor", "bc").unwrap();
    assert!(score >= 200, "camelCase boundary match, got {score}");
}

#[test]
fn completion_context_detection() {
    use crate::completion::{CompletionContext, detect_completion_context};

    // After `$` → Variable
    let ctx = detect_completion_context("  color: $", 10);
    assert!(matches!(ctx, CompletionContext::Variable));

    // After `@include ` → IncludeMixin
    let ctx = detect_completion_context("  @include ", 11);
    assert!(matches!(ctx, CompletionContext::IncludeMixin));

    // After `@use "` → UseModulePath
    let ctx = detect_completion_context("  @use \"", 8);
    assert!(matches!(ctx, CompletionContext::UseModulePath(_)));

    // On `bor` → PropertyName
    let ctx = detect_completion_context("  bor", 5);
    assert!(matches!(ctx, CompletionContext::PropertyName(_)));

    // After `color:` → PropertyValue with property name and partial
    let ctx = detect_completion_context("  color: ", 9);
    assert!(
        matches!(ctx, CompletionContext::PropertyValue(ref p, ref v) if p == "color" && v.is_empty()),
        "expected PropertyValue(\"color\", \"\"), got {ctx:?}"
    );

    // After `display: fl` → PropertyValue with partial
    let ctx = detect_completion_context("  display: fl", 13);
    assert!(
        matches!(ctx, CompletionContext::PropertyValue(ref p, ref v) if p == "display" && v == "fl"),
        "expected PropertyValue(\"display\", \"fl\"), got {ctx:?}"
    );

    // Pseudo-selectors must NOT be detected as PropertyValue
    let ctx = detect_completion_context("  a:hover", 9);
    assert!(
        !matches!(ctx, CompletionContext::PropertyValue(..)),
        "a:hover should not be PropertyValue, got {ctx:?}"
    );

    let ctx = detect_completion_context("  &:focus", 9);
    assert!(
        !matches!(ctx, CompletionContext::PropertyValue(..)),
        "&:focus should not be PropertyValue, got {ctx:?}"
    );

    let ctx = detect_completion_context("  :root", 7);
    assert!(
        !matches!(ctx, CompletionContext::PropertyValue(..)),
        ":root should not be PropertyValue, got {ctx:?}"
    );

    // Decimal number must NOT trigger Namespace (e.g., `font-size: 1.`)
    let ctx = detect_completion_context("  font-size: 1.", 15);
    assert!(
        !matches!(ctx, CompletionContext::Namespace(..)),
        "decimal 1. should not be Namespace, got {ctx:?}"
    );

    // @include with namespace prefix → Namespace, not IncludeMixin
    // "  @include math." — after dot = col 16
    let ctx = detect_completion_context("  @include math.", 16);
    assert!(
        matches!(ctx, CompletionContext::Namespace(ref ns, 16) if ns == "math"),
        "expected Namespace(\"math\", 16), got {ctx:?}"
    );

    // @include without namespace → IncludeMixin
    let ctx = detect_completion_context("  @include btn", 14);
    assert!(matches!(ctx, CompletionContext::IncludeMixin));

    // @extend → Extend
    let ctx = detect_completion_context("  @extend %btn", 14);
    assert!(matches!(ctx, CompletionContext::Extend));

    // Namespace inside parentheses: `@include t.subtitle1(cc.`
    // "cc" at col 23, dot at 25, after dot = 26
    let ctx = detect_completion_context("  @include t.subtitle1(cc.", 26);
    assert!(
        matches!(ctx, CompletionContext::Namespace(ref ns, 26) if ns == "cc"),
        "expected Namespace(\"cc\", 26), got {ctx:?}"
    );

    // Namespace variable inside parentheses: `@include t.subtitle1(cc.$`
    let ctx = detect_completion_context("  @include t.subtitle1(cc.$", 27);
    assert!(
        matches!(ctx, CompletionContext::Namespace(ref ns, 26) if ns == "cc"),
        "expected Namespace(\"cc\", 26) for cc.$, got {ctx:?}"
    );

    // Namespace with cursor mid-line (text after cursor): `c.` with `)` after
    // "c" at col 23, dot at 24, after dot = 25
    let ctx = detect_completion_context("  @include t.subtitle1(c.)", 25);
    assert!(
        matches!(ctx, CompletionContext::Namespace(ref ns, 25) if ns == "c"),
        "expected Namespace(\"c\", 25) mid-line, got {ctx:?}"
    );
}
