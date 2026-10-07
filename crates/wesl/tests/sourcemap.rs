//! Tests for [`wesl::sourcemap`]: mapping naga's byte ranges back to the original sources.

use std::borrow::Cow;

use wesl::{
    CompileOptions, CompileResult, Compiler,
    resolver::VirtualResolver,
    sourcemap::{BasicSourceMap, MappedDiagnostic, SourceMap, SourceMapEntry},
    syntax::ModulePath,
};

const MAIN: &str = "
import package::util::helper;

@compute @workgroup_size(1)
fn main() {
    let r = helper();
}
";

fn compile(files: &[(&str, &str)], options: CompileOptions) -> CompileResult {
    let mut resolver = VirtualResolver::new();
    for (path, src) in files {
        resolver.add_module(
            path.parse::<ModulePath>().unwrap(),
            Cow::Owned(src.to_string()),
        );
    }
    Compiler::new_with_resolver(options, resolver)
        .compile_root()
        .expect("WESL itself does not catch these errors")
}

/// The adapters one writes in a build script to turn naga errors into labels.
fn naga_diagnostic(result: &CompileResult) -> Option<MappedDiagnostic> {
    let map = result.sourcemap().unwrap();
    let module = match naga::front::wgsl::parse_str(result.wgsl()) {
        Ok(module) => module,
        Err(e) => {
            return Some(
                map.diagnostic(
                    e.message(),
                    e.labels()
                        .filter_map(|(span, msg)| Some((span.to_range()?, msg.to_string()))),
                ),
            );
        }
    };
    let result = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module);
    match result {
        Ok(_) => None,
        Err(e) => Some(
            map.diagnostic_from_error(
                e.as_inner(),
                e.spans()
                    .filter_map(|(span, msg)| Some((span.to_range()?, msg.clone()))),
            ),
        ),
    }
}

fn diagnostic(files: &[(&str, &str)]) -> MappedDiagnostic {
    let result = compile(files, CompileOptions::default());
    naga_diagnostic(&result).expect("naga should reject this program")
}

/// the label of the most specific node, which naga reports last.
fn primary(diag: &MappedDiagnostic) -> &wesl::sourcemap::SourceLocation {
    diag.labels
        .iter()
        .rev()
        .find_map(|l| l.location.as_ref())
        .expect("a label should resolve to the original source")
}

#[test]
fn parse_error_in_imported_module_points_to_that_module() {
    let util = "
// some padding so offsets in the generated code differ from offsets in this file
public fn helper() -> f32 {
    let x: f32 = 1u;
    return x;
}
";
    let diag = diagnostic(&[("package", MAIN), ("package::util", util)]);
    let loc = primary(&diag);
    // naga points at the definition of `x`: an exact range inside the original file.
    assert_eq!(loc.module.to_string(), "package::util");
    assert_eq!(loc.snippet(), "x");
    assert!(loc.exact);
    assert_eq!((loc.line, loc.column), (4, 9), "{loc}");
    assert_eq!(loc.source(), util);
    assert!(diag.labels[0].text.contains("definition of `x`"));
}

#[test]
fn parse_error_in_main_module_points_to_main() {
    let main = "
@compute @workgroup_size(1)
fn main() {
    let x: f32 = 1u;
}
";
    let diag = diagnostic(&[("package", main)]);
    let loc = primary(&diag);
    assert_eq!(loc.module.to_string(), "package");
    assert_eq!(loc.snippet(), "x");
    assert_eq!((loc.line, loc.column), (4, 9));
}

#[test]
fn validation_error_points_to_the_offending_function() {
    // a function that does not return: reported by the naga validator, not its parser.
    let util = "
public fn helper() -> f32 {
}
";
    let diag = diagnostic(&[("package", MAIN), ("package::util", util)]);
    let loc = primary(&diag);
    assert_eq!(loc.module.to_string(), "package::util");
    assert!(
        loc.snippet().contains("fn helper"),
        "snippet: {:?}",
        loc.snippet()
    );
    assert_eq!(loc.line, 2);
}

#[test]
fn ranges_inside_an_unchanged_node_resolve_exactly() {
    let main = "
@compute @workgroup_size(1)
fn main() {
    let value = 12345u + 1u;
}
";
    let result = compile(&[("package", main)], CompileOptions::default());
    let map = result.sourcemap().unwrap();
    let start = result.wgsl().find("12345u").unwrap();
    let loc = map
        .destination_to_source(start..start + "12345u".len())
        .unwrap();
    assert!(loc.exact);
    assert_eq!(loc.snippet(), "12345u");
    assert_eq!((loc.line, loc.column), (4, 17));
}

