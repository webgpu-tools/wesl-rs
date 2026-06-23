//! Type conversion algorithms.
//!
//! Implementation of the [conversion_rank] algorithm and utilities to convert
//! [`Type`]s and [`Instance`]s using [automatic conversions].
//!
//! [automatic conversions]: https://www.w3.org/TR/WGSL/#feasible-automatic-conversion

use half::f16;
use itertools::Itertools;
use num_traits::{FromPrimitive, ToPrimitive};

use crate::{
    Error, Instance,
    inst::{ArrayInstance, LiteralInstance, MatInstance, StructInstance, VecInstance},
    syntax::AccessMode,
    ty::{Ty, Type},
};

#[cfg(feature = "complex")]
use crate::inst::{ComplexInstance, QuatInstance};

pub trait Convert: Sized + Clone + Ty {
    /// Convert an instance to another type, if a feasible conversion exists.
    ///
    /// E.g. `array<u32>.convert_inner_to(array<f32>)` becomes `array<f32>`.
    ///
    /// Reference: <https://www.w3.org/TR/WGSL/#conversion-rank>
    fn convert_to(&self, ty: &Type) -> Option<Self>;

    /// Convert an instance by changing its inner type to another.
    ///
    /// E.g. `array<u32>.convert_inner_to(f32)` becomes `array<f32>`.
    ///
    /// Identical to [`Convert::convert_to`] if the type has no inner type.
    /// Does not check that the resulting type exists, e.g. `i32` is not a valid inner type for
    /// matrix types.
    ///
    ///
    /// See [`Ty::inner_ty`]
    /// See [`Convert::convert_to`]
    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        self.convert_to(ty)
    }

    /// Convert an abstract instance to a concrete type.
    ///
    /// E.g. `array<vec<AbstractInt>>` becomes `array<vec<i32>>`.
    fn concretize(&self) -> Option<Self> {
        self.convert_to(&self.ty().concretize())
    }
}

impl Convert for Type {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        self.is_convertible_to(ty).then_some(ty.clone())
    }
    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        match self {
            Type::Array(inner, n) => inner
                .convert_to(ty)
                .map(|inner| Type::Array(inner.into(), *n)),
            Type::Vec(n, inner) => inner
                .convert_to(ty)
                .map(|inner| Type::Vec(*n, inner.into())),
            #[cfg(feature = "complex")]
            Type::Complex(inner) => inner
                .convert_to(ty)
                .map(|inner| Type::Complex(inner.into())),
            #[cfg(feature = "complex")]
            Type::Quat(inner) => inner.convert_to(ty).map(|inner| Type::Quat(inner.into())),
            Type::Mat(c, r, inner) => inner
                .convert_to(ty)
                .map(|inner| Type::Mat(*c, *r, inner.into())),
            Type::Atomic(_) => (self == ty).then_some(ty.clone()),
            Type::Ptr(_, _, _) => (self == ty).then_some(ty.clone()),
            _ => self.convert_to(ty), // for types that don't have an inner ty
        }
    }
    fn concretize(&self) -> Option<Self> {
        Some(self.concretize())
    }
}

impl Type {
    pub fn is_convertible_to(&self, ty: &Type) -> bool {
        conversion_rank(self, ty).is_some()
    }
    pub fn concretize(&self) -> Self {
        match self {
            Self::AbstractInt => Type::I32,
            Self::AbstractFloat => Type::F32,
            Self::Array(ty, n) => Type::Array(ty.concretize().into(), *n),
            Self::Vec(n, ty) => Type::Vec(*n, ty.concretize().into()),
            #[cfg(feature = "complex")]
            Self::Complex(ty) => Type::Complex(ty.concretize().into()),
            #[cfg(feature = "complex")]
            Self::Quat(ty) => Type::Quat(ty.concretize().into()),
            Self::Mat(c, r, ty) if ty.is_abstract() => Type::Mat(*c, *r, Type::F32.into()),
            _ => self.clone(),
        }
    }

    /// Apply the load rule.
    ///
    /// Reference: <https://www.w3.org/TR/WGSL/#load-rule>
    pub fn loaded(self) -> Self {
        if let Type::Ref(_, ty, _) = self {
            ty.loaded()
        } else {
            self
        }
    }
}

