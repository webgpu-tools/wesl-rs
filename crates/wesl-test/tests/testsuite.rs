//! JSON Tests from the `wesl-testsuite`
//! See schemas: https://github.com/webgpu-tools/wesl-testsuite/blob/main/src/TestSchema.ts
//!
//! These tests are run with `harness = false` in `Cargo.toml`, because they rely on the
//! `libtest_mimic` custom harness to generate tests at runtime based on the JSON files.

use std::{ffi::OsStr, path::PathBuf, str::FromStr};

use expect_test::expect_file;
use wesl::{
    CompileOptions, Compiler, Features, ManglerKind,
    error::Diagnostic,
    resolver::{Constants, VirtualResolver},
    syntax::*,
};
use wesl_test::schemas::*;

fn eprint_test(case: &Test) {
    eprintln!(
        "case: `{}`\n* desc: {}{}\n* kind: {}\n* expect: {}\n* skip: {}\n* issue: {}",
        case.name,
        case.desc,
        case.note
            .as_ref()
            .map(|n| format!(" (note: {n})"))
            .unwrap_or_default(),
        case.kind,
        case.expect,
        case.skip.unwrap_or(false),
        case.issue.as_deref().unwrap_or("<none>")
    );
}

fn eprint_parsing_test(case: &ParsingTest) {
    let expects = if case.fails {
        "Fail"
    } else {
        "Pass"
    };
    println!("case `{}`: expect {expects}", case.src);
}

fn eprint_wgsl_test(case: &WgslTestSrc) {
    println!(
        "case: `{}`{}",
        case.name,
        case.notes
            .as_ref()
            .map(|n| format!(" (note: {n})"))
            .unwrap_or_default(),
    );
}

fn test_name(path: impl AsRef<std::path::Path>) -> String {
    path.as_ref()
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string()
}

