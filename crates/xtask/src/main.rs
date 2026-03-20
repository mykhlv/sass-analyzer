mod codegen;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str);
    match task {
        Some("codegen") => {
            let check = args.iter().any(|a| a == "--check");
            codegen::generate(check)?;
        }
        other => {
            eprintln!("unknown task: {}", other.unwrap_or("<none>"));
            eprintln!("usage: cargo xtask codegen [--check]");
            std::process::exit(1);
        }
    }
    Ok(())
}
