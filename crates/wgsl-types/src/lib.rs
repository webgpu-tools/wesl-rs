#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod display;
mod error;
mod mem;

pub mod arena;
pub mod builtin;
pub mod conv;
pub mod idents;
pub mod inst;
pub mod syntax;
pub mod tplt;
pub mod ty;
pub mod ty_context;

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

/// Insertion-order-preserving hash set (`IndexSet<K>`), but with the same
/// hasher as `FastHashSet<K>` (faster but not resilient to DoS attacks).
pub type FastIndexSet<K> =
    indexmap::IndexSet<K, core::hash::BuildHasherDefault<rustc_hash::FxHasher>>;

/// Insertion-order-preserving hash map (`IndexMap<K, V>`), but with the same
/// hasher as `FastHashMap<K, V>` (faster but not resilient to DoS attacks).
pub type FastIndexMap<K, V> =
    indexmap::IndexMap<K, V, core::hash::BuildHasherDefault<rustc_hash::FxHasher>>;
