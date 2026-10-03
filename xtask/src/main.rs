mod update_grammar;
mod wesl_rs_web;

use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("update_grammar") => update_grammar::run(&workspace_root()),
        Some("wesl-rs-web") => wesl_rs_web::run(&workspace_root(), &args[1..]),
        _ => {
            eprintln!("usage: cargo xtask <command>");
            eprintln!();
            eprintln!("commands:");
            eprintln!("    update_grammar    run lalrpop on crates/wgsl-parse/src/grammar.lalrpop");
            eprintln!("    wesl-rs-web          build the wesl-rs-web npm package into dist/");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("xtask has a parent directory")
        .to_path_buf()
}