fn main() {
    let mut tests: Vec<libtest_mimic::Trial> = Vec::new();

    let spec_tests = [
        "spec-tests/idents.json",
        "spec-tests/literals.json",
        "spec-tests/lit-type-inference.json",
        "spec-tests/imports.json",
        "spec-tests/circular.json",
        "spec-tests/types.json",
        "spec-tests/dead-code.json",
        "spec-tests/condcomp-flatten.json",
        "spec-tests/visibility.json",
    ];
    for path in spec_tests {
        tests.extend({
            let file = std::fs::read_to_string(path).expect("failed to read test file");
            let json: Vec<Test> = serde_json::from_str(&file).expect("failed to parse json file");
            json.into_iter().map(|case| {
                let name = format!("spec-tests__{}__{}", test_name(path), case.name);
                let ignored = case.skip.unwrap_or(false);
                libtest_mimic::Trial::test(name, move || {
                    json_case(&case).inspect_err(|_| eprint_test(&case))
                })
                .with_ignored_flag(ignored)
            })
        });
    }

    let coverage_tests = ["spec-tests/ctor_coverage.wgsl"];
    for path in coverage_tests {
        tests.push({
            let name = format!("spec-tests__{}", test_name(path));
            libtest_mimic::Trial::test(name.clone(), move || {
                validation_case(name.clone(), PathBuf::from(path))
            })
        });
    }

    {
        let base_dir = std::path::Path::new("wesl-testsuite");
        fetch_git_repository(
            &WgslGitSrc {
                url: "https://github.com/webgpu-tools/wesl-testsuite.git".to_owned(),
                revision: "5e37bc1b5ae6c5559d7d64205808804f9ad29a47".to_owned(),
            },
            base_dir,
        )
        .unwrap_or_else(|_| panic!("failed to fetch bulk test repository"));
    }
    let testsuite_syntax_tests = ["wesl-testsuite/src/test-cases-json/importSyntaxCases.json"];
    for path in testsuite_syntax_tests {
        tests.extend({
            let file = std::fs::read_to_string(path).expect("failed to read test file");
            let json: Vec<ParsingTest> =
                serde_json::from_str(&file).expect("failed to parse json file");
            json.into_iter().map(|mut case| {
                case.normalize();
                let name = format!("testsuite__{}__{}", test_name(path), case.src);
                libtest_mimic::Trial::test(name, move || {
                    testsuite_syntax_case(&case).inspect_err(|_| eprint_parsing_test(&case))
                })
            })
        });
    }

    let testsuite_tests = [
        "wesl-testsuite/src/test-cases-json/importCases.json",
        "wesl-testsuite/src/test-cases-json/conditionalTranslationCases.json",
    ];
    for path in testsuite_tests {
        tests.extend({
            let file = std::fs::read_to_string(path).expect("failed to read test file");
            let json: Vec<WgslTestSrc> =
                serde_json::from_str(&file).expect("failed to parse json file");
            json.into_iter().map(|case| {
                let name = format!("testsuite__{}__{}", test_name(path), case.name);
                let ignored = case.name == "@else with package function reference"; // TODO: update this test in the testsuite, it does not flatten @if
                libtest_mimic::Trial::test(name, move || {
                    testsuite_case(&case).inspect_err(|_| eprint_wgsl_test(&case))
                })
                .with_ignored_flag(ignored)
            })
        });
    }

    tests.extend({
        let file = std::fs::read_to_string("wesl-testsuite/src/test-cases-json/bulkTests.json")
            .expect("failed to read test file");
        let json: Vec<WgslBulkTest> =
            serde_json::from_str(&file).expect("failed to parse json file");
        json.into_iter().flat_map(|bulk_case| {
            let name = format!("bulkTests__{}", test_name(&bulk_case.base_dir));
            let base_dir = std::path::Path::new("wesl-testsuite").join(&bulk_case.base_dir);
            if let Some(git) = &bulk_case.git {
                fetch_git_repository(git, &base_dir)
                    .unwrap_or_else(|_| panic!("failed to fetch bulk test {name}"));
            }

            assert!(
                bulk_case.exclude.is_none_or(|v| v.is_empty()),
                "Globs are not supported"
            );
            let include_paths: Vec<_> = bulk_case
                .include
                .expect("Required include field")
                .iter()
                .map(|v| base_dir.join(v))
                .collect();

            include_paths.into_iter().map(move |path| {
                let name = format!(
                    "{name}__{}",
                    path.strip_prefix(&base_dir).unwrap().display()
                );
                libtest_mimic::Trial::test(name.clone(), move || {
                    validation_case(name.clone(), path)
                })
            })
        })
    });

    tests.extend({
        let entries = std::fs::read_dir("bevy").expect("missing dir `bevy`");
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension() == Some(OsStr::new("wgsl")))
            .map(|e| {
                let name = format!("bevy__{}", test_name(e.path()));
                libtest_mimic::Trial::test(name.clone(), move || bevy_case(name.clone(), e.path()))
            })
    });

    tests.extend({
        let in_entries = std::fs::read_dir("wgpu/in")
            .expect("missing dir `wgpu/in`")
            .map(|f| (f, "in"));
        let out_entries = std::fs::read_dir("wgpu/out")
            .expect("missing dir `wgpu/out`")
            .map(|f| (f, "out"));
        in_entries
            .chain(out_entries)
            .filter_map(|(e, d)| e.ok().map(|e| (e, d)))
            .filter(|(e, _)| e.path().extension() == Some(OsStr::new("wgsl")))
            .map(|(e, d)| {
                let filename = e.file_name();
                let name = format!("wgpu__{d}__{}", test_name(&filename));
                libtest_mimic::Trial::test(name.clone(), move || {
                    validation_case(name.clone(), e.path())
                })
                .with_ignored_flag(
                    [
                        "lexical-scopes.wgsl",     // https://github.com/gfx-rs/wgpu/issues/8235
                        "msl-vpt-formats-x1.wgsl", // https://github.com/gfx-rs/wgpu/issues/8225
                        "msl-vpt-formats-x2.wgsl", // https://github.com/gfx-rs/wgpu/issues/8225
                        "msl-vpt-formats-x3.wgsl", // https://github.com/gfx-rs/wgpu/issues/8225
                        "msl-vpt-formats-x4.wgsl", // https://github.com/gfx-rs/wgpu/issues/8225
                    ]
                    .iter()
                    .any(|f| filename.to_str() == Some(f)),
                )
            })
    });

    let args = libtest_mimic::Arguments::from_args();
    libtest_mimic::run(&args, tests).exit();
}

