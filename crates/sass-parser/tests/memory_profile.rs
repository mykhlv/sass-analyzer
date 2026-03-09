//! dhat heap profiling for sass-parser (Task 5.13).
//!
//! Uses dhat as global allocator to track all heap allocations.
//! Run: `cargo test --test memory_profile -- --test-threads=1 --nocapture`

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

static NORMALIZE_CSS: &str = include_str!("fixtures/normalize.css");

fn generate_large_scss(target_bytes: usize) -> String {
    let block = r#"
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
"#;
    let mut buf = String::with_capacity(target_bytes + block.len());
    buf.push_str("$i: 1;\n");
    while buf.len() < target_bytes {
        buf.push_str(block);
    }
    buf
}

#[test]
fn dhat_heap_profile() {
    let _profiler = dhat::Profiler::builder().testing().build();

    // --- normalize.css (6 KB) ---
    let (green, _) = sass_parser::parse(NORMALIZE_CSS);
    let _tree = sass_parser::syntax::SyntaxNode::new_root(green);

    let stats = dhat::HeapStats::get();
    eprintln!("── dhat: normalize.css ({} bytes) ──", NORMALIZE_CSS.len());
    eprintln!("  Total blocks:     {:>8}", stats.total_blocks);
    eprintln!("  Total bytes:      {:>8}", stats.total_bytes);
    eprintln!("  Max blocks live:  {:>8}", stats.max_blocks);
    eprintln!("  Max bytes live:   {:>8}", stats.max_bytes);
    eprintln!(
        "  Bytes/input byte: {:.1}",
        stats.total_bytes as f64 / NORMALIZE_CSS.len() as f64
    );

    assert!(stats.total_blocks > 0, "should have heap allocations");

    // --- large SCSS (~1 MB) ---
    drop(_tree);

    let source = generate_large_scss(1_000_000);
    let blocks_before = dhat::HeapStats::get().total_blocks;

    let (green, _) = sass_parser::parse(&source);
    let _tree = sass_parser::syntax::SyntaxNode::new_root(green);

    let stats = dhat::HeapStats::get();
    let new_blocks = stats.total_blocks - blocks_before;
    let input_kb = source.len() / 1024;

    eprintln!("── dhat: generated SCSS ({input_kb} KB) ──");
    eprintln!("  New blocks:       {:>8}", new_blocks);
    eprintln!("  Max bytes live:   {:>8}", stats.max_bytes);
    eprintln!(
        "  Allocs/KB input:  {:.1}",
        new_blocks as f64 / input_kb as f64
    );

    assert!(new_blocks > 100, "large parse should allocate many blocks");
}