impl Instance {
    pub fn is_convertible_to(&self, ty: &Type) -> bool {
        self.ty().is_convertible_to(ty)
    }

    /// Apply the load rule.
    ///
    /// Reference: <https://www.w3.org/TR/WGSL/#load-rule>
    pub fn loaded(mut self) -> Result<Self, Error> {
        while let Instance::Ref(r) = self {
            self = r.read()?.to_owned();
        }
        Ok(self)
    }
}

impl LiteralInstance {
    pub fn is_infinite(&self) -> bool {
        match self {
            LiteralInstance::Bool(_) => false,
            LiteralInstance::AbstractInt(_) => false,
            LiteralInstance::AbstractFloat(n) => n.is_infinite(),
            LiteralInstance::I32(_) => false,
            LiteralInstance::U32(_) => false,
            LiteralInstance::F32(n) => n.is_infinite(),
            LiteralInstance::F16(n) => n.is_infinite(),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => false,
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => false,
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => n.is_infinite(),
        }
    }
    pub fn is_finite(&self) -> bool {
        !self.is_infinite()
    }
}

impl Convert for LiteralInstance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        if ty == &self.ty() {
            return Some(*self);
        }

        // TODO: check that these conversions are correctly implemented.
        // I think they are incorrect. the to_xyz() functions do not perform rounding.
        // reference: <https://www.w3.org/TR/WGSL/#floating-point-conversion>
        // ... except that hex literals must be *exactly* representable in the target type.
        match (self, ty) {
            (Self::AbstractInt(n), Type::AbstractFloat) => n.to_f64().map(Self::AbstractFloat),
            (Self::AbstractInt(n), Type::I32) => n.to_i32().map(Self::I32),
            (Self::AbstractInt(n), Type::U32) => n.to_u32().map(Self::U32),
            (Self::AbstractInt(n), Type::F32) => n.to_f32().map(Self::F32),
            (Self::AbstractInt(n), Type::F16) => f16::from_i64(*n).map(Self::F16),
            (Self::AbstractFloat(n), Type::F32) => n.to_f32().map(Self::F32),
            (Self::AbstractFloat(n), Type::F16) => Some(Self::F16(f16::from_f64(*n))),
            _ => None,
        }
        .and_then(|n| n.is_finite().then_some(n))
    }
}

impl Convert for ArrayInstance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        if let Type::Array(c_ty, Some(n)) = ty {
            if *n == self.n() {
                self.convert_inner_to(c_ty)
            } else {
                None
            }
        } else if let Type::Array(c_ty, None) = ty {
            self.convert_inner_to(c_ty)
        } else {
            None
        }
    }
    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        let components = self
            .iter()
            .map(|c| c.convert_to(ty))
            .collect::<Option<Vec<_>>>()?;
        Some(ArrayInstance::new(components, self.runtime_sized))
    }
}

impl Convert for VecInstance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        // TODO: Add conversion to complex/quat here?
        if let Type::Vec(n, c_ty) = ty {
            if *n as usize == self.n() {
                self.convert_inner_to(c_ty)
            } else {
                None
            }
        } else {
            None
        }
    }
    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        let components = self
            .iter()
            .map(|c| c.convert_to(ty))
            .collect::<Option<Vec<_>>>()?;
        Some(VecInstance::new(components))
    }
}

#[cfg(feature = "complex")]
impl Convert for ComplexInstance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        if let Type::Complex(c_ty) = ty {
            self.convert_inner_to(c_ty)
        } else {
            None
        }
    }
    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        let components = self
            .iter()
            .map(|c| c.convert_to(ty))
            .collect::<Option<Vec<_>>>()?;
        Some(ComplexInstance::new(components))
    }
}

#[cfg(feature = "complex")]
impl Convert for QuatInstance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        if let Type::Quat(c_ty) = ty {
            self.convert_inner_to(c_ty)
        } else {
            None
        }
    }
    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        let components = self
            .iter()
            .map(|c| c.convert_to(ty))
            .collect::<Option<Vec<_>>>()?;
        Some(QuatInstance::new(components))
    }
}

