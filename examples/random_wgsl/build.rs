fn main() {
    wesl::PackageBuilder::new("random")
        .scan_root("src/shaders")
        .expect("failed to scan WESL files")
        .validate()
        .inspect_err(|e| {
            eprintln!("{e}");
            panic!();
        })
        .unwrap()
        .build_artifact()
        .expect("failed to build artifact")
}
