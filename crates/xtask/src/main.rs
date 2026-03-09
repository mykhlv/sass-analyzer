mod codegen;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("codegen") => codegen::generate()?,
        other => {
            eprintln!("unknown task: {}", other.unwrap_or("<none>"));
            eprintln!("usage: cargo xtask codegen");
            std::process::exit(1);
        }
    }
    Ok(())
}
