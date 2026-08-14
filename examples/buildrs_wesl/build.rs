fn main() {
    wesl::Compiler::new(wesl::CompileOptions {
        dependencies: vec![&random_wgsl::PACKAGE],
        ..Default::default()
    })
    .compile("src/shaders/")
    .inspect_err(|e| eprintln!("{e}")) // pretty-print errors
    .expect("compilation error")
    .write_artifact("main");
}
