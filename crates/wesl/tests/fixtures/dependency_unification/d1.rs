pub static PACKAGE: StaticPackage = StaticPackage {
    crate_name: "d",
    root: &MODULE,
    dependencies: &[],
};
pub static MODULE: StaticPackageModule = StaticPackageModule {
    name: "d",
    source: "const VERSION = 0x010;\n",
    submodules: &[],
};
