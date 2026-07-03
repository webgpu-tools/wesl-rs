//! Built-in functions call implementations.
//!
//! Functions bear the same name as the WGSL counterpart.
//! Functions that take template parameters are suffixed with `_t` and take `tplt_*` arguments.
//!
//! ### Warning
//!
//! Some functions are still TODO and are documented as such.
//! Derivatives and texture functions are also missing.
//!
//! ### Usage quirks
//!
//! * The arguments must be [loaded][Type::loaded].
//! * User-defined functions can shadow WGSL built-in functions.
//! * Type aliases must be resolved: WGSL allows calling functions with the name of the alias.

#![allow(non_snake_case)]

use half::prelude::*;
use itertools::{Itertools, chain, izip};
use num_traits::{One, ToBytes, ToPrimitive, Zero, real::Real};

use crate::{
    Error, Instance, ShaderStage,
    conv::{Convert, convert, convert_all_ty},
    inst::{
        AtomicInstance, LiteralInstance, MatInstance, RefInstance, StructInstance, VecInstance,
    },
    ty::{Ty, Type},
};

use super::{Compwise, atomic_compare_exchange_struct_type, frexp_struct_type};

type E = Error;

// -------
// BITCAST
// -------
// reference: <https://www.w3.org/TR/WGSL/#bit-reinterp-builtin-functions>

