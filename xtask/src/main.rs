use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("update_grammar") => update_grammar(),
        _ => {
            eprintln!("usage: cargo xtask <command>");
            eprintln!();
            eprintln!("commands:");
            eprintln!("    update_grammar    run lalrpop on crates/wgsl-parse/src/grammar.lalrpop");
            ExitCode::FAILURE
        }
    }
}

fn update_grammar() -> ExitCode {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().expect("xtask has a parent directory");
    let grammar = workspace_root.join("crates/wgsl-parse/src/grammar.lalrpop");

    println!("generating grammar from {}", grammar.display());
    lalrpop::Configuration::new()
        .set_out_dir(grammar.parent().expect("grammar file has a parent directory"))
        .process_file(&grammar)
        .unwrap();
    println!("done");
    ExitCode::SUCCESS
}
