pub static PACKAGE: CodegenPkg = CodegenPkg {
    crate_name: "c",
    root: &MODULE,
    dependencies: &[],
};
pub static MODULE: CodegenModule = CodegenModule {
    name: "c",
    source: "const VERSION = 0x011;\n",
    submodules: &[],
};
