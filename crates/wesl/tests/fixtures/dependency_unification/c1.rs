#[allow(unused)]
pub static PACKAGE: CodegenPkg = CodegenPkg {
    crate_name: "c",
    root: &MODULE,
    dependencies: &[],
};
#[allow(unused)]
pub static MODULE: CodegenModule = CodegenModule {
    name: "c",
    source: "const VERSION = 0x011;\n",
    submodules: &[],
};
