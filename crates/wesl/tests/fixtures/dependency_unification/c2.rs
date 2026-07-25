pub static PACKAGE: StaticPackage = StaticPackage {
    crate_name: "c",
    root: &MODULE,
    dependencies: &[],
};
pub static MODULE: StaticPackageModule = StaticPackageModule {
    name: "c",
    source: "const VERSION = 0x011;\n",
    submodules: &[],
};
