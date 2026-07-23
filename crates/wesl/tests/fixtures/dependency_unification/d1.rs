pub static PACKAGE: CodegenPkg = CodegenPkg {
    crate_name: "d",
    root: &MODULE,
    dependencies: &[],
};
pub static MODULE: CodegenModule = CodegenModule {
    name: "d",
    source: "const VERSION = 0x010;\n",
    submodules: &[],
};
