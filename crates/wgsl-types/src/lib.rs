#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod display;
mod error;
mod mem;

pub mod builtin;
pub mod conv;
pub mod idents;
pub mod inst;
pub mod syntax;
pub mod tplt;
pub mod ty;

pub use error::Error;
pub use inst::Instance;
pub use ty::Type;

pub use half::f16;

use tplt::TpltParam;

/// Function call signature.
#[derive(Clone, Debug, PartialEq)]
pub struct CallSignature {
    pub name: String,
    pub tplt: Option<Vec<TpltParam>>,
    pub args: Vec<Type>,
}

/// Shader compilation stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderStage {
    /// Shader module creation
    Const,
    /// Pipeline creation
    Override,
    /// Shader execution
    Exec,
}
