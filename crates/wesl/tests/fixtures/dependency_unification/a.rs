pub static PACKAGE: StaticPackage = StaticPackage {
    crate_name: "a",
    root: &MODULE,
    dependencies: &[&super::r#c2::PACKAGE, &super::r#d1::PACKAGE],
};
pub static MODULE: StaticPackageModule = StaticPackageModule {
    name: "a",
    source: "public import c::{ VERSION as C_VERSION };\npublic import d::{ VERSION as D_VERSION };\npublic const VERSION = 0x010;\n",
    submodules: &[],
};
