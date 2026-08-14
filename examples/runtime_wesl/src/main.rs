fn main() {
    let source = wesl::Compiler::new(wesl::CompileOptions {
        dependencies: vec![&random_wgsl::PACKAGE],
        ..Default::default()
    })
    .compile("src/shaders/main.wesl")
    .inspect_err(|e| {
        eprintln!("{e}");
        panic!();
    })
    .unwrap()
    .to_string();

    println!("{source}");
}
