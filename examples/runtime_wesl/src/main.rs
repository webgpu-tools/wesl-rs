fn main() {
    let source = wesl::Wesl::new("src/shaders")
        .add_package(&random_wgsl::PACKAGE)
        .compile(&"package::main".parse().unwrap())
        .inspect_err(|e| eprintln!("{e}"))
        .unwrap()
        .to_string();

    println!("{source}");
}
