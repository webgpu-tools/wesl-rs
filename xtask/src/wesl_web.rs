//! Builds the `wesl-web` crate as an npm package.
//!
//! Runs `wasm-pack build crates/wesl-web --target web --out-dir crates/wesl-web/dist`.

use std::{
    ffi::OsStr,
    fs,
    path::Path,
    process::{Command, ExitCode},
};

/// Relative to the workspace root.
const PACKAGE_ROOT: &str = "crates/wesl-web";

pub(crate) fn run(workspace_root: &Path, args: &[String]) -> ExitCode {
    let mut release = false;
    for arg in args {
        match arg.as_str() {
            "--release" => release = true,
            other => {
                eprintln!("xtask wesl_web failed: unknown argument `{other}`");
                return ExitCode::FAILURE;
            }
        }
    }

    match dist_web(workspace_root, release) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask wesl_web failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dist_web(workspace_root: &Path, release: bool) -> Result<(), String> {
    let package_root = workspace_root.join(PACKAGE_ROOT);
    check_wasm_pack()?;
    check_version(workspace_root, &package_root)?;

    // build the wasm + JS glue + .d.ts. `--no-pack` because we ship our own package.json.
    let out_dir = package_root.join("dist");
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)
            .map_err(|e| format!("failed to clean {}: {e}", out_dir.display()))?;
    }
    let mut cmd = Command::new("wasm-pack");
    cmd.current_dir(&package_root).args([
        "build",
        if release {
            "--release"
        } else {
            "--dev"
        },
        "--target",
        "web",
        "--out-dir",
        "dist",
        "--out-name",
        "wesl_web",
        "--no-pack",
    ]);
    if !release {
        cmd.args(["--", "--features", "debug"]);
    }
    exec(&mut cmd)?;

    Ok(())
}

fn check_wasm_pack() -> Result<(), String> {
    let found = Command::new("wasm-pack")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !found {
        return Err(format!("`wasm-pack` command not found"));
    }
    Ok(())
}

/// The npm package version must match the workspace crate version.
fn check_version(workspace_root: &Path, package_root: &Path) -> Result<(), String> {
    let cargo = read(&workspace_root.join("Cargo.toml"))?;
    let crate_version = cargo
        .split("[workspace.package]")
        .nth(1)
        .and_then(|s| find_string_value(s, "version ="))
        .ok_or("could not find the workspace version in Cargo.toml")?;
    let package = read(&package_root.join("package.json"))?;
    let npm_version = find_string_value(&package, "\"version\":")
        .ok_or("could not find the version in package.json")?;
    if crate_version != npm_version {
        return Err(format!(
            "package.json version ({npm_version}) does not match the workspace version ({crate_version})"
        ));
    }
    Ok(())
}

/// Returns the first quoted string following `key` in `text`.
fn find_string_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = &text[text.find(key)? + key.len()..];
    let start = rest.find('"')? + 1;
    let len = rest[start..].find('"')?;
    Some(&rest[start..start + len])
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))
}

fn exec(cmd: &mut Command) -> Result<(), String> {
    let display = format!(
        "{} {}",
        cmd.get_program().to_string_lossy(),
        cmd.get_args()
            .map(OsStr::to_string_lossy)
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("xtask wesl_web: running `{display}`");
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run `{display}`: {e}"))?;
    if !status.success() {
        return Err(format!(
            "xtask wesl_web failed: {display} (status {status})"
        ));
    }
    Ok(())
}
