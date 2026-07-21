//! Compilation passes.

mod compile;
mod condcomp;
mod driver;
mod link;
mod lower;
mod mangle;
mod retarget_idents;
mod usage_analysis;
mod validate;

pub use compile::{compile, compile_async, load_module, load_module_async, root_entry_points};
pub use condcomp::{Feature, Features, condcomp};
pub use driver::{AsyncCompilerDriver, CompileResult, CompilerDriver};
pub use link::link;
pub use lower::lower;
pub use mangle::mangle;
pub use retarget_idents::retarget_idents;
pub use usage_analysis::{Imports, Module, UsedItems, flatten_imports, usage_analysis};
pub use validate::{validate_wesl, validate_wgsl};
