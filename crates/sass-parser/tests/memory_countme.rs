//! countme-based allocation tracking for rowan GreenNode/GreenToken (Task 5.13).
//! Separate binary from dhat tests to avoid allocator conflicts.

static NORMALIZE_CSS: &str = include_str!("fixtures/normalize.css");

fn generate_large_scss(target_bytes: usize) -> String {
    let block = r"
.component-#{$i} {
  $color: hsl($i * 10, 50%, 50%);
  color: $color;
  margin: #{$i}px #{$i * 2}px;
  padding: 10px 20px;

  &:hover {
    color: darken($color, 10%);
  }

  &__inner {
    display: flex;
    align-items: center;
  }
}
";
    let mut buf = String::with_capacity(target_bytes + block.len());
    buf.push_str("$i: 1;\n");
    while buf.len() < target_bytes {
        buf.push_str(block);
    }
    buf
}

#[test]
fn countme_normalize_css() {
    countme::enable(true);

    let (green, errors) = sass_parser::parse_scss(NORMALIZE_CSS);
    let _tree = sass_parser::syntax::SyntaxNode::new_root(green);

    let nodes = countme::get::<rowan::GreenNode>();
    let tokens = countme::get::<rowan::GreenToken>();

    assert!(
        errors.is_empty(),
        "normalize.css should parse without errors"
    );

    eprintln!(
        "── countme: normalize.css ({} bytes) ──",
        NORMALIZE_CSS.len()
    );
    eprintln!(
        "  GreenNode:  total={:>6}, live={:>6}, max_live={:>6}",
        nodes.total, nodes.live, nodes.max_live
    );
    eprintln!(
        "  GreenToken: total={:>6}, live={:>6}, max_live={:>6}",
        tokens.total, tokens.live, tokens.max_live
    );
    eprintln!(
        "  Ratio:      {:.1} tokens/node",
        tokens.total as f64 / nodes.total.max(1) as f64
    );

    assert!(nodes.total > 0, "should create GreenNodes");
    assert!(tokens.total > 0, "should create GreenTokens");
    assert!(nodes.live <= nodes.total);
}

#[test]
fn countme_large_scss() {
    countme::enable(true);

    let baseline_nodes = countme::get::<rowan::GreenNode>().total;
    let baseline_tokens = countme::get::<rowan::GreenToken>().total;

    let source = generate_large_scss(1_000_000);
    let (green, _errors) = sass_parser::parse_scss(&source);
    let _tree = sass_parser::syntax::SyntaxNode::new_root(green);

    let nodes = countme::get::<rowan::GreenNode>();
    let tokens = countme::get::<rowan::GreenToken>();

    let new_nodes = nodes.total - baseline_nodes;
    let new_tokens = tokens.total - baseline_tokens;
    let input_kb = source.len() / 1024;

    eprintln!("── countme: generated SCSS ({input_kb} KB) ──");
    eprintln!("  GreenNode:  total={new_nodes:>6}");
    eprintln!("  GreenToken: total={new_tokens:>6}");
    eprintln!("  Nodes/KB:   {:.1}", new_nodes as f64 / input_kb as f64);
    eprintln!("  Tokens/KB:  {:.1}", new_tokens as f64 / input_kb as f64);

    assert!(new_nodes > 100, "large input should create many nodes");
    assert!(new_tokens > 100, "large input should create many tokens");
}