fn json_case(case: &Test) -> Result<(), libtest_mimic::Failed> {
    match &case.kind {
        TestKind::Syntax { syntax } => {
            let res = match syntax {
                SyntaxKind::Declaration => case.code.parse::<TranslationUnit>().map(|_| ()),
                SyntaxKind::Statement => case.code.parse::<Statement>().map(|_| ()),
                SyntaxKind::Expression => case.code.parse::<Expression>().map(|_| ()),
            };
            match (res, case.expect) {
                (Err(_), Expectation::Fail) | (Ok(()), Expectation::Pass) => Ok(()),
                (Ok(()), Expectation::Fail) => Err("expected Fail, got Pass".into()),
                (Err(e), Expectation::Pass) => {
                    Err(format!("expected Pass, got Fail (`{e}`)").into())
                }
            }
        }
        TestKind::Eval { eval, result } => {
            let module = case.code.parse::<TranslationUnit>()?;
            let expr = eval.parse::<Expression>()?;
            let (eval_inst, _) = wesl::eval(&expr, &module);
            let expect = result
                .as_ref()
                .map(|expect| -> Result<_, wesl::Error> {
                    let expr = expect.parse::<Expression>()?;
                    let (expect_inst, _) = wesl::eval(&expr, &module);
                    Ok(expect_inst?)
                })
                .transpose()?;
            match (eval_inst, expect) {
                (Err(_), None) => Ok(()),
                (Ok(inst), Some(expect)) => {
                    if inst != expect {
                        Err(format!("expected `{expect}`, got `{inst}`").into())
                    } else {
                        Ok(())
                    }
                }
                (Ok(inst), None) => Err(format!("expected Fail, got Pass (`{inst}`)").into()),
                (Err(err), Some(expect)) => {
                    Err(format!("expected `{expect}`, got Fail (`{err}`)").into())
                }
            }
        }
        TestKind::Context { lower } => {
            let mut module = case.code.parse::<TranslationUnit>()?;
            wesl::pass::retarget_idents(&mut module);
            let mut valid = wesl::pass::validate_wesl(&module);
            if *lower && valid.is_ok() {
                valid = wesl::pass::lower(&mut module).map_err(Diagnostic::from);
            }
            match (valid, case.expect) {
                (Err(_), Expectation::Fail) | (Ok(()), Expectation::Pass) => Ok(()),
                (Ok(()), Expectation::Fail) => Err("expected Fail, got Pass".into()),
                (Err(e), Expectation::Pass) => {
                    Err(format!("expected Pass, got Fail (`{e}`)").into())
                }
            }
        }
        TestKind::Modules { modules, result } => {
            let mut resolver = VirtualResolver::new();

            for (path, file) in modules {
                let path = ModulePath::from_str(path)?;
                resolver.add_module(path, file.into());
            }

            let main_module = ModulePath::new_root();
            resolver.add_module(main_module.clone(), case.code.clone().into());

            let compile_options = CompileOptions {
                keep_main: true,
                ..Default::default()
            };

            let res =
                Compiler::new_with_resolver(compile_options, resolver).compile_module(&main_module);

            let expect = result
                .as_ref()
                .map(|expect| expect.parse::<TranslationUnit>())
                .transpose()?;

            match (res, expect) {
                (Err(_), None) => Ok(()),
                (Ok(mut res), Some(mut expect)) => {
                    res.syntax.sort_declarations();
                    expect.sort_declarations();
                    if res.to_string() != expect.to_string() {
                        Err(format!("expected `{expect}`, got `{res}`").into())
                    } else {
                        Ok(())
                    }
                }
                (Ok(res), None) => Err(format!("expected Fail, got Pass (`{res}`)").into()),
                (Err(err), Some(expect)) => {
                    Err(format!("expected `{expect}`, got Fail (`{err}`)").into())
                }
            }
        }
    }
}

fn testsuite_syntax_case(case: &ParsingTest) -> Result<(), libtest_mimic::Failed> {
    let parse = wgsl_parse::parse_str(&case.src);
    match parse {
        Ok(s) if case.fails => Err(format!("expected Fail, got Pass (`{s}`)").into()),
        Ok(s) => {
            let str1 = s.to_string();
            let str2 = wgsl_parse::parse_str(&str1)
                .map_err(|e| {
                    format!("failed to parse after stringification\nerror: `{e}`\nsource: `{str1}`")
                })?
                .to_string();
            if str1 == str2 {
                Ok(())
            } else {
                Err(format!("stringification is lossy\nbefore: `{str1}`\nafter: `{str2}`").into())
            }
        }
        Err(e) if !case.fails => Err(format!("expected Pass, got Fail (`{e}`)").into()),
        Err(_) => Ok(()),
    }
}
pub fn testsuite_case(case: &WgslTestSrc) -> Result<(), libtest_mimic::Failed> {
    let mut resolver = VirtualResolver::new();

    for (path, file) in &case.wesl_src {
        let path = ModulePath::new_root().join_path(&ModulePath::from_path(path));
        resolver.add_module(path, file.into());
    }

    let main_module = ModulePath::from_str("package::main")?;
    let compile_options = CompileOptions {
        keep_main: true,
        ..Default::default()
    };

    let mut case_wgsl =
        Compiler::new_with_resolver(compile_options, resolver).compile_module(&main_module)?;

    if let Some(expect_wgsl) = &case.underscore_wgsl {
        let mut expect_wgsl = wgsl_parse::parse_str(expect_wgsl)?;
        case_wgsl.syntax.sort_declarations();
        expect_wgsl.sort_declarations();
        assert_eq!(case_wgsl.to_string(), expect_wgsl.to_string());
    }

    Ok(())
}