impl Convert for MatInstance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        if let Type::Mat(c, r, c_ty) = ty {
            if *c as usize == self.c() && *r as usize == self.r() {
                self.convert_inner_to(c_ty)
            } else {
                None
            }
        } else {
            None
        }
    }
    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        let components = self
            .iter_cols()
            .map(|c| c.convert_inner_to(ty))
            .collect::<Option<Vec<_>>>()?;
        Some(MatInstance::from_cols(components))
    }
}

impl Convert for StructInstance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        if &self.ty() == ty {
            Some(self.clone())
        } else if let Type::Struct(s2) = ty {
            let s1 = &self.ty;
            if s1.name.starts_with("__") && s2.name.starts_with("__") {
                // this is a struct type conversion of built-in types.
                // __frexp_result_* or __modf_result_*
                // TODO: here we just assume that s2 is a variant of s1. We should
                // check.
                if s2.name.ends_with("f32") {
                    let members = self
                        .members
                        .iter()
                        .map(|inst| inst.convert_inner_to(&Type::F32))
                        .collect::<Option<Vec<_>>>()?;
                    Some(StructInstance::new((**s2).clone(), members))
                } else if s2.name.ends_with("f16") {
                    let members = self
                        .members
                        .iter()
                        .map(|inst| inst.convert_inner_to(&Type::F16))
                        .collect::<Option<Vec<_>>>()?;
                    Some(StructInstance::new((**s2).clone(), members))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Convert for Instance {
    fn convert_to(&self, ty: &Type) -> Option<Self> {
        if &self.ty() == ty {
            return Some(self.clone());
        }
        match self {
            Self::Literal(l) => l.convert_to(ty).map(Self::Literal),
            Self::Struct(s) => s.convert_to(ty).map(Self::Struct),
            Self::Array(a) => a.convert_to(ty).map(Self::Array),
            Self::Vec(v) => v.convert_to(ty).map(Self::Vec),
            #[cfg(feature = "complex")]
            Self::Complex(c) => c.convert_to(ty).map(Self::Complex),
            #[cfg(feature = "complex")]
            Self::Quat(q) => q.convert_to(ty).map(Self::Quat),
            Self::Mat(m) => m.convert_to(ty).map(Self::Mat),
            Self::Ptr(_) => None,
            Self::Ref(r) => r.read().ok().and_then(|r| r.convert_to(ty)), // this is the "load rule". Also performed by `eval_value`.
            Self::Atomic(_) => None,
            Self::Deferred(_) => None,
        }
    }

    fn convert_inner_to(&self, ty: &Type) -> Option<Self> {
        match self {
            Self::Literal(l) => l.convert_inner_to(ty).map(Self::Literal),
            Self::Struct(_) => None,
            Self::Array(a) => a.convert_inner_to(ty).map(Self::Array),
            Self::Vec(v) => v.convert_inner_to(ty).map(Self::Vec),
            #[cfg(feature = "complex")]
            Self::Complex(c) => c.convert_inner_to(ty).map(Self::Complex),
            #[cfg(feature = "complex")]
            Self::Quat(q) => q.convert_inner_to(ty).map(Self::Quat),
            Self::Mat(m) => m.convert_inner_to(ty).map(Self::Mat),
            Self::Ptr(_) => None,
            Self::Ref(r) => r.read().ok().and_then(|r| r.convert_inner_to(ty)), // this is the "load rule". Also performed by `eval_value`.
            Self::Atomic(_) => None,
            Self::Deferred(_) => None,
        }
    }
}

/// Implements the [conversion rank algorithm](https://www.w3.org/TR/WGSL/#conversion-rank)
pub fn conversion_rank(ty1: &Type, ty2: &Type) -> Option<u32> {
    // reference: <https://www.w3.org/TR/WGSL/#conversion-rank>
    match (ty1, ty2) {
        (_, _) if ty1 == ty2 => Some(0),
        (Type::Ref(_, ty1, AccessMode::Read | AccessMode::ReadWrite), ty2) if &**ty1 == ty2 => {
            Some(0)
        }
        (Type::AbstractInt, Type::AbstractFloat) => Some(5),
        (Type::AbstractInt, Type::I32) => Some(3),
        (Type::AbstractInt, Type::U32) => Some(4),
        (Type::AbstractInt, Type::F32) => Some(6),
        (Type::AbstractInt, Type::F16) => Some(7),
        (Type::AbstractFloat, Type::F32) => Some(1),
        (Type::AbstractFloat, Type::F16) => Some(2),
        // frexp and modf
        (Type::Struct(s1), Type::Struct(s2)) => {
            if s1.name.starts_with("__") && s1.name.ends_with("abstract") {
                if s2.name.ends_with("f32") {
                    Some(1)
                } else if s2.name.ends_with("f16") {
                    Some(2)
                } else {
                    None
                }
            } else {
                None
            }
        }
        (Type::Array(ty1, n1), Type::Array(ty2, n2)) if n1 == n2 => conversion_rank(ty1, ty2),
        (Type::Vec(n1, ty1), Type::Vec(n2, ty2)) if n1 == n2 => conversion_rank(ty1, ty2),
        #[cfg(feature = "complex")]
        (Type::Complex(ty1), Type::Complex(ty2)) => conversion_rank(ty1, ty2),
        #[cfg(feature = "complex")]
        (Type::Quat(ty1), Type::Quat(ty2)) => conversion_rank(ty1, ty2),
        (Type::Mat(c1, r1, ty1), Type::Mat(c2, r2, ty2)) if c1 == c2 && r1 == r2 => {
            conversion_rank(ty1, ty2)
        }
        _ => None,
    }
}

/// Performs overload resolution when two instances of T are involved (which is the most common).
/// it just makes sure that the two instance types are the same. This is sufficient in most cases.
pub fn convert<T: Convert + Ty + Clone>(i1: &T, i2: &T) -> Option<(T, T)> {
    let (ty1, ty2) = (i1.ty(), i2.ty());
    let ty = convert_ty(&ty1, &ty2)?;
    let i1 = i1.convert_to(ty)?;
    let i2 = i2.convert_to(ty)?;
    Some((i1, i2))
}

/// See [`convert`]
pub fn convert_inner<T1: Convert + Ty + Clone, T2: Convert + Ty + Clone>(
    i1: &T1,
    i2: &T2,
) -> Option<(T1, T2)> {
    let (ty1, ty2) = (i1.inner_ty(), i2.inner_ty());
    let ty = convert_ty(&ty1, &ty2)?;
    let i1 = i1.convert_inner_to(ty)?;
    let i2 = i2.convert_inner_to(ty)?;
    Some((i1, i2))
}

/// See [`convert`]
pub fn convert_all<'a, T: Convert + Ty + Clone + 'a>(insts: &[T]) -> Option<Vec<T>> {
    let tys = insts.iter().map(|i| i.ty()).collect_vec();
    let ty = convert_all_ty(&tys)?;
    convert_all_to(insts, ty)
}

/// See [`convert`]
pub fn convert_all_to<'a, T: Convert + Ty + Clone + 'a>(insts: &[T], ty: &Type) -> Option<Vec<T>> {
    insts
        .iter()
        .map(|inst| inst.convert_to(ty))
        .collect::<Option<Vec<_>>>()
}

/// See [`convert`]
pub fn convert_all_inner_to<'a, T: Convert + Ty + Clone + 'a>(
    insts: &[T],
    ty: &Type,
) -> Option<Vec<T>> {
    insts
        .iter()
        .map(|inst| inst.convert_inner_to(ty))
        .collect::<Option<Vec<_>>>()
}

/// Performs overload resolution when two instances of T are involved (which is the most common).
/// it just makes sure that the two types are the same. This is sufficient in most cases.
pub fn convert_ty<'a>(ty1: &'a Type, ty2: &'a Type) -> Option<&'a Type> {
    conversion_rank(ty1, ty2)
        .map(|_rank| ty2)
        .or_else(|| conversion_rank(ty2, ty1).map(|_rank| ty1))
}

/// Performs overload resolution (find the type that all others can be automatically converted to)
pub fn convert_all_ty<'a>(tys: impl IntoIterator<Item = &'a Type> + 'a) -> Option<&'a Type> {
    tys.into_iter()
        .map(Option::Some)
        .reduce(|ty1, ty2| convert_ty(ty1?, ty2?))
        .flatten()
}
