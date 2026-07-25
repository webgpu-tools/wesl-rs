#[allow(unused)]
pub static PACKAGE: StaticPackage = StaticPackage {
    crate_name: "c",
    root: &MODULE,
    dependencies: &[],
};
#[allow(unused)]
pub static MODULE: StaticPackageModule = StaticPackageModule {
    name: "c",
    source: "const VERSION = 0x011;\n",
    submodules: &[],
};
