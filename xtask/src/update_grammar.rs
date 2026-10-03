//! Regenerates the wgsl-parse parser from `crates/wgsl-parse/src/grammar.lalrpop`.

use std::{path::Path, process::ExitCode};

pub(crate) fn run(workspace_root: &Path) -> ExitCode {
    let grammar = workspace_root.join("crates/wgsl-parse/src/grammar.lalrpop");

    println!("generating grammar from {}", grammar.display());
    lalrpop::Configuration::new()
        .set_out_dir(
            grammar
                .parent()
                .expect("grammar file has a parent directory"),
        )
        .process_file(&grammar)
        .unwrap();
    println!("done");
    ExitCode::SUCCESS
}
