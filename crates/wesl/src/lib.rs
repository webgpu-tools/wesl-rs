#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod frontend;
#[cfg(feature = "eval")]
mod frontend_eval;
mod idents;
mod util;

pub(crate) use util::*;

pub mod error;
#[cfg(feature = "eval")]
pub mod eval;
pub mod mangler;
pub mod package;
pub mod pass;
pub mod resolver;
pub mod sourcemap;
pub mod toml_cfg;

pub use crate::{
    error::Error,
    frontend::*,
    mangler::Mangler,
    package::PackageBuilder,
    pass::{Feature, Features},
    resolver::{AsyncResolver, Constants, Resolver},
};

#[cfg(feature = "eval")]
pub use crate::frontend_eval::*;

// re-exports
pub use wgsl_parse::syntax;

use wgsl_parse::syntax::ModulePath;
