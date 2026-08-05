fn main() {
    wesl::PkgBuilder::new("c")
        .scan_root("src/main")
        .expect("failed to scan WESL files")
        .validate()
        .inspect_err(|e| eprintln!("{}", e.diagnostic().colored()))
        .expect("validation error")
        .build_artifact()
        .expect("failed to build artifact");
}
