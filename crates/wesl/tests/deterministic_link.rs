use wesl::{CompileOptions, compile, resolver::VirtualResolver};

/// Linking the same modules must always produce the same WGSL text.
#[test]
fn link_output_is_deterministic() {
    let modules = ["a", "b", "c", "d", "e", "f", "g", "h"];

    let mut resolver = VirtualResolver::new();
    let root = format!(
        "{}\n@compute @workgroup_size(1) fn main() {{ {} }}",
        modules
            .iter()
            .map(|name| format!("import package::{name}::f_{name};"))
            .collect::<Vec<_>>()
            .join("\n"),
        modules
            .iter()
            .map(|name| format!("f_{name}();"))
            .collect::<String>(),
    );
    resolver.add_module("package::main".parse().unwrap(), root.into());
    for name in modules {
        let source = format!("fn f_{name}() {{}}");
        resolver.add_module(format!("package::{name}").parse().unwrap(), source.into());
    }

    let main_path = "package::main".parse().unwrap();
    let options = CompileOptions::default();
    let link = || {
        compile(&main_path, &options, &resolver)
            .unwrap()
            .to_string()
    };

    let first = link();
    for _ in 0..16 {
        assert_eq!(link(), first);
    }
}