#[test]
fn sorted_declarations_still_resolve_to_their_files() {
    let util = "public const FIRST: u32 = 1u;\npublic fn helper() -> u32 { return FIRST; }";
    let result = compile(
        &[("package", MAIN), ("package::util", util)],
        CompileOptions {
            sort_declarations: true,
            ..Default::default()
        },
    );
    let map = result.sourcemap().unwrap();
    let emitted = result.wgsl();
    assert_eq!(emitted, result.to_string());
    let start = emitted.find("fn package_util_helper").unwrap() + "fn ".len();
    let loc = map
        .destination_to_source(start..start + "package_util_helper".len())
        .unwrap();
    assert_eq!(loc.module.to_string(), "package::util");
}

#[test]
fn ranges_in_reformatted_or_renamed_nodes_widen_to_the_node() {
    // the call is printed with different whitespace and a mangled callee name.
    let main = "
import package::util::helper;

@compute @workgroup_size(1)
fn main() {
    let r =   helper(  );
}
";
    let util = "public fn helper() -> f32 { return 1.0; }";
    let result = compile(
        &[("package", main), ("package::util", util)],
        CompileOptions::default(),
    );
    let map = result.sourcemap().unwrap();
    let start = result.wgsl().find("package_util_helper()").unwrap();
    let loc = map
        .destination_to_source(start..start + "package_util_helper".len())
        .unwrap();
    assert!(!loc.exact);
    assert_eq!(loc.module.to_string(), "package");
    assert!(
        loc.snippet().contains("helper("),
        "snippet: {:?}",
        loc.snippet()
    );
}

#[test]
fn mangled_names_are_demangled_in_messages() {
    let util = "public fn helper() -> f32 { return 1.0; }";
    let result = compile(
        &[("package", MAIN), ("package::util", util)],
        CompileOptions::default(),
    );
    assert!(result.to_string().contains("package_util_helper"));
    let map = result.sourcemap().unwrap();
    assert_eq!(
        map.demangle_message("`package_util_helper` is not a function"),
        "`package::util::helper` is not a function"
    );
    // other identifiers are untouched, even if a mangled name is a prefix of them.
    assert_eq!(
        map.demangle_message("package_util_helper2 main"),
        "package_util_helper2 main"
    );
}

#[test]
fn rendered_diagnostic_shows_the_original_file_and_line() {
    let util = "
public fn helper() -> f32 {
    let x: f32 = 1u;
    return x;
}
";
    let diag = diagnostic(&[("package", MAIN), ("package::util", util)]);
    let text = diag.render_plain();
    assert!(text.contains("package::util"), "{text}");
    assert!(text.contains("let x: f32 = 1u;"), "{text}");
    // no trace of the mangled name or the generated layout
    assert!(!text.contains("package_util_helper"), "{text}");
    assert!(!text.contains("<generated WGSL>"), "{text}");
}

#[test]
fn unresolvable_labels_fall_back_to_the_generated_code() {
    let result = compile(
        &[
            ("package", MAIN),
            ("package::util", "public fn helper() -> f32 { return 1.0; }"),
        ],
        CompileOptions::default(),
    );
    let map = result.sourcemap().unwrap();
    // a range past the end of the generated code cannot be resolved.
    let len = result.wgsl().len();
    let diag = map.diagnostic("oops", [(len + 10..len + 12, "somewhere".to_string())]);
    assert!(diag.labels[0].location.is_none());
    assert!(diag.render_plain().contains("somewhere"));
}

#[test]
fn sourcemaps_demangle_the_names_in_a_text() {
    let mut map = BasicSourceMap::new();
    let entry = SourceMapEntry {
        path: "package::util".parse::<ModulePath>().unwrap(),
        name: "helper".to_string(),
        span: None,
    };
    map.add_item("package_util_helper".to_string(), entry);
    let text = map.demangle_message("cannot call package_util_helper() with main");
    assert_eq!(text, "cannot call package::util::helper() with main");
}

#[test]
fn stripped_and_unstripped_output_both_map() {
    let util = "
public fn helper() -> f32 {
    let x: f32 = 1u;
    return x;
}
fn unused() {}
";
    for strip in [false, true] {
        let result = compile(
            &[("package", MAIN), ("package::util", util)],
            CompileOptions {
                strip,
                ..Default::default()
            },
        );
        let diag = naga_diagnostic(&result).unwrap();
        assert_eq!(
            primary(&diag).module.to_string(),
            "package::util",
            "strip={strip}"
        );
    }
}

#[test]
fn large_fixture_resolves_every_function() {
    // the same program as the `compile_wesl_directory` snapshot test.
    mod package_random {
        use wesl_core::{StaticPackage, StaticPackageModule};
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/package_random.rs"
        ));
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/compile_wesl/shaders");
    for (lower, strip) in [(false, false), (false, true)] {
        let mut compiler = Compiler::default();
        compiler.options.lower = lower;
        compiler.options.strip = strip;
        compiler.options.mangle_main = true;
        let mut constants = wesl::Constants::new();
        constants.set("PI", std::f64::consts::PI);
        constants.set("TRUE", true);
        compiler.options.constants = constants;
        compiler.options.dependencies = vec![&package_random::PACKAGE];
        let result = compiler.compile(&root).unwrap();
        let map = result.sourcemap().unwrap();

        // every function of the output resolves to a function of the original sources.
        let text = result.wgsl();
        let mut checked = 0;
        for (i, _) in text.match_indices("\nfn ") {
            let start = i + 1;
            let loc = map
                .destination_to_source(start..start + 2)
                .expect("function should resolve");
            assert!(loc.snippet().contains("fn "), "{loc}: {:?}", loc.snippet());
            checked += 1;
        }
        assert!(3 < checked, "the fixture should have several functions");
    }
}

