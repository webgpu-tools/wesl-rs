fn main() {
    let shader = wesl::Compiler::new(wesl::CompileOptions {
        dependencies: vec![&a::a::PACKAGE, &b::b::PACKAGE],
        ..Default::default()
    })
    .compile_module("src", &"package::main".parse().unwrap())
    .inspect_err(|e| eprintln!("{e}"))
    .expect("compilation error")
    .to_string();

    println!("{shader}");
}