/// `bitcast<T>()` builtin function.
///
/// Expects `tplt_ty` to be valid (a numeric scalar or vector).
///
/// Reference: <https://www.w3.org/TR/WGSL/#bitcast-builtin>
pub fn bitcast_t(tplt_ty: &Type, e: &Instance) -> Result<Instance, E> {
    fn lit_bytes(l: &LiteralInstance) -> Result<Vec<u8>, E> {
        match l {
            LiteralInstance::Bool(_) => Err(E::Builtin("bitcast argument cannot be bool")),
            LiteralInstance::AbstractInt(_) => Err(E::Unreachable), // because of `concretize` below
            LiteralInstance::AbstractFloat(_) => Err(E::Unreachable),
            LiteralInstance::I32(n) => Ok(n.to_le_bytes().to_vec()),
            LiteralInstance::U32(n) => Ok(n.to_le_bytes().to_vec()),
            LiteralInstance::F32(n) => Ok(n.to_le_bytes().to_vec()),
            LiteralInstance::F16(n) => Ok(n.to_le_bytes().to_vec()),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(n) => Ok(n.to_le_bytes().to_vec()),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(n) => Ok(n.to_le_bytes().to_vec()),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => Ok(n.to_le_bytes().to_vec()),
        }
    }

    fn vec_bytes(v: &VecInstance) -> Result<Vec<u8>, E> {
        v.iter()
            .map(|n| lit_bytes(n.unwrap_literal_ref()))
            .reduce(|n1, n2| Ok(chain(n1?, n2?).collect_vec()))
            .unwrap()
    }

    let inner_ty = tplt_ty.inner_ty();

    let e = match inner_ty {
        // there is a special overload `bitcast<u32>(AbstractInt)` which prevents
        // `AbstractInt` from being automatically converted to `i32`, then `u32`.
        Type::U32 if e.ty().is_abstract_int() => e.convert_inner_to(&Type::U32),
        _ => e.concretize(),
    };

    let bytes = match e {
        Some(Instance::Literal(l)) => lit_bytes(&l),
        Some(Instance::Vec(v)) => vec_bytes(&v),
        _ => Err(E::Builtin(
            "`bitcast` expects a numeric scalar or vector argument",
        )),
    }?;

    let size_err = E::Builtin("`bitcast` input and output types must have the same size");

    match tplt_ty {
        Type::I32 => {
            let n = i32::from_le_bytes(bytes.try_into().map_err(|_| size_err)?);
            Ok(LiteralInstance::I32(n).into())
        }
        Type::U32 => {
            let n = u32::from_le_bytes(bytes.try_into().map_err(|_| size_err)?);
            Ok(LiteralInstance::U32(n).into())
        }
        Type::F32 => {
            let n = f32::from_le_bytes(bytes.try_into().map_err(|_| size_err)?);
            Ok(LiteralInstance::F32(n).into())
        }
        Type::F16 => {
            let n = f16::from_le_bytes(bytes.try_into().map_err(|_| size_err)?);
            Ok(LiteralInstance::F16(n).into())
        }
        Type::Vec(n, ty) => {
            if **ty == Type::I32 && bytes.len() == 4 * (*n as usize) {
                let v = bytes
                    .chunks(4)
                    .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
                    .map(|n| LiteralInstance::from(n).into())
                    .collect_vec();
                Ok(VecInstance::new(v).into())
            } else if **ty == Type::U32 && bytes.len() == 4 * (*n as usize) {
                let v = bytes
                    .chunks(4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .map(|n| LiteralInstance::from(n).into())
                    .collect_vec();
                Ok(VecInstance::new(v).into())
            } else if **ty == Type::F32 && bytes.len() == 4 * (*n as usize) {
                let v = bytes
                    .chunks(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .map(|n| LiteralInstance::from(n).into())
                    .collect_vec();
                Ok(VecInstance::new(v).into())
            } else if **ty == Type::F16 && bytes.len() == 2 * (*n as usize) {
                let v = bytes
                    .chunks(2)
                    .map(|b| f16::from_le_bytes(b.try_into().unwrap()))
                    .map(|n| LiteralInstance::from(n).into())
                    .collect_vec();
                Ok(VecInstance::new(v).into())
            } else {
                #[cfg(feature = "naga-ext")]
                if matches!(**ty, Type::I64 | Type::U64 | Type::F64) {
                    return Err(E::Todo(format!("`bitcast` with 64-bit type")));
                }
                Err(size_err)
            }
        }
        #[cfg(feature = "naga-ext")]
        Type::I64 | Type::U64 | Type::F64 => Err(E::Todo(format!("`bitcast` with 64-bit type"))),
        _ => Err(E::Builtin("invalid `bitcast` template")),
    }
}

// -------
// LOGICAL
// -------
// reference: <https://www.w3.org/TR/WGSL/#logical-builtin-functions>

/// `all()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#all-builtin>
pub fn all(e: &Instance) -> Result<Instance, E> {
    match e {
        Instance::Literal(LiteralInstance::Bool(_)) => Ok(e.clone()),
        Instance::Vec(v) if v.inner_ty() == Type::Bool => {
            let b = v.iter().all(|b| b.unwrap_literal_ref().unwrap_bool());
            Ok(LiteralInstance::Bool(b).into())
        }
        _ => Err(E::Builtin(
            "`all` expects a boolean or vector of boolean argument",
        )),
    }
}

/// `any()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#any-builtin>
pub fn any(e: &Instance) -> Result<Instance, E> {
    match e {
        Instance::Literal(LiteralInstance::Bool(_)) => Ok(e.clone()),
        Instance::Vec(v) if v.inner_ty() == Type::Bool => {
            let b = v.iter().any(|b| b.unwrap_literal_ref().unwrap_bool());
            Ok(LiteralInstance::Bool(b).into())
        }
        _ => Err(E::Builtin(
            "`any` expects a boolean or vector of boolean argument",
        )),
    }
}

/// `select()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#select-builtin>
pub fn select(f: &Instance, t: &Instance, cond: &Instance) -> Result<Instance, E> {
    let (f, t) = convert(f, t).ok_or(E::Builtin(
        "`select` 1st and 2nd arguments are incompatible",
    ))?;

    match cond {
        Instance::Literal(LiteralInstance::Bool(b)) => Ok(b.then_some(t).unwrap_or(f)),
        Instance::Vec(v) if v.inner_ty() == Type::Bool => match (f, t) {
            (Instance::Vec(v1), Instance::Vec(v2)) => {
                if v1.n() != v.n() {
                    Err(E::Builtin(
                        "`select` vector arguments must have the same number of components",
                    ))
                } else {
                    let v = izip!(v1, v2, v.iter())
                        .map(|(f, t, b)| {
                            if b.unwrap_literal_ref().unwrap_bool() {
                                t.to_owned() // BUG: is it a bug in rust_analyzer? it displays f as Instance and t as &Instance
                            } else {
                                f.to_owned()
                            }
                        })
                        .collect_vec();
                    Ok(VecInstance::new(v).into())
                }
            }
            _ => Err(E::Builtin(
                "`select` arguments must be vectors when the condition is a vector",
            )),
        },
        _ => Err(E::Builtin(
            "`select` 3rd argument must be a boolean or vector of boolean",
        )),
    }
}

// -----
// ARRAY
// -----
// reference: <https://www.w3.org/TR/WGSL/#array-builtin-functions>

/// `arrayLength()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#arrayLength-builtin>
pub fn arrayLength(p: &Instance) -> Result<Instance, E> {
    let err = E::Builtin("`arrayLength` expects a pointer to array argument");
    let r = match p {
        Instance::Ptr(p) => RefInstance::from(p.clone()),
        _ => return Err(err),
    };
    let r = r.read()?;
    match &*r {
        Instance::Array(a) => Ok(LiteralInstance::U32(a.n() as u32).into()),
        _ => Err(err),
    }
}

// -------
// NUMERIC
// -------
// reference: <https://www.w3.org/TR/WGSL/#numeric-builtin-function>

macro_rules! impl_call_float_unary {
    ($name:literal, $e:ident, $n:ident => $expr:expr) => {{
        const ERR: E = E::Builtin(concat!(
            "`",
            $name,
            "` expects a float or vector of float argument"
        ));
        fn lit_fn(l: &LiteralInstance) -> Result<LiteralInstance, E> {
            match l {
                LiteralInstance::Bool(_) => Err(ERR),
                LiteralInstance::AbstractInt(_) => {
                    let $n = l
                        .convert_to(&Type::AbstractFloat)
                        .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?
                        .unwrap_abstract_float();
                    Ok(LiteralInstance::from($expr))
                }
                LiteralInstance::AbstractFloat($n) => Ok(LiteralInstance::from($expr)),
                LiteralInstance::I32(_) => Err(ERR),
                LiteralInstance::U32(_) => Err(ERR),
                LiteralInstance::F32($n) => Ok(LiteralInstance::from($expr)),
                LiteralInstance::F16($n) => Ok(LiteralInstance::from($expr)),
                #[cfg(feature = "naga-ext")]
                LiteralInstance::I64(_) => Err(ERR),
                #[cfg(feature = "naga-ext")]
                LiteralInstance::U64(_) => Err(ERR),
                #[cfg(feature = "naga-ext")]
                LiteralInstance::F64($n) => Ok(LiteralInstance::F64($expr)),
            }
        }
        match $e {
            Instance::Literal(l) => lit_fn(l).map(Into::into),
            Instance::Vec(v) => v.compwise_unary(lit_fn).map(Into::into),
            _ => Err(ERR),
        }
    }};
}

/// `abs()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#abs-float-builtin>
// TODO: use checked_abs
pub fn abs(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`abs` expects a scalar or vector of scalar argument");
    fn lit_abs(l: &LiteralInstance) -> Result<LiteralInstance, E> {
        match l {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(n) => Ok(LiteralInstance::from(n.wrapping_abs())),
            LiteralInstance::AbstractFloat(n) => Ok(LiteralInstance::from(n.abs())),
            LiteralInstance::I32(n) => Ok(LiteralInstance::from(n.wrapping_abs())),
            LiteralInstance::U32(_) => Ok(*l),
            LiteralInstance::F32(n) => Ok(LiteralInstance::from(n.abs())),
            LiteralInstance::F16(n) => Ok(LiteralInstance::from(n.abs())),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(n) => Ok(LiteralInstance::I64(n.wrapping_abs())),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Ok(*l),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => Ok(LiteralInstance::F64(n.abs())),
        }
    }
    match e {
        Instance::Literal(l) => lit_abs(l).map(Into::into),
        Instance::Vec(v) => v.compwise_unary(lit_abs).map(Into::into),
        _ => Err(ERR),
    }
}

/// `acos()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#acos-builtin>
pub fn acos(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("acos", e, n => n.acos())
}

/// `acosh()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#acosh-builtin>
pub fn acosh(e: &Instance) -> Result<Instance, E> {
    // TODO: Rust's acosh implementation overflows for inputs close to max_float.
    // it's no big deal, but some cts tests fail because of that.
    impl_call_float_unary!("acosh", e, n => n.acosh())
}

/// `asin()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#asin-builtin>
pub fn asin(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("asin", e, n => n.asin())
}

/// `asinh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#asinh-builtin>
pub fn asinh(e: &Instance) -> Result<Instance, E> {
    // TODO: Rust's asinh implementation overflows for inputs close to max_float.
    // it's no big deal, but some cts tests fail because of that.
    impl_call_float_unary!("asinh", e, n => n.asinh())
}

/// `atan()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atan-builtin>
pub fn atan(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("atan", e, n => n.atan())
}

/// `atanh()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#atanh-builtin>
pub fn atanh(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("atanh", e, n => n.atanh())
}

/// `atan2()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atan2-builtin>
pub fn atan2(y: &Instance, x: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`atan2` expects a float or vector of float argument");
    fn lit_atan2(y: &LiteralInstance, x: &LiteralInstance) -> Result<LiteralInstance, E> {
        match y {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => {
                let y = y
                    .convert_to(&Type::AbstractFloat)
                    .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?;
                let x = x
                    .convert_to(&Type::AbstractFloat)
                    .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?;
                Ok(LiteralInstance::from(
                    y.unwrap_abstract_float().atan2(x.unwrap_abstract_float()),
                ))
            }
            LiteralInstance::AbstractFloat(y) => {
                Ok(LiteralInstance::from(y.atan2(x.unwrap_abstract_float())))
            }
            LiteralInstance::I32(_) => Err(ERR),
            LiteralInstance::U32(_) => Err(ERR),
            LiteralInstance::F32(y) => Ok(LiteralInstance::from(y.atan2(x.unwrap_f32()))),
            LiteralInstance::F16(y) => Ok(LiteralInstance::from(y.atan2(x.unwrap_f16()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(y) => Ok(LiteralInstance::F64(y.atan2(x.unwrap_f64()))),
        }
    }
    let (y, x) = convert(y, x).ok_or(E::Builtin("`atan2` arguments are incompatible"))?;
    match (y, x) {
        (Instance::Literal(y), Instance::Literal(x)) => lit_atan2(&y, &x).map(Into::into),
        (Instance::Vec(y), Instance::Vec(x)) => y.compwise_binary(&x, lit_atan2).map(Into::into),
        _ => Err(ERR),
    }
}

/// `ceil()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#ceil-builtin>
pub fn ceil(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("ceil", e, n => n.ceil())
}

/// `clamp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#clamp>
pub fn clamp(e: &Instance, low: &Instance, high: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`clamp` arguments are incompatible");
    let tys = [e.ty(), low.ty(), high.ty()];
    let ty = convert_all_ty(&tys).ok_or(ERR)?;
    let e = e.convert_to(ty).ok_or(ERR)?;
    let low = low.convert_to(ty).ok_or(ERR)?;
    let high = high.convert_to(ty).ok_or(ERR)?;
    min(&max(&e, &low)?, &high)
}

/// `cos()` builtin function.
///
/// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#cos-builtin>
pub fn cos(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("cos", e, n => n.cos())
}

/// `cosh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#cosh-builtin>
pub fn cosh(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("cosh", e, n => n.cosh())
}

/// `countLeadingZeros()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#countLeadingZeros-builtin>
pub fn countLeadingZeros(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`countLeadingZeros` expects an integer scalar or vector argument");
    fn lit_leading_zeros(l: &LiteralInstance) -> Result<LiteralInstance, E> {
        match l {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => Err(E::Unreachable), // because of `concretize` below
            LiteralInstance::AbstractFloat(_) => Err(E::Unreachable),
            LiteralInstance::I32(n) => Ok(LiteralInstance::I32(n.leading_zeros() as i32)),
            LiteralInstance::U32(n) => Ok(LiteralInstance::U32(n.leading_zeros())),
            LiteralInstance::F32(_) => Err(ERR),
            LiteralInstance::F16(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(n) => Ok(LiteralInstance::I64(n.leading_zeros() as i64)),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(n) => Ok(LiteralInstance::U64(n.leading_zeros() as u64)),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(_) => Err(ERR),
        }
    }
    match e.concretize() {
        Some(Instance::Literal(l)) => lit_leading_zeros(&l).map(Into::into),
        Some(Instance::Vec(v)) => v.compwise_unary(lit_leading_zeros).map(Into::into),
        _ => Err(ERR),
    }
}

/// `countOneBits()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#countOneBits-builtin>
pub fn countOneBits(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`countOneBits` expects an integer scalar or vector argument");
    fn lit_count_ones(l: &LiteralInstance) -> Result<LiteralInstance, E> {
        match l {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => Err(E::Unreachable), // because of `concretize` below
            LiteralInstance::AbstractFloat(_) => Err(E::Unreachable),
            LiteralInstance::I32(n) => Ok(LiteralInstance::I32(n.count_ones() as i32)),
            LiteralInstance::U32(n) => Ok(LiteralInstance::U32(n.count_ones())),
            LiteralInstance::F32(_) => Err(ERR),
            LiteralInstance::F16(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(n) => Ok(LiteralInstance::I64(n.count_ones() as i64)),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(n) => Ok(LiteralInstance::U64(n.count_ones() as u64)),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(_) => Err(ERR),
        }
    }
    match e.concretize() {
        Some(Instance::Literal(l)) => lit_count_ones(&l).map(Into::into),
        Some(Instance::Vec(v)) => v.compwise_unary(lit_count_ones).map(Into::into),
        _ => Err(ERR),
    }
}

/// `countTrailingZeros()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#countTrailingZeros-builtin>
pub fn countTrailingZeros(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`countTrailingZeros` expects an integer scalar or vector argument");
    fn lit_trailing_zeros(l: &LiteralInstance) -> Result<LiteralInstance, E> {
        match l {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => Err(E::Unreachable), // because of `concretize` below
            LiteralInstance::AbstractFloat(_) => Err(E::Unreachable),
            LiteralInstance::I32(n) => Ok(LiteralInstance::I32(n.trailing_zeros() as i32)),
            LiteralInstance::U32(n) => Ok(LiteralInstance::U32(n.trailing_zeros())),
            LiteralInstance::F32(_) => Err(ERR),
            LiteralInstance::F16(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(n) => Ok(LiteralInstance::I64(n.trailing_zeros() as i64)),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(n) => Ok(LiteralInstance::U64(n.trailing_zeros() as u64)),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(_) => Err(ERR),
        }
    }
    match e.concretize() {
        Some(Instance::Literal(l)) => lit_trailing_zeros(&l).map(Into::into),
        Some(Instance::Vec(v)) => v.compwise_unary(lit_trailing_zeros).map(Into::into),
        _ => Err(ERR),
    }
}

/// `cross()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#cross-builtin>
pub fn cross(a: &Instance, b: &Instance, stage: ShaderStage) -> Result<Instance, E> {
    let (a, b) = convert(a, b).ok_or(E::Builtin("`cross` arguments are incompatible"))?;
    match (a, b) {
        (Instance::Vec(a), Instance::Vec(b)) if a.n() == 3 => {
            let s1 = a[1]
                .op_mul(&b[2], stage)?
                .op_sub(&a[2].op_mul(&b[1], stage)?, stage)?;
            let s2 = a[2]
                .op_mul(&b[0], stage)?
                .op_sub(&a[0].op_mul(&b[2], stage)?, stage)?;
            let s3 = a[0]
                .op_mul(&b[1], stage)?
                .op_sub(&a[1].op_mul(&b[0], stage)?, stage)?;
            Ok(VecInstance::new(vec![s1, s2, s3]).into())
        }
        _ => Err(E::Builtin(
            "`cross` expects a 3-component vector of float arguments",
        )),
    }
}

/// `degrees()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#degrees-builtin>
pub fn degrees(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("degrees", e, n => n.to_degrees())
}

/// TODO: This built-in is not implemented!
pub fn determinant(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("determinant".to_string()))
}

/// `distance()` builtin function.
///
/// NOTE: the function returns an error if computed out of domain
///
/// Reference: <https://www.w3.org/TR/WGSL/#distance-builtin>
pub fn distance(e1: &Instance, e2: &Instance, stage: ShaderStage) -> Result<Instance, E> {
    length(&e1.op_sub(e2, stage)?)
}

/// `dot()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#dot-builtin>
pub fn dot(e1: &Instance, e2: &Instance, stage: ShaderStage) -> Result<Instance, E> {
    let (e1, e2) = convert(e1, e2).ok_or(E::Builtin("`dot` arguments are incompatible"))?;
    match (e1, e2) {
        (Instance::Vec(e1), Instance::Vec(e2)) => e1.dot(&e2, stage).map(Into::into),
        _ => Err(E::Builtin("`dot` expects vector arguments")),
    }
}

/// TODO: This built-in is not implemented!
pub fn dot4U8Packed(_a1: &Instance, _a2: &Instance) -> Result<Instance, E> {
    Err(E::Todo("dot4U8Packed".to_string()))
}

/// TODO: This built-in is not implemented!
pub fn dot4I8Packed(_a1: &Instance, _a2: &Instance) -> Result<Instance, E> {
    Err(E::Todo("dot4I8Packed".to_string()))
}

/// `exp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#exp-builtin>
pub fn exp(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("exp", e, n => n.exp())
}

/// `exp2()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#exp2-builtin>
pub fn exp2(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("exp2", e, n => n.exp2())
}

/// TODO: This built-in is not implemented!
pub fn extractBits(_a1: &Instance, _a2: &Instance, _a3: &Instance) -> Result<Instance, E> {
    Err(E::Todo("extractBits".to_string()))
}

/// TODO: This built-in is not implemented!
pub fn faceForward(_a1: &Instance, _a2: &Instance, _a3: &Instance) -> Result<Instance, E> {
    Err(E::Todo("faceForward".to_string()))
}

/// TODO: This built-in is not implemented!
pub fn firstLeadingBit(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("firstLeadingBit".to_string()))
}

/// TODO: This built-in is not implemented!
pub fn firstTrailingBit(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("firstTrailingBit".to_string()))
}

/// `floor()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#floor-builtin>
pub fn floor(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("floor", e, n => n.floor())
}

/// TODO: This built-in is not implemented!
pub fn fma(_a1: &Instance, _a2: &Instance, _a3: &Instance) -> Result<Instance, E> {
    Err(E::Todo("fma".to_string()))
}

/// `fract()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#fract-builtin>
pub fn fract(e: &Instance, stage: ShaderStage) -> Result<Instance, E> {
    e.op_sub(&floor(e)?, stage)
    // impl_call_float_unary!("fract", e, n => n.fract())
}

/// `frexp()` builtin function.
///
/// TODO: This built-in is only partially implemented.
///
/// Reference: <https://www.w3.org/TR/WGSL/#frexp-builtin>
pub fn frexp(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`frexp` expects a float or vector of float argument");
    fn make_frexp_inst(fract: Instance, exp: Instance) -> Instance {
        Instance::Struct(StructInstance::new(
            frexp_struct_type(&fract.ty()).unwrap(),
            vec![fract, exp],
        ))
    }
    // from: https://docs.rs/libm/latest/src/libm/math/frexp.rs.html#1-20
    fn frexp(x: f64) -> (f64, i32) {
        let mut y = x.to_bits();
        let ee = ((y >> 52) & 0x7ff) as i32;

        if ee == 0 {
            if x != 0.0 {
                let x1p64 = f64::from_bits(0x43f0000000000000);
                let (x, e) = frexp(x * x1p64);
                return (x, e - 64);
            }
            return (x, 0);
        } else if ee == 0x7ff {
            return (x, 0);
        }

        let e = ee - 0x3fe;
        y &= 0x800fffffffffffff;
        y |= 0x3fe0000000000000;
        (f64::from_bits(y), e)
    }
    match e {
        Instance::Literal(l) => match l {
            LiteralInstance::Bool(_) => Err(E::Todo(format!("frexp with bool input"))),
            LiteralInstance::AbstractInt(_) => {
                Err(E::Todo(format!("frexp with AbstractInt input")))
            }
            LiteralInstance::AbstractFloat(n) => {
                let (fract, exp) = frexp(*n);
                Ok(make_frexp_inst(
                    LiteralInstance::AbstractFloat(fract).into(),
                    LiteralInstance::AbstractInt(exp as i64).into(),
                ))
            }
            LiteralInstance::I32(_) => Err(E::Todo(format!("frexp with i32 input"))),
            LiteralInstance::U32(_) => Err(E::Todo(format!("frexp with u32 input"))),
            LiteralInstance::F32(n) => {
                let (fract, exp) = frexp(*n as f64);
                Ok(make_frexp_inst(
                    LiteralInstance::F32(fract as f32).into(),
                    LiteralInstance::I32(exp).into(),
                ))
            }
            LiteralInstance::F16(n) => {
                let (fract, exp) = frexp(n.to_f64().unwrap(/* SAFETY: f16 to f64 is lossless */));
                Ok(make_frexp_inst(
                    LiteralInstance::F16(f16::from_f64(fract)).into(),
                    LiteralInstance::I32(exp).into(),
                ))
            }
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => Err(E::Todo(format!("frexp with i64 input"))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Err(E::Todo(format!("frexp with u64 input"))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => {
                let (fract, exp) = frexp(*n);
                Ok(make_frexp_inst(
                    LiteralInstance::F64(fract).into(),
                    LiteralInstance::I64(exp as i64).into(),
                ))
            }
        },
        Instance::Vec(v) => {
            let ty = v.inner_ty();
            let (fracts, exps): (Vec<_>, Vec<_>) = v
                .iter()
                .map(|l| match l.unwrap_literal_ref() {
                    LiteralInstance::AbstractFloat(n) => Ok(*n),
                    LiteralInstance::F32(n) => Ok(*n as f64),
                    LiteralInstance::F16(n) => {
                        Ok(n.to_f64().unwrap(/* SAFETY: f16 to f64 is lossless */))
                    }
                    _ => Err(ERR),
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(frexp)
                .unzip();
            let fracts = fracts
                .into_iter()
                .map(|n| match ty {
                    Type::AbstractFloat => LiteralInstance::AbstractFloat(n).into(),
                    Type::F32 => LiteralInstance::F32(n as f32).into(),
                    Type::F16 => LiteralInstance::F16(f16::from_f64(n)).into(),
                    _ => unreachable!("case handled above"),
                })
                .collect_vec();
            let exps = exps
                .into_iter()
                .map(|n| match ty {
                    Type::AbstractFloat => LiteralInstance::AbstractInt(n as i64).into(),
                    Type::F32 => LiteralInstance::I32(n).into(),
                    Type::F16 => LiteralInstance::I32(n).into(),
                    _ => unreachable!("case handled above"),
                })
                .collect_vec();
            let fract = VecInstance::new(fracts).into();
            let exp = VecInstance::new(exps).into();
            Ok(make_frexp_inst(fract, exp))
        }
        _ => Err(ERR),
    }
}

/// TODO: This built-in is not implemented!
pub fn insertBits(
    _a1: &Instance,
    _a2: &Instance,
    _a3: &Instance,
    _a4: &Instance,
) -> Result<Instance, E> {
    Err(E::Todo("insertBits".to_string()))
}

/// `inverseSqrt()` builtin function.
///
// NOTE: the function returns NaN as an "indeterminate value" if computed out of domain
//
/// Reference: <https://www.w3.org/TR/WGSL/#inverseSqrt-builtin>
pub fn inverseSqrt(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`inverseSqrt` expects a float or vector of float argument");
    fn lit_isqrt(l: &LiteralInstance) -> Result<LiteralInstance, E> {
        match l {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => l
                .convert_to(&Type::AbstractFloat)
                .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))
                .map(|n| LiteralInstance::from(1.0 / n.unwrap_abstract_float().sqrt())),
            LiteralInstance::AbstractFloat(n) => Ok(LiteralInstance::from(1.0 / n.sqrt())),
            LiteralInstance::I32(_) => Err(ERR),
            LiteralInstance::U32(_) => Err(ERR),
            LiteralInstance::F32(n) => Ok(LiteralInstance::from(1.0 / n.sqrt())),
            LiteralInstance::F16(n) => Ok(LiteralInstance::from(f16::one() / n.sqrt())),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => Ok(LiteralInstance::F64(1.0 / n.sqrt())),
        }
    }
    match e {
        Instance::Literal(l) => lit_isqrt(l).map(Into::into),
        Instance::Vec(v) => v.compwise_unary(lit_isqrt).map(Into::into),
        _ => Err(ERR),
    }
}

/// `ldexp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#ldexp-builtin>
// TODO: shader error if e2 > bias + 1
pub fn ldexp(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    // from: https://docs.rs/libm/latest/src/libm/math/scalbn.rs.html#3-34
    fn scalbn(x: f64, mut n: i32) -> f64 {
        let x1p1023 = f64::from_bits(0x7fe0000000000000); // 0x1p1023 === 2 ^ 1023
        let x1p53 = f64::from_bits(0x4340000000000000); // 0x1p53 === 2 ^ 53
        let x1p_1022 = f64::from_bits(0x0010000000000000); // 0x1p-1022 === 2 ^ (-1022)

        let mut y = x;

        if n > 1023 {
            y *= x1p1023;
            n -= 1023;
            if n > 1023 {
                y *= x1p1023;
                n -= 1023;
                if n > 1023 {
                    n = 1023;
                }
            }
        } else if n < -1022 {
            /* make sure final n < -53 to avoid double
            rounding in the subnormal range */
            y *= x1p_1022 * x1p53;
            n += 1022 - 53;
            if n < -1022 {
                y *= x1p_1022 * x1p53;
                n += 1022 - 53;
                if n < -1022 {
                    n = -1022;
                }
            }
        }
        y * f64::from_bits(((0x3ff + n) as u64) << 52)
    }
    fn ldexp_lit(l1: &LiteralInstance, l2: &LiteralInstance) -> Result<LiteralInstance, E> {
        match (l1, l2) {
            (LiteralInstance::AbstractInt(n1), LiteralInstance::AbstractInt(n2)) => Ok(
                LiteralInstance::AbstractFloat(scalbn(n1.to_f64().unwrap(), n2.to_i32().unwrap())),
            ),
            (LiteralInstance::AbstractFloat(n1), LiteralInstance::AbstractInt(n2)) => Ok(
                LiteralInstance::AbstractFloat(scalbn(*n1, n2.to_i32().unwrap())),
            ),
            (LiteralInstance::AbstractInt(n1), LiteralInstance::I32(n2)) => Ok(
                LiteralInstance::F32(scalbn(n1.to_f64().unwrap(), *n2) as f32),
            ),
            (LiteralInstance::AbstractFloat(n1), LiteralInstance::I32(n2)) => Ok(
                LiteralInstance::F32(scalbn(*n1, n2.to_i32().unwrap()) as f32),
            ),
            (LiteralInstance::F32(n1), LiteralInstance::AbstractInt(n2)) => Ok(
                LiteralInstance::F32(scalbn(n1.to_f64().unwrap(), n2.to_i32().unwrap()) as f32),
            ),
            (LiteralInstance::F32(n1), LiteralInstance::I32(n2)) => Ok(LiteralInstance::F32(
                scalbn(n1.to_f64().unwrap(), n2.to_i32().unwrap()) as f32,
            )),
            (LiteralInstance::F16(n1), LiteralInstance::AbstractInt(n2)) => {
                Ok(LiteralInstance::F16(f16::from_f64(scalbn(
                    n1.to_f64().unwrap(),
                    n2.to_i32().unwrap(),
                ))))
            }
            (LiteralInstance::F16(n1), LiteralInstance::I32(n2)) => Ok(LiteralInstance::F16(
                f16::from_f64(scalbn(n1.to_f64().unwrap(), *n2)),
            )),
            _ => Err(E::Builtin(
                "`ldexp` with scalar arguments expects a float and a i32 arguments",
            )),
        }
    }

    // TODO conversion errors
    match (e1, e2) {
        (Instance::Literal(l1), Instance::Literal(l2)) => ldexp_lit(l1, l2).map(Into::into),
        (Instance::Vec(v1), Instance::Vec(v2)) => v1.compwise_binary(v2, ldexp_lit).map(Into::into),
        _ => Err(E::Builtin(
            "`ldexp` expects two scalar or two vector arguments",
        )),
    }
}

/// `length()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#length-builtin>
pub fn length(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`length` expects a float or vector of float argument");
    match e {
        Instance::Literal(LiteralInstance::AbstractInt(_)) => {
            abs(&e.convert_to(&Type::AbstractFloat).unwrap())
        }
        Instance::Literal(l) if l.ty().is_float() => abs(e),
        Instance::Vec(v) => sqrt(
            &v.op_mul(v, ShaderStage::Exec)?
                .into_iter()
                .map(Ok)
                .reduce(|a, b| a?.op_add(&b?, ShaderStage::Exec))
                .unwrap()?,
        ),
        _ => Err(ERR),
    }
}

/// `log()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#log-builtin>
pub fn log(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("log", e, n => n.ln())
}

/// `log2()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#log2-builtin>
pub fn log2(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("log2", e, n => n.log2())
}

/// `max()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#max-builtin>
pub fn max(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`max` expects a scalar or vector of scalar argument");
    fn lit_max(e1: &LiteralInstance, e2: &LiteralInstance) -> Result<LiteralInstance, E> {
        match e1 {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(e1) => {
                Ok(LiteralInstance::from(*e1.max(&e2.unwrap_abstract_int())))
            }
            LiteralInstance::AbstractFloat(e1) => {
                Ok(LiteralInstance::from(e1.max(e2.unwrap_abstract_float())))
            }
            LiteralInstance::I32(e1) => Ok(LiteralInstance::from(*e1.max(&e2.unwrap_i32()))),
            LiteralInstance::U32(e1) => Ok(LiteralInstance::from(*e1.max(&e2.unwrap_u32()))),
            LiteralInstance::F32(e1) => Ok(LiteralInstance::from(e1.max(e2.unwrap_f32()))),
            LiteralInstance::F16(e1) => Ok(LiteralInstance::from(e1.max(e2.unwrap_f16()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(e1) => Ok(LiteralInstance::I64(*e1.max(&e2.unwrap_i64()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(e1) => Ok(LiteralInstance::U64(*e1.max(&e2.unwrap_u64()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(e1) => Ok(LiteralInstance::F64(e1.max(e2.unwrap_f64()))),
        }
    }
    let (e1, e2) = convert(e1, e2).ok_or(E::Builtin("`max` arguments are incompatible"))?;
    match (e1, e2) {
        (Instance::Literal(e1), Instance::Literal(e2)) => lit_max(&e1, &e2).map(Into::into),
        (Instance::Vec(e1), Instance::Vec(e2)) => e1.compwise_binary(&e2, lit_max).map(Into::into),
        _ => Err(ERR),
    }
}

/// `min()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#min-builtin>
pub fn min(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`min` expects a scalar or vector of scalar argument");
    fn lit_min(e1: &LiteralInstance, e2: &LiteralInstance) -> Result<LiteralInstance, E> {
        match e1 {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(e1) => {
                Ok(LiteralInstance::from(*e1.min(&e2.unwrap_abstract_int())))
            }
            LiteralInstance::AbstractFloat(e1) => {
                Ok(LiteralInstance::from(e1.min(e2.unwrap_abstract_float())))
            }
            LiteralInstance::I32(e1) => Ok(LiteralInstance::from(*e1.min(&e2.unwrap_i32()))),
            LiteralInstance::U32(e1) => Ok(LiteralInstance::from(*e1.min(&e2.unwrap_u32()))),
            LiteralInstance::F32(e1) => Ok(LiteralInstance::from(e1.min(e2.unwrap_f32()))),
            LiteralInstance::F16(e1) => Ok(LiteralInstance::from(e1.min(e2.unwrap_f16()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(e1) => Ok(LiteralInstance::I64(*e1.min(&e2.unwrap_i64()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(e1) => Ok(LiteralInstance::U64(*e1.min(&e2.unwrap_u64()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(e1) => Ok(LiteralInstance::F64(e1.min(e2.unwrap_f64()))),
        }
    }
    let (e1, e2) = convert(e1, e2).ok_or(E::Builtin("`min` arguments are incompatible"))?;
    match (e1, e2) {
        (Instance::Literal(e1), Instance::Literal(e2)) => lit_min(&e1, &e2).map(Into::into),
        (Instance::Vec(e1), Instance::Vec(e2)) => e1.compwise_binary(&e2, lit_min).map(Into::into),
        _ => Err(ERR),
    }
}

/// `mix()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#mix-builtin>
pub fn mix(e1: &Instance, e2: &Instance, e3: &Instance, stage: ShaderStage) -> Result<Instance, E> {
    let tys = [e1.inner_ty(), e2.inner_ty(), e3.inner_ty()];
    let inner_ty = convert_all_ty(&tys).ok_or(E::Builtin("`mix` arguments are incompatible"))?;
    let e1 = e1.convert_inner_to(inner_ty).unwrap();
    let e2 = e2.convert_inner_to(inner_ty).unwrap();
    let e3 = e3.convert_inner_to(inner_ty).unwrap();
    let (e1, e2) = convert(&e1, &e2).ok_or(E::Builtin("`mix` arguments are incompatible"))?;

    // TODO is it ok with abstract int? it's supposed to be of type inner_ty
    let one = Instance::Literal(LiteralInstance::AbstractInt(1));

    e1.op_mul(&one.op_sub(&e3, stage)?, stage)?
        .op_add(&e2.op_mul(&e3, stage)?, stage)
}

/// TODO: This built-in is not implemented!
pub fn modf(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("modf".to_string()))
}

/// `normalize()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#normalize-builtin>
pub fn normalize(e: &Instance, stage: ShaderStage) -> Result<Instance, E> {
    e.op_div(&length(e)?, stage)
}

/// `pow()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pow-builtin>
pub fn pow(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pow` expects a scalar or vector of scalar argument");
    fn lit_powf(e1: &LiteralInstance, e2: &LiteralInstance) -> Result<LiteralInstance, E> {
        match e1 {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => {
                let e1 = e1
                    .convert_to(&Type::AbstractFloat)
                    .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?
                    .unwrap_abstract_float();
                let e2 = e2
                    .convert_to(&Type::AbstractFloat)
                    .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?
                    .unwrap_abstract_float();
                Ok(LiteralInstance::from(e1.powf(e2)))
            }
            LiteralInstance::AbstractFloat(e1) => {
                Ok(LiteralInstance::from(e1.powf(e2.unwrap_abstract_float())))
            }
            LiteralInstance::I32(_) => Err(ERR),
            LiteralInstance::U32(_) => Err(ERR),
            LiteralInstance::F32(e1) => Ok(LiteralInstance::from(e1.powf(e2.unwrap_f32()))),
            LiteralInstance::F16(e1) => Ok(LiteralInstance::from(e1.powf(e2.unwrap_f16()))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(e1) => Ok(LiteralInstance::F64(e1.powf(e2.unwrap_f64()))),
        }
    }
    let (e1, e2) = convert(e1, e2).ok_or(E::Builtin("`pow` arguments are incompatible"))?;
    match (e1, e2) {
        (Instance::Literal(e1), Instance::Literal(e2)) => lit_powf(&e1, &e2).map(Into::into),
        (Instance::Vec(e1), Instance::Vec(e2)) => e1.compwise_binary(&e2, lit_powf).map(Into::into),
        _ => Err(ERR),
    }
}

/// TODO: This built-in is not implemented!
pub fn quantizeToF16(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("quantizeToF16".to_string()))
}

/// `radians()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#radians-builtin>
pub fn radians(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("radians", e, n => n.to_radians())
}

/// TODO: This built-in is not implemented!
pub fn reflect(_a1: &Instance, _a2: &Instance) -> Result<Instance, E> {
    Err(E::Todo("reflect".to_string()))
}

/// TODO: This built-in is not implemented!
pub fn refract(_a1: &Instance, _a2: &Instance, _a3: &Instance) -> Result<Instance, E> {
    Err(E::Todo("refract".to_string()))
}

/// TODO: This built-in is not implemented!
pub fn reverseBits(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("reverseBits".to_string()))
}

/// `round()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#round-builtin>
pub fn round(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`round` expects a float or vector of float argument");
    fn lit_fn(l: &LiteralInstance) -> Result<LiteralInstance, E> {
        match l {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => {
                let n = l
                    .convert_to(&Type::AbstractFloat)
                    .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?
                    .unwrap_abstract_float();
                Ok(LiteralInstance::from(n.round_ties_even()))
            }
            LiteralInstance::AbstractFloat(n) => Ok(LiteralInstance::from(n.round_ties_even())),
            LiteralInstance::I32(_) => Err(ERR),
            LiteralInstance::U32(_) => Err(ERR),
            LiteralInstance::F32(n) => Ok(LiteralInstance::from(n.round_ties_even())),
            LiteralInstance::F16(n) => Ok(LiteralInstance::from(f16::from_f32(
                f16::to_f32(*n).round_ties_even(),
            ))),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => Ok(LiteralInstance::F64(n.round_ties_even())),
        }
    }
    match e {
        Instance::Literal(l) => lit_fn(l).map(Into::into),
        Instance::Vec(v) => v.compwise_unary(lit_fn).map(Into::into),
        _ => Err(ERR),
    }
}

/// `saturate()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#saturate-builtin>
pub fn saturate(e: &Instance) -> Result<Instance, E> {
    match e {
        Instance::Literal(_) => {
            let zero = LiteralInstance::AbstractFloat(0.0);
            let one = LiteralInstance::AbstractFloat(1.0);
            clamp(e, &zero.into(), &one.into())
        }
        Instance::Vec(v) => {
            let n = v.n();
            let zero = Instance::from(LiteralInstance::AbstractFloat(0.0));
            let one = Instance::from(LiteralInstance::AbstractFloat(1.0));
            let zero = VecInstance::new((0..n).map(|_| zero.clone()).collect_vec());
            let one = VecInstance::new((0..n).map(|_| one.clone()).collect_vec());
            clamp(e, &zero.into(), &one.into())
        }
        _ => Err(E::Builtin(
            "`saturate` expects a float or vector of float argument",
        )),
    }
}

/// `sign()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sign-builtin>
pub fn sign(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`sign` expects a signed scalar or vector argument");
    fn lit_fn(l: &LiteralInstance) -> Result<LiteralInstance, E> {
        match l {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(n) => Ok(LiteralInstance::from(n.signum())),
            LiteralInstance::AbstractFloat(n) => Ok(LiteralInstance::from(if n.is_zero() {
                *n
            } else {
                n.signum()
            })),
            LiteralInstance::I32(n) => Ok(LiteralInstance::from(n.signum())),
            LiteralInstance::U32(_) => Err(ERR),
            LiteralInstance::F32(n) => Ok(LiteralInstance::from(if n.is_zero() {
                *n
            } else {
                n.signum()
            })),
            LiteralInstance::F16(n) => Ok(LiteralInstance::from(if n.is_zero() {
                *n
            } else {
                n.signum()
            })),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(n) => Ok(LiteralInstance::I64(n.signum())),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => Ok(LiteralInstance::F64(if n.is_zero() {
                *n
            } else {
                n.signum()
            })),
        }
    }
    match e {
        Instance::Literal(l) => lit_fn(l).map(Into::into),
        Instance::Vec(v) => v.compwise_unary(lit_fn).map(Into::into),
        _ => Err(ERR),
    }
}

/// `sin()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sin-builtin>
pub fn sin(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("sin", e, n => n.sin())
}

/// `sinh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sinh-builtin>
pub fn sinh(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("sinh", e, n => n.sinh())
}

/// TODO: This built-in is not implemented!
pub fn smoothstep(_low: &Instance, _high: &Instance, _x: &Instance) -> Result<Instance, E> {
    Err(E::Todo("smoothstep".to_string()))
}

/// `sqrt()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#sqrt-builtin>
pub fn sqrt(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("sqrt", e, n => n.sqrt())
}

/// `step()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#step-builtin>
pub fn step(edge: &Instance, x: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`step` expects a float or vector of float argument");
    fn lit_step(edge: &LiteralInstance, x: &LiteralInstance) -> Result<LiteralInstance, E> {
        match edge {
            LiteralInstance::Bool(_) => Err(ERR),
            LiteralInstance::AbstractInt(_) => {
                let edge = edge
                    .convert_to(&Type::AbstractFloat)
                    .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?
                    .unwrap_abstract_float();
                let x = x
                    .convert_to(&Type::AbstractFloat)
                    .ok_or(E::Conversion(Type::AbstractInt, Type::AbstractFloat))?
                    .unwrap_abstract_float();
                Ok(LiteralInstance::from(if edge <= x {
                    1.0
                } else {
                    0.0
                }))
            }
            LiteralInstance::AbstractFloat(edge) => Ok(LiteralInstance::from(
                if *edge <= x.unwrap_abstract_float() {
                    1.0
                } else {
                    0.0
                },
            )),
            LiteralInstance::I32(_) => Err(ERR),
            LiteralInstance::U32(_) => Err(ERR),
            LiteralInstance::F32(edge) => Ok(LiteralInstance::from(if *edge <= x.unwrap_f32() {
                1.0
            } else {
                0.0
            })),
            LiteralInstance::F16(edge) => Ok(LiteralInstance::from(if *edge <= x.unwrap_f16() {
                1.0
            } else {
                0.0
            })),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(_) => Err(ERR),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(edge) => Ok(LiteralInstance::F64(if *edge <= x.unwrap_f64() {
                1.0
            } else {
                0.0
            })),
        }
    }
    let (edge, x) = convert(edge, x).ok_or(E::Builtin("`step` arguments are incompatible"))?;
    match (edge, x) {
        (Instance::Literal(edge), Instance::Literal(x)) => lit_step(&edge, &x).map(Into::into),
        (Instance::Vec(edge), Instance::Vec(x)) => {
            edge.compwise_binary(&x, lit_step).map(Into::into)
        }
        _ => Err(ERR),
    }
}

/// `tan()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#tan-builtin>
pub fn tan(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("tan", e, n => n.tan())
}

/// `tanh()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#tanh-builtin>
pub fn tanh(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("tanh", e, n => n.tanh())
}

/// `transpose()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#transpose-builtin>
pub fn transpose(e: &Instance) -> Result<Instance, E> {
    match e {
        Instance::Mat(e) => Ok(e.transpose().into()),
        _ => Err(E::Builtin("`transpose` expects a matrix argument")),
    }
}

/// `trunc()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#trunc-builtin>
pub fn trunc(e: &Instance) -> Result<Instance, E> {
    impl_call_float_unary!("trunc", e, n => n.trunc())
}

// ------
// ATOMIC
// ------
// reference: <https://www.w3.org/TR/WGSL/#atomic-builtin-functions>

/// `atomicLoad()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicLoad-builtin>
pub fn atomicLoad(e: &Instance) -> Result<Instance, E> {
    let err = E::Builtin("`atomicLoad` expects a pointer to atomic argument");
    if let Instance::Ptr(ptr) = e {
        // TODO: there is a ptr.ptr.ptr chain here. Rename it.
        let inst = ptr.ptr.read()?;
        if let Instance::Atomic(inst) = &*inst {
            Ok(inst.inner().clone())
        } else {
            Err(err)
        }
    } else {
        Err(err)
    }
}

/// `atomicStore()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicStore-builtin>
pub fn atomicStore(e1: &Instance, e2: &Instance) -> Result<(), E> {
    let err = E::Builtin("`atomicStore` expects a pointer to atomic argument");
    if let Instance::Ptr(ptr) = e1 {
        let mut inst = ptr.ptr.read_write()?;
        if let Instance::Atomic(inst) = &mut *inst {
            let ty = inst.inner().ty();
            let e2 = e2
                .convert_to(&ty)
                .ok_or_else(|| E::ParamType(ty, e2.ty()))?;
            *inst = AtomicInstance::new(e2);
            Ok(())
        } else {
            Err(err)
        }
    } else {
        Err(err)
    }
}

/// `atomicAdd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicSub-builtin>
pub fn atomicAdd(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, &initial.op_add(e2, ShaderStage::Exec)?)?;
    Ok(initial)
}

/// `atomicSub()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicSub-builtin>
pub fn atomicSub(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, &initial.op_sub(e2, ShaderStage::Exec)?)?;
    Ok(initial)
}

/// `atomicMax()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicMax-builtin>
pub fn atomicMax(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, &max(&initial, e2)?)?;
    Ok(initial)
}

/// `atomicMin()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicMin-builtin>
pub fn atomicMin(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, &min(&initial, e2)?)?;
    Ok(initial)
}

/// `atomicAnd()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicAnd-builtin>
pub fn atomicAnd(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, &initial.op_bitand(e2)?)?;
    Ok(initial)
}

/// `atomicOr()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicOr-builtin>
pub fn atomicOr(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, &initial.op_bitor(e2)?)?;
    Ok(initial)
}

/// `atomicXor()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicXor-builtin>
pub fn atomicXor(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, &initial.op_bitxor(e2)?)?;
    Ok(initial)
}

/// `atomicExchange()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicExchange-builtin>
pub fn atomicExchange(e1: &Instance, e2: &Instance) -> Result<Instance, E> {
    let initial = atomicLoad(e1)?;
    atomicStore(e1, e2)?;
    Ok(initial)
}

/// `atomicCompareExchangeWeak()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomicCompareExchangeWeak-builtin>
pub fn atomicCompareExchangeWeak(
    atomic_ptr: &Instance,
    cmp: &Instance,
    v: &Instance,
) -> Result<Instance, E> {
    let old_value = atomicLoad(atomic_ptr)?;

    let ty = old_value.ty();

    let Some(cmp) = cmp.convert_to(&ty) else {
        return Err(E::ParamType(ty, cmp.ty()));
    };

    let Some(v) = v.convert_to(&ty) else {
        return Err(E::ParamType(ty, v.ty()));
    };

    let exchanged = old_value == cmp;
    if exchanged {
        atomicStore(atomic_ptr, &v)?;
    }
    Ok(Instance::Struct(StructInstance::new(
        atomic_compare_exchange_struct_type(&old_value.ty()),
        vec![old_value, LiteralInstance::Bool(exchanged).into()],
    )))
}

// ------------
// DATA PACKING
// ------------
// reference: <https://www.w3.org/TR/WGSL/#pack-builtin-functions>

/// `pack4x8snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4x8snorm-builtin>
pub fn pack4x8snorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack4x8snorm` expects a `vec4<f32>` argument");

    let v = e
        .convert_to(&Type::Vec(4, Type::F32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..4 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_f32();
        let bits = (0.5 + 127.0 * val.clamp(-1.0, 1.0)).floor() as i8 as u8;
        result |= (bits as u32) << (8 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack4x8unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4x8unorm-builtin>
pub fn pack4x8unorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack4x8unorm` expects a `vec4<f32>` argument");

    let v = e
        .convert_to(&Type::Vec(4, Type::F32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..4 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_f32();
        let bits = (0.5 + 255.0 * val.clamp(0.0, 1.0)).floor() as u8;
        result |= (bits as u32) << (8 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack4xI8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xI8-builtin>
pub fn pack4xI8(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack4xI8` expects a `vec4<i32>` argument");

    let v = e
        .convert_to(&Type::Vec(4, Type::I32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..4 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_i32();
        result |= (val as u8 as u32) << (8 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack4xU8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xU8-builtin>
pub fn pack4xU8(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack4xU8` expects a `vec4<u32>` argument");

    let v = e
        .convert_to(&Type::Vec(4, Type::U32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..4 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_u32();
        result |= (val as u8 as u32) << (8 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack4xI8Clamp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xI8Clamp-builtin>
pub fn pack4xI8Clamp(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack4xI8Clamp` expects a `vec4<i32>` argument");

    let v = e
        .convert_to(&Type::Vec(4, Type::I32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..4 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_i32();
        result |= (val.clamp(-128, 127) as i8 as u32) << (8 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack4xU8Clamp()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack4xU8Clamp-builtin>
pub fn pack4xU8Clamp(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack4xU8Clamp` expects a `vec4<u32>` argument");

    let v = e
        .convert_to(&Type::Vec(4, Type::U32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..4 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_u32();
        result |= (val.min(255) as u8 as u32) << (8 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack2x16snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack2x16snorm-builtin>
pub fn pack2x16snorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack2x16snorm` expects a `vec2<f32>` argument");

    let v = e
        .convert_to(&Type::Vec(2, Type::F32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..2 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_f32();
        let bits = (0.5 + 32767.0 * val.clamp(-1.0, 1.0)).floor() as i16 as u16;
        result |= (bits as u32) << (16 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack2x16unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack2x16unorm-builtin>
pub fn pack2x16unorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack2x16unorm` expects a `vec2<f32>` argument");

    let v = e
        .convert_to(&Type::Vec(2, Type::F32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..2 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_f32();
        let bits = (0.5 + 65535.0 * val.clamp(0.0, 1.0)).floor() as u16;
        result |= (bits as u32) << (16 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `pack2x16float()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#pack2x16float-builtin>
pub fn pack2x16float(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`pack2x16float` expects a `vec2<f32>` argument");

    let v = e
        .convert_to(&Type::Vec(2, Type::F32.into()))
        .ok_or(ERR)?
        .unwrap_vec();

    let mut result = 0u32;
    for i in 0..2 {
        let val = v.get(i).unwrap().unwrap_literal_ref().unwrap_f32();
        let bits = f16::from_f32(val).to_bits();
        result |= (bits as u32) << (16 * i);
    }
    Ok(LiteralInstance::U32(result).into())
}

/// `unpack4x8snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4x8snorm-builtin>
pub fn unpack4x8snorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`unpack4x8snorm` expects a `u32` argument");

    let e = e
        .convert_to(&Type::U32)
        .ok_or(ERR)?
        .unwrap_literal()
        .unwrap_u32();

    let comps = e
        .to_le_bytes()
        .map(|c| ((c as i8 as f32) / 127.0).max(-1.0))
        .map(Instance::from)
        .to_vec();

    Ok(VecInstance::new(comps).into())
}

/// `unpack4x8unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4x8unorm-builtin>
pub fn unpack4x8unorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`unpack4x8unorm` expects a `u32` argument");

    let e = e
        .convert_to(&Type::U32)
        .ok_or(ERR)?
        .unwrap_literal()
        .unwrap_u32();

    let comps = e
        .to_le_bytes()
        .map(|c| (c as f32) / 255.0)
        .map(Instance::from)
        .to_vec();

    Ok(VecInstance::new(comps).into())
}

/// `unpack4xI8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4xI8-builtin>
pub fn unpack4xI8(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`unpack4xI8` expects a `u32` argument");

    let e = e
        .convert_to(&Type::U32)
        .ok_or(ERR)?
        .unwrap_literal()
        .unwrap_u32();

    let comps = e
        .to_le_bytes()
        .map(|c| c as i8 as i32)
        .map(Instance::from)
        .to_vec();

    Ok(VecInstance::new(comps).into())
}

/// `unpack4xU8()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack4xU8-builtin>
pub fn unpack4xU8(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`unpack4xU8` expects a `u32` argument");

    let e = e
        .convert_to(&Type::U32)
        .ok_or(ERR)?
        .unwrap_literal()
        .unwrap_u32();

    let comps = e
        .to_le_bytes()
        .map(|c| c as u32)
        .map(Instance::from)
        .to_vec();

    Ok(VecInstance::new(comps).into())
}

/// `unpack2x16snorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack2x16snorm-builtin>
pub fn unpack2x16snorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`unpack2x16snorm` expects a `u32` argument");

    let e = e
        .convert_to(&Type::U32)
        .ok_or(ERR)?
        .unwrap_literal()
        .unwrap_u32();

    let lsb = e as u16 as i16;
    let msb = (e >> 16) as u16 as i16;

    let comps = [lsb, msb]
        .map(|c| ((c as f32) / 32767.0).max(-1.0))
        .map(Instance::from)
        .to_vec();

    Ok(VecInstance::new(comps).into())
}

/// `unpack2x16unorm()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack2x16unorm-builtin>
pub fn unpack2x16unorm(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`unpack2x16unorm` expects a `u32` argument");

    let e = e
        .convert_to(&Type::U32)
        .ok_or(ERR)?
        .unwrap_literal()
        .unwrap_u32();

    let lsb = e as u16;
    let msb = (e >> 16) as u16;

    let comps = [lsb, msb]
        .map(|c| (c as f32) / 65535.0)
        .map(Instance::from)
        .to_vec();

    Ok(VecInstance::new(comps).into())
}

/// `unpack2x16float()` builtin function.
///
/// Reference: <https://www.w3.org/TR/WGSL/#unpack2x16float-builtin>
pub fn unpack2x16float(e: &Instance) -> Result<Instance, E> {
    const ERR: E = E::Builtin("`unpack2x16float` expects a `u32` argument");

    let e = e
        .convert_to(&Type::U32)
        .ok_or(ERR)?
        .unwrap_literal()
        .unwrap_u32();

    let lsb = e as u16;
    let msb = (e >> 16) as u16;

    let comps = [lsb, msb]
        .map(|c| f16::from_bits(c).to_f32())
        .map(Instance::from)
        .to_vec();

    Ok(VecInstance::new(comps).into())
}

impl VecInstance {
    /// Warning, this function does not check operand types
    pub fn dot(&self, rhs: &VecInstance, stage: ShaderStage) -> Result<LiteralInstance, E> {
        self.compwise_binary(rhs, |a, b| a.op_mul(b, stage))?
            .into_iter()
            .map(|c| Ok(c.unwrap_literal()))
            .reduce(|a, b| a?.op_add(&b?, stage))
            .unwrap()
    }
}

impl MatInstance {
    /// Warning, this function does not check operand types
    pub fn transpose(&self) -> MatInstance {
        let components = (0..self.r())
            .map(|j| {
                VecInstance::new(
                    (0..self.c())
                        .map(|i| self.get(i, j).unwrap().clone())
                        .collect_vec(),
                )
                .into()
            })
            .collect_vec();
        MatInstance::from_cols(components)
    }
}
