//! JSON Tests from the `wesl-testsuite`
//! See schemas: https://github.com/wgsl-tooling-wg/wesl-testsuite/blob/main/src/TestSchema.ts
//!
//! These tests are run with `harness = false` in `Cargo.toml`, because they rely on the
//! `libtest_mimic` custom harness to generate tests at runtime based on the JSON files.

use std::{ffi::OsStr, path::PathBuf, process::Command, str::FromStr};

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

fn main() {
    let mut tests: Vec<libtest_mimic::Trial> = Vec::new();

    let spec_tests = [
        "spec-tests/idents.json",
        "spec-tests/literals.json",
        "spec-tests/lit-type-inference.json",
        "spec-tests/imports.json",
        "spec-tests/circular.json",
        "spec-tests/types.json",
    ];
    for path in spec_tests {
        tests.extend({
            let file = std::fs::read_to_string(path).expect("failed to read test file");
            let json: Vec<Test> = serde_json::from_str(&file).expect("failed to parse json file");
            json.into_iter().map(|case| {
                let name = format!("{path}::{}", case.name);
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
            libtest_mimic::Trial::test(path, move || validation_case(PathBuf::from(path)))
        });
    }

    tests.extend({
        let file =
            std::fs::read_to_string("wesl-testsuite/src/test-cases-json/importSyntaxCases.json")
                .expect("failed to read test file");
        let json: Vec<ParsingTest> =
            serde_json::from_str(&file).expect("failed to parse json file");
        json.into_iter().map(|mut case| {
            case.normalize();
            let name = format!("importSyntaxCases.json::{}", case.src);
            libtest_mimic::Trial::test(name, move || {
                testsuite_syntax_case(&case).inspect_err(|_| eprint_parsing_test(&case))
            })
        })
    });

    let testsuite_tests = [
        "wesl-testsuite/src/test-cases-json/importCases.json",
        "wesl-testsuite/src/test-cases-json/conditionalTranslationCases.json",
        "spec-tests/dead-code.json",
    ];
    for path in testsuite_tests {
        tests.extend({
            let file = std::fs::read_to_string(path).expect("failed to read test file");
            let json: Vec<WgslTestSrc> =
                serde_json::from_str(&file).expect("failed to parse json file");
            json.into_iter().map(|case| {
                let name = format!("importCases.json::{}", case.name);
                libtest_mimic::Trial::test(name, move || {
                    testsuite_case(&case).inspect_err(|_| eprint_wgsl_test(&case))
                })
            })
        });
    }

    tests.extend({
        let file = std::fs::read_to_string("wesl-testsuite/src/test-cases-json/bulkTests.json")
            .expect("failed to read test file");
        let json: Vec<WgslBulkTest> =
            serde_json::from_str(&file).expect("failed to parse json file");
        json.into_iter().flat_map(|bulk_case| {
            let name = format!("bulkTests.json::{}", bulk_case.name);
            let cwd = std::path::Path::new("wesl-testsuite");
            fetch_bulk_test(&bulk_case, cwd)
                .unwrap_or_else(|_| panic!("failed to fetch bulk test {name}"));

            assert!(
                bulk_case.exclude.is_none_or(|v| v.is_empty()),
                "Globs are not supported"
            );
            let base_dir = cwd.join(&bulk_case.base_dir);
            let include_paths: Vec<_> = bulk_case
                .include
                .map(|v| v.iter().map(|v| base_dir.join(v)).collect())
                .unwrap_or_else(|| {
                    std::fs::read_dir(&bulk_case.base_dir)
                        .unwrap_or_else(|_| panic!("missing dir `{}`", &bulk_case.base_dir))
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension() == Some(OsStr::new("wgsl")))
                        .map(|v| v.path())
                        .collect()
                });

            include_paths.into_iter().map(move |shader_path| {
                libtest_mimic::Trial::test(format!("{name}::{shader_path:?}"), move || {
                    validation_case(shader_path)
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
                let name = format!("bevy::{}", e.file_name().display());
                libtest_mimic::Trial::test(name, move || bevy_case(e.path()))
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
                let name = format!("wgpu::{d}::{}", filename.display());
                libtest_mimic::Trial::test(name, move || validation_case(e.path()))
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

fn fetch_bulk_test(bulk_test: &WgslBulkTest, cwd: &std::path::Path) -> std::io::Result<()> {
    // Modeled after https://github.com/gfx-rs/wgpu/blob/c0a580d6f0343a725b3defa8be4fdf0a9691eaad/xtask/src/cts.rs
    let Some(WgslGitSrc { url, revision }) = &bulk_test.git else {
        return Ok(());
    };
    let base_dir = cwd.join(&bulk_test.base_dir);
    if std::fs::exists(&base_dir)? {
        // Do a git update
        let commit_exists = Command::new("git")
            .args(["cat-file", "commit", revision])
            .current_dir(&base_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to execute git cat-file")
            .wait()
            .expect("failed to wait on git")
            .success();

        if !commit_exists {
            let git_fetch = Command::new("git")
                .args(["fetch", "--quiet", "--depth", "1", "origin", revision])
                .current_dir(&base_dir)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .expect("failed to execute git fetch")
                .wait()
                .expect("failed to wait on git");
            if !git_fetch.success() {
                panic!("Git fetch failed");
            }
        }

        let git_checkout = Command::new("git")
            .args(["checkout", "--quiet", revision])
            .current_dir(&base_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("failed to execute git checkout")
            .wait()
            .expect("failed to wait on git");

        if !git_checkout.success() {
            panic!("Git checkout failed");
        }
    } else {
        // Note: The --revision flag is not supported by git versions below 2.49.0 (so we don't use it)
        let git_clone = Command::new("git")
            .args([
                "clone",
                url,
                "--no-checkout",
                "--depth",
                "1",
                &bulk_test.base_dir,
            ])
            .current_dir(cwd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("failed to execute git clone")
            .wait()
            .expect("failed to wait on git");

        if !git_clone.success() {
            panic!("Git clone failed");
        }

        let git_fetch = Command::new("git")
            .args(["fetch", "--quiet", "--depth", "1", "origin", revision])
            .current_dir(&base_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("failed to execute git fetch")
            .wait()
            .expect("failed to wait on git");
        if !git_fetch.success() {
            panic!("Git fetch failed");
        }

        let git_checkout = Command::new("git")
            .args(["checkout", "--quiet", revision])
            .current_dir(&base_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("failed to execute git checkout")
            .wait()
            .expect("failed to wait on git");

        if !git_checkout.success() {
            panic!("Git checkout {:?} failed", revision);
        }
    }

    Ok(())
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
        // TODO
        // lazy: !case.requires.iter().any(|r| r == "eager"),
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

pub fn validation_case(path: PathBuf) -> Result<(), libtest_mimic::Failed> {
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
    let _ = Compiler::new_with_resolver(compile_options, resolver).compile_module(&main_path)?;
    Ok(())
}

pub fn bevy_case(path: PathBuf) -> Result<(), libtest_mimic::Failed> {
    let base = path.parent().ok_or("file not found")?;
    let name = path
        .file_stem()
        .ok_or("file not found")?
        .to_string_lossy()
        .to_string();

    let mut constants = Constants::new();
    constants.add_constant("MAX_CASCADES_PER_LIGHT", 10u32);
    constants.add_constant("MAX_DIRECTIONAL_LIGHTS", 10);
    constants.add_constant("PER_OBJECT_BUFFER_BATCH_SIZE", 10);
    constants.add_constant("TONEMAPPING_LUT_TEXTURE_BINDING_INDEX", 10);
    constants.add_constant("TONEMAPPING_LUT_SAMPLER_BINDING_INDEX", 10);

    let mut features = Features::new();
    features.add_feature("MULTISAMPLED", true); // show_prepass needs it
    features.add_feature("DEPTH_PREPASS", true); // show_prepass needs it
    features.add_feature("NORMAL_PREPASS", true); // show_prepass needs it
    features.add_feature("IRRADIANCE_VOLUMES_ARE_USABLE", true); // irradiance_volume_voxel_visualization needs it
    features.add_feature("IRRADIANCE_VOLUMES_ARE_USABLE", true); // irradiance_volume_voxel_visualization needs it
    features.add_feature("MOTION_VECTOR_PREPASS", true); // show_prepass needs it
    features.add_feature("CLUSTERED_DECALS_ARE_USABLE", true); // custom_clustered_decal needs it
    features.add_feature("VERTEX_UVS_A", true); // texture_binding_array needs it
    features.add_feature("VERTEX_OUTPUT_INSTANCE_INDEX", true); // extended_material needs it

    if name == "water_material" {
        features.add_feature("PREPASS_FRAGMENT", true); // water_material needs it
        features.add_feature("PREPASS_PIPELINE", true); // water_material needs it
        features.add_feature("NORMAL_PREPASS_OR_DEFERRED_PREPASS", true); // water_material needs it
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

    Compiler::new(compile_options)
        .compile_module(base, &ModulePath::new(PathOrigin::Absolute, vec![name]))?;
    Ok(())
}
