//! Bumps the version of the workspace.
//!
//! * Updates `[workspace.package] version` and `package.json` in `wesl-rs-web`.
//! * Updates the lockfiles.
//! * Commits the changes and creates a version tag.
//!
//! It's part of the release process. The next step is `git push`, and the CI
//! will create the release to GitHub, crates.io and npm (for `wesl-rs-web`).

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, ExitCode},
};

const PACKAGE_JSON: &str = "crates/wesl-rs-web/package.json";

pub(crate) fn run(workspace_root: &Path, args: &[String]) -> ExitCode {
    let version = match args {
        [version] if !version.starts_with('-') => version,
        _ => {
            eprintln!("usage: cargo xtask version <version>");
            return ExitCode::FAILURE;
        }
    };

    match bump(workspace_root, version) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask version failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn bump(workspace_root: &Path, version: &str) -> Result<(), String> {
    check_semver(version)?;
    let tag = format!("v{version}");
    check_git_clean(workspace_root)?;
    check_tag_free(workspace_root, &tag)?;

    // Cargo.toml
    let cargo_path = workspace_root.join("Cargo.toml");
    bump_cargo_toml(&cargo_path, version)?;

    // package.json
    let package_path = workspace_root.join(PACKAGE_JSON);
    bump_package_json(&package_path, version)?;

    // Cargo.lock
    exec(
        Command::new(env!("CARGO"))
            .args(["update", "--workspace", "--offline"])
            .current_dir(workspace_root),
    )?;

    // commit + tag
    git(workspace_root, &["add", "."])?;
    git(
        workspace_root,
        &["commit", "--quiet", "--message", &format!("release {tag}")],
    )?;
    git(
        workspace_root,
        &["tag", "--annotate", "--message", &tag, &tag],
    )?;

    println!("---");
    println!("xtask version: bumped to version {version}, committed and tagged {tag}.");
    println!("review the commit, then run `git push --follow-tags`.");
    println!("this will trigger CI to publish the release on GitHub, crates.io and npm.");
    println!("---");
    Ok(())
}

/// Fails if the working tree has uncommitted changes.
fn check_git_clean(workspace_root: &Path) -> Result<(), String> {
    let status = git_output(workspace_root, &["status", "--porcelain"])?;
    if !status.trim().is_empty() {
        return Err("the git working tree is not clean, commit or stash your changes first".into());
    }
    Ok(())
}

/// Fails if `tag` already exists.
fn check_tag_free(workspace_root: &Path, tag: &str) -> Result<(), String> {
    let tags = git_output(workspace_root, &["tag", "--list", tag])?;
    if !tags.trim().is_empty() {
        return Err(format!("git tag `{tag}` already exists"));
    }
    Ok(())
}

fn git(workspace_root: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .status()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !status.success() {
        return Err(format!("`git {}` failed (status {status})", args.join(" ")));
    }
    Ok(())
}

fn git_output(workspace_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`git {}` failed (status {})",
            args.join(" "),
            output.status
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("git output is not UTF-8: {e}"))
}

/// Checks that `version` looks like `MAJOR.MINOR.PATCH[-pre][+build]`.
fn check_semver(version: &str) -> Result<(), String> {
    let core = version
        .split_once(['-', '+'])
        .map_or(version, |(core, _)| core);
    let parts: Vec<&str> = core.split('.').collect();
    let valid = parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if !valid {
        return Err(format!("`{version}` is not a valid semver version"));
    }
    Ok(())
}

/// Replaces the `[workspace.package] version`, with `cargo set-version` from `cargo-edit`.
fn bump_cargo_toml(cargo_path: &Path, version: &str) -> Result<(), String> {
    check_cargo_edit()?;
    let manifest = cargo_path.to_string_lossy();
    exec(Command::new(env!("CARGO")).args([
        "set-version",
        "--manifest-path",
        &manifest,
        "--package",
        "wesl",
        version,
    ]))
}

/// Replaces the `"version"` of a package.json, with `npm version`.
fn bump_package_json(package_path: &Path, version: &str) -> Result<(), String> {
    let package_dir = package_path
        .parent()
        .ok_or("package.json has no parent directory")?;
    exec(
        Command::new("npm")
            .args(["version", "--no-git-tag-version", version])
            .current_dir(package_dir),
    )
}

/// Fails with an install hint if `cargo set-version` is not available.
fn check_cargo_edit() -> Result<(), String> {
    let found = Command::new(env!("CARGO"))
        .args(["set-version", "--version"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !found {
        return Err(
            "`cargo set-version` not found, install it with `cargo install cargo-edit`".to_string(),
        );
    }
    Ok(())
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
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run `{display}`: {e}"))?;
    if !status.success() {
        return Err(format!("`{display}` failed (status {status})"));
    }
    Ok(())
}
