//! Compilation passes.

mod compile;
mod condcomp;
mod driver;
mod import;
mod link;
mod lower;
mod mangle;
mod retarget_idents;
mod usage_analysis;
mod validate;
mod visit;

pub use compile::{compile, compile_async, load_module, load_module_async, root_entry_points};
pub use condcomp::{Feature, Features, condcomp};
pub use driver::{AsyncCompilerDriver, CompileResult, CompilerDriver};
pub use import::{ImportedItem, Imports, flatten_imports, imported_item_path};
pub use link::link;
pub use lower::lower;
pub use mangle::mangle;
pub use retarget_idents::{retarget_idents, retarget_modules};
pub use usage_analysis::{Module, UsedItems, usage_analysis};
pub use validate::{validate_wesl, validate_wgsl};
pub use visit::Visit;