#[test]
fn causes_of_validation_errors_become_notes() {
    // two resources with the same binding, declared in different files. naga's top-level
    // message does not say what is wrong, the cause is in the chain of sources.
    let main = "
import package::res::light;

@group(0) @binding(0) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(1)
fn main() {
    out[0] = light.x;
}
";
    let res = "
// the same binding as `out` in the main module
@group(0) @binding(0) public var<uniform> light: vec4f;
";
    let diag = diagnostic(&[("package", main), ("package::res", res)]);
    let loc = primary(&diag);
    assert_eq!(loc.module.to_string(), "package::res");
    assert!(
        loc.snippet().contains("var<uniform> light"),
        "{:?}",
        loc.snippet()
    );
    assert_eq!(loc.line, 3);
    assert!(
        diag.notes.iter().any(|n| n.contains("conflict")),
        "notes: {:?}",
        diag.notes
    );
    let rendered = diag.render_plain();
    assert!(rendered.contains("= Bindings for"), "{rendered}");
    assert!(!rendered.contains("caused by"), "{rendered}");
}

#[test]
fn notes_are_shown_after_the_snippets() {
    let diag = diagnostic(&[
        ("package", MAIN),
        ("package::util", "public fn helper() -> f32 {}"),
    ]);
    let rendered = diag.with_note("see the docs").render_plain();
    assert!(rendered.contains("= note: see the docs"), "{rendered}");
}

#[test]
fn attributes_resolve_to_themselves() {
    let main = "
@compute @workgroup_size(64)
fn main() {}
";
    let result = compile(&[("package", main)], CompileOptions::default());
    let map = result.sourcemap().unwrap();
    let text = "@workgroup_size(64)";
    let start = result.wgsl().find(text).unwrap();
    let loc = map
        .destination_to_source(start..start + text.len())
        .unwrap();
    assert_eq!(loc.snippet(), text);
    assert!(loc.exact);
    assert_eq!(loc.line, 2);
}

#[test]
fn struct_members_resolve_to_themselves() {
    let main = "
struct Light {
    position: vec3f,
    @size(16) color: vec3f,
}

@group(0) @binding(0) var<uniform> light: Light;

@compute @workgroup_size(1)
fn main() {
    let c = light.color;
}
";
    let result = compile(&[("package", main)], CompileOptions::default());
    let map = result.sourcemap().unwrap();
    let text = "@size(16)\ncolor: vec3<f32>";
    let start = result.wgsl().find("@size(16)").unwrap();
    let loc = map
        .destination_to_source(start..start + text.len())
        .unwrap();
    assert_eq!(loc.snippet(), "@size(16) color: vec3f");
    assert!(!loc.exact);
    assert_eq!(loc.line, 4);
}

#[test]
fn lowered_expressions_resolve_to_the_expression_they_replace() {
    let main = "
const N = 4u;

fn f() -> u32 {
    return N + 1u;
}
";
    let result = compile(
        &[("package", main)],
        CompileOptions {
            lower: true,
            strip: false,
            ..Default::default()
        },
    );
    let map = result.sourcemap().unwrap();

    let (lowered, original) = match cfg!(feature = "eval") {
        true => ("5u", "N + 1u"),
        false => ("4u", "N"),
    };
    let start = result.wgsl().find(lowered).unwrap();
    let loc = map
        .destination_to_source(start..start + lowered.len())
        .unwrap();
    assert_eq!(loc.module.to_string(), "package");
    assert_eq!(loc.snippet(), original);
    assert!(!loc.exact);
    assert_eq!(loc.line, 5);
}

#[test]
fn code_from_several_modules_keeps_its_own_file() {
    let a = "public fn a() -> f32 { return 1.0; }";
    let b = "\n\n\npublic fn b() -> f32 { return 2.0; }";
    let main = "
import package::a::a;
import package::b::b;

@compute @workgroup_size(1)
fn main() {
    let x = a() + b();
}
";
    let result = compile(
        &[("package", main), ("package::a", a), ("package::b", b)],
        CompileOptions::default(),
    );
    let map = result.sourcemap().unwrap();
    for (needle, module, line) in [
        ("return 1.0", "package::a", 1),
        ("return 2.0", "package::b", 4),
        ("let x", "package", 7),
    ] {
        let start = result.wgsl().find(needle).unwrap();
        let loc = map
            .destination_to_source(start..start + needle.len())
            .unwrap();
        assert_eq!(loc.module.to_string(), module, "{needle}");
        assert_eq!(loc.line, line, "{needle}");
    }
}