pub fn validation_case(test_name: String, path: PathBuf) -> Result<(), libtest_mimic::Failed> {
    let input = std::fs::read_to_string(path).expect("failed to read test file");
    let mut resolver = VirtualResolver::new();
    let main_path = ModulePath::from_str("package::main")?;
    resolver.add_module(main_path.clone(), input.into());
    let compile_options = CompileOptions {
        strip: false,
        lower: true,
        validate: true,
        mangler: ManglerKind::None,
        ..Default::default()
    };

    let mut compiler = Compiler::new_with_resolver(compile_options, resolver);

    // first we compile with strip: false to catch more bugs.
    let _ = compiler.compile_module(&main_path)?;

    // second, we run with strip: true, which is the default for WESL, and save the snapshot.
    compiler.options.strip = true;
    let mut res = compiler.compile_module(&main_path)?;
    res.syntax.sort_declarations();
    let expected = expect_file![format!(
        "./snapshots/testsuite__{}.snap",
        test_name.replace("/", "__")
    )];
    let actual = res.syntax.to_string();
    expected.assert_eq(&actual);
    Ok(())
}

pub fn bevy_case(test_name: String, path: PathBuf) -> Result<(), libtest_mimic::Failed> {
    let pkg_root_dir = path.parent().ok_or("file not found")?;
    let name = path
        .file_stem()
        .ok_or("file not found")?
        .to_string_lossy()
        .to_string();

    let mut constants = Constants::new();
    constants.set("MAX_CASCADES_PER_LIGHT", 10u32);
    constants.set("MAX_DIRECTIONAL_LIGHTS", 10);
    constants.set("PER_OBJECT_BUFFER_BATCH_SIZE", 10);
    constants.set("TONEMAPPING_LUT_TEXTURE_BINDING_INDEX", 10);
    constants.set("TONEMAPPING_LUT_SAMPLER_BINDING_INDEX", 10);

    let mut features = Features::new();
    features.set("MULTISAMPLED", true); // show_prepass needs it
    features.set("DEPTH_PREPASS", true); // show_prepass needs it
    features.set("NORMAL_PREPASS", true); // show_prepass needs it
    features.set("IRRADIANCE_VOLUMES_ARE_USABLE", true); // irradiance_volume_voxel_visualization needs it
    features.set("IRRADIANCE_VOLUMES_ARE_USABLE", true); // irradiance_volume_voxel_visualization needs it
    features.set("MOTION_VECTOR_PREPASS", true); // show_prepass needs it
    features.set("CLUSTERED_DECALS_ARE_USABLE", true); // custom_clustered_decal needs it
    features.set("VERTEX_UVS_A", true); // texture_binding_array needs it
    features.set("VERTEX_OUTPUT_INSTANCE_INDEX", true); // extended_material needs it

    if name == "water_material" {
        features.set("PREPASS_FRAGMENT", true); // water_material needs it
        features.set("PREPASS_PIPELINE", true); // water_material needs it
        features.set("NORMAL_PREPASS_OR_DEFERRED_PREPASS", true); // water_material needs it
    }

    let compile_options = CompileOptions {
        strip: false,
        lower: true,
        validate: true,
        constants,
        features,
        dependencies: vec![&bevy_wgsl::PACKAGE],
        ..Default::default()
    };

    let mut compiler = Compiler::new(compile_options);
    let main_path = ModulePath::new(PathOrigin::Absolute, vec![name]);

    // first we compile with strip: false to catch more bugs.
    let _ = compiler.compile_module(pkg_root_dir, &main_path)?;

    // second, we run with strip: true, which is the default for WESL, and save the snapshot.
    compiler.options.strip = true;
    let mut res = compiler.compile_module(pkg_root_dir, &main_path)?;
    res.syntax.sort_declarations();
    let expected = expect_file![format!("./snapshots/testsuite__{}.snap", test_name)];
    let actual = res.syntax.to_string();
    expected.assert_eq(&actual);
    Ok(())
}
