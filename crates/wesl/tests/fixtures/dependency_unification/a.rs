pub static PACKAGE: CodegenPkg = CodegenPkg {
    crate_name: "a",
    root: &MODULE,
    dependencies: &[&super::r#c2::PACKAGE, &super::r#d1::PACKAGE],
};
pub static MODULE: CodegenModule = CodegenModule {
    name: "a",
    source: "@publish import c::{ VERSION as C_VERSION };\n@publish import d::{ VERSION as D_VERSION };\nconst VERSION = 0x010;\n",
    submodules: &[],
};
