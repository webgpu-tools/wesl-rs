//! Constructor implementations, including zero-value constructors.
//!
//! Functions bear the same name as the WGSL counterpart.
//! Functions that take template parameters are suffixed with `_t` and take `tplt_*` arguments.
//!
//! ### Usage quirks
//!
//! * The arguments must be [loaded][Type::loaded].
//! * The struct constructor is a bit special since it is the only user-defined type.
//!   Use [`struct_ctor`] and [`typecheck_struct_ctor`] for structs.
//! * User-defined functions can shadow WGSL built-in functions.
//! * Type aliases must be resolved: WGSL allows calling functions with the name of the alias.

use half::prelude::*;
use itertools::Itertools;
use num_traits::{FromPrimitive, One, ToPrimitive, Zero};

use crate::{
    CallSignature, Error, ShaderStage,
    conv::{Convert, convert_all, convert_all_inner_to, convert_all_to, convert_all_ty},
    inst::{ArrayInstance, Instance, LiteralInstance, MatInstance, StructInstance, VecInstance},
    tplt::{ArrayTemplate, MatTemplate, TpltParam, VecTemplate},
    ty::{StructType, Ty, Type},
};

type E = Error;

/// Check if a function name could correspond to a built-in constructor function.
///
/// Warning: WGSL allows shadowing built-in functions. Check that a user-defined
/// function does not shadow the built-in one.
pub fn is_ctor(name: &str) -> bool {
    match name {
        "array" | "bool" | "i32" | "u32" | "f32" | "f16" | "mat2x2" | "mat2x3" | "mat2x4"
        | "mat3x2" | "mat3x3" | "mat3x4" | "mat4x2" | "mat4x3" | "mat4x4" | "vec2" | "vec3"
        | "vec4" => true,
        #[cfg(feature = "naga-ext")]
        "i64" | "u64" | "f64" => true,
        _ => false,
    }
}

// ------------
// CONSTRUCTORS
// ------------
// reference: <https://www.w3.org/TR/WGSL/#constructor-builtin-function>

/// `array<T,N>()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#array-builtin>
pub fn array_t(tplt_ty: &Type, tplt_n: usize, args: &[Instance]) -> Result<Instance, E> {
    let args = args
        .iter()
        .map(|a| {
            a.convert_to(tplt_ty).ok_or_else(|| {
                E::ParamType(Type::Array(Box::new(tplt_ty.clone()), Some(tplt_n)), a.ty())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if args.len() != tplt_n {
        return Err(E::ParamCount("array".to_string(), tplt_n, args.len()));
    }

    Ok(ArrayInstance::new(args, false).into())
}

/// `array()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#array-builtin>
pub fn array(args: &[Instance]) -> Result<Instance, E> {
    let args = convert_all(args).ok_or(E::Builtin("array elements are incompatible"))?;

    if args.is_empty() {
        return Err(E::Builtin("array constructor expects at least 1 argument"));
    }

    Ok(ArrayInstance::new(args, false).into())
}

/// `bool()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#bool-builtin>
pub fn bool(a1: &Instance) -> Result<Instance, E> {
    match a1 {
        Instance::Literal(l) => {
            let zero = LiteralInstance::zero_value(&l.ty())?;
            Ok(LiteralInstance::Bool(*l != zero).into())
        }
        _ => Err(E::Builtin("bool constructor expects a scalar argument")),
    }
}

/// `i32()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#i32-builtin>
// TODO: check that "If T is a floating point type, e is converted to i32, rounding towards zero."
pub fn i32(a1: &Instance) -> Result<Instance, E> {
    match a1 {
        Instance::Literal(l) => {
            let val = match l {
                LiteralInstance::Bool(n) => Some(n.then_some(1).unwrap_or(0)),
                LiteralInstance::AbstractInt(n) => n.to_i32(), // identity if representable
                LiteralInstance::AbstractFloat(n) => Some(*n as i32), // rounding towards 0
                LiteralInstance::I32(n) => Some(*n),           // identity operation
                LiteralInstance::U32(n) => Some(*n as i32),    // reinterpretation of bits
                LiteralInstance::F32(n) => Some((*n as i32).min(2147483520)), // rounding towards 0 AND representable in f32
                LiteralInstance::F16(n) => Some((f16::to_f32(*n) as i32).min(65504)), // rounding towards 0 AND representable in f16
                #[cfg(feature = "naga-ext")]
                LiteralInstance::I64(n) => n.to_i32(), // identity if representable
                #[cfg(feature = "naga-ext")]
                LiteralInstance::U64(n) => n.to_i32(), // identity if representable
                #[cfg(feature = "naga-ext")]
                LiteralInstance::F64(n) => Some(*n as i32), // rounding towards 0
            }
            .ok_or(E::ConvOverflow(*l, Type::I32))?;
            Ok(LiteralInstance::I32(val).into())
        }
        _ => Err(E::Builtin("i32 constructor expects a scalar argument")),
    }
}

/// `u32()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#u32-builtin>
pub fn u32(a1: &Instance) -> Result<Instance, E> {
    match a1 {
        Instance::Literal(l) => {
            let val = match l {
                LiteralInstance::Bool(n) => Some(n.then_some(1).unwrap_or(0)),
                LiteralInstance::AbstractInt(n) => n.to_u32(), // identity if representable
                LiteralInstance::AbstractFloat(n) => Some(*n as u32), // rounding towards 0
                LiteralInstance::I32(n) => Some(*n as u32),    // reinterpretation of bits
                LiteralInstance::U32(n) => Some(*n),           // identity operation
                LiteralInstance::F32(n) => Some((*n as u32).min(4294967040)), // rounding towards 0 AND representable in f32
                LiteralInstance::F16(n) => Some((f16::to_f32(*n) as u32).min(65504)), // rounding towards 0 AND representable in f16
                #[cfg(feature = "naga-ext")]
                LiteralInstance::I64(n) => n.to_u32(), // identity if representable
                #[cfg(feature = "naga-ext")]
                LiteralInstance::U64(n) => n.to_u32(), // identity if representable
                #[cfg(feature = "naga-ext")]
                LiteralInstance::F64(n) => Some(*n as u32), // rounding towards 0
            }
            .ok_or(E::ConvOverflow(*l, Type::U32))?;
            Ok(LiteralInstance::U32(val).into())
        }
        _ => Err(E::Builtin("u32 constructor expects a scalar argument")),
    }
}

/// `f32()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#f32-builtin>
pub fn f32(a1: &Instance, _stage: ShaderStage) -> Result<Instance, E> {
    match a1 {
        Instance::Literal(l) => {
            let val = match l {
                LiteralInstance::Bool(n) => Some(n.then_some(f32::one()).unwrap_or(f32::zero())),
                LiteralInstance::AbstractInt(n) => n.to_f32(), // implicit conversion
                LiteralInstance::AbstractFloat(n) => n.to_f32(), // implicit conversion
                LiteralInstance::I32(n) => Some(*n as f32),    // scalar to float (never overflows)
                LiteralInstance::U32(n) => Some(*n as f32),    // scalar to float (never overflows)
                LiteralInstance::F32(n) => Some(*n),           // identity operation
                LiteralInstance::F16(n) => Some(f16::to_f32(*n)), // exactly representable
                #[cfg(feature = "naga-ext")]
                LiteralInstance::I64(n) => n.to_f32(), // implicit conversion
                #[cfg(feature = "naga-ext")]
                LiteralInstance::U64(n) => n.to_f32(), // implicit conversion
                #[cfg(feature = "naga-ext")]
                LiteralInstance::F64(n) => n.to_f32(), // implicit conversion
            }
            .ok_or(E::ConvOverflow(*l, Type::F32))?;
            Ok(LiteralInstance::F32(val).into())
        }
        _ => Err(E::Builtin("f32 constructor expects a scalar argument")),
    }
}

/// `f16()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#f16-builtin>
pub fn f16(a1: &Instance, stage: ShaderStage) -> Result<Instance, E> {
    match a1 {
        Instance::Literal(l) => {
            let val = match l {
                LiteralInstance::Bool(n) => Some(n.then_some(f16::one()).unwrap_or(f16::zero())),
                LiteralInstance::AbstractInt(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        let range = -65504..=65504;
                        range.contains(n).then_some(f16::from_f32(*n as f32))
                    } else {
                        Some(f16::from_f32(*n as f32))
                    }
                }
                LiteralInstance::AbstractFloat(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        let range = -65504.0..=65504.0;
                        range.contains(n).then_some(f16::from_f32(*n as f32))
                    } else {
                        Some(f16::from_f32(*n as f32))
                    }
                }
                LiteralInstance::I32(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        f16::from_i32(*n)
                    } else {
                        Some(f16::from_f32(*n as f32))
                    }
                }
                LiteralInstance::U32(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        f16::from_u32(*n)
                    } else {
                        Some(f16::from_f32(*n as f32))
                    }
                }
                LiteralInstance::F32(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        let range = -65504.0..=65504.0;
                        range.contains(n).then_some(f16::from_f32(*n))
                    } else {
                        Some(f16::from_f32(*n))
                    }
                }
                LiteralInstance::F16(n) => Some(*n), // identity operation
                #[cfg(feature = "naga-ext")]
                LiteralInstance::I64(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        let range = -65504..=65504;
                        range.contains(n).then_some(f16::from_f32(*n as f32))
                    } else {
                        Some(f16::from_f32(*n as f32))
                    }
                }
                #[cfg(feature = "naga-ext")]
                LiteralInstance::U64(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        f16::from_u64(*n)
                    } else {
                        Some(f16::from_f32(*n as f32))
                    }
                }
                #[cfg(feature = "naga-ext")]
                LiteralInstance::F64(n) => {
                    // scalar to float (can overflow)
                    if stage == ShaderStage::Const {
                        let range = -65504.0..=65504.0;
                        range.contains(n).then_some(f16::from_f32(*n as f32))
                    } else {
                        Some(f16::from_f32(*n as f32))
                    }
                }
            }
            .ok_or(E::ConvOverflow(*l, Type::F16))?;
            Ok(LiteralInstance::F16(val).into())
        }
        _ => Err(E::Builtin("f16 constructor expects a scalar argument")),
    }
}

/// `i64()` constructor (naga extension).
///
/// TODO: This built-in is not implemented!
pub fn i64(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("i64".to_string()))
}

/// `u64()` constructor (naga extension).
///
/// TODO: This built-in is not implemented!
pub fn u64(_a1: &Instance) -> Result<Instance, E> {
    Err(E::Todo("u64".to_string()))
}

/// `f64()` constructor (naga extension).
///
/// TODO: This built-in is not implemented!
pub fn f64(_a1: &Instance, _stage: ShaderStage) -> Result<Instance, E> {
    Err(E::Todo("f64".to_string()))
}

/// `matCxR<T>()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#mat2x2-builtin>
pub fn mat_t(
    c: usize,
    r: usize,
    tplt_ty: &Type,
    args: &[Instance],
    stage: ShaderStage,
) -> Result<Instance, E> {
    // overload 1: mat conversion constructor
    if let [Instance::Mat(m)] = args {
        if m.c() != c || m.r() != r {
            return Err(E::Conversion(
                m.ty(),
                Type::Mat(c as u8, r as u8, Box::new(tplt_ty.clone())),
            ));
        }

        let conv_fn = match tplt_ty {
            Type::F32 => f32,
            Type::F16 => f16,
            _ => return Err(E::Builtin("matrix type must be a f32 or f16")),
        };

        let comps = m
            .iter_cols()
            .map(|v| {
                v.unwrap_vec_ref()
                    .iter()
                    .map(|n| conv_fn(n, stage))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|s| Instance::Vec(VecInstance::new(s)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(MatInstance::from_cols(comps).into())
    } else {
        let ty = args
            .first()
            .ok_or(E::Builtin("matrix constructor expects arguments"))?
            .ty();
        let ty = ty
            .convert_inner_to(tplt_ty)
            .ok_or(E::Conversion(ty.inner_ty(), tplt_ty.clone()))?;
        let args =
            convert_all_to(args, &ty).ok_or(E::Builtin("matrix components are incompatible"))?;

        // overload 2: mat from column vectors
        if ty.is_vec() {
            if args.len() != c {
                return Err(E::ParamCount(format!("mat{c}x{r}"), c, args.len()));
            }

            Ok(MatInstance::from_cols(args).into())
        }
        // overload 3: mat from float values
        else if ty.is_float() {
            if args.len() != c * r {
                return Err(E::ParamCount(format!("mat{c}x{r}"), c * r, args.len()));
            }

            let args = args
                .chunks(r)
                .map(|v| Instance::Vec(VecInstance::new(v.to_vec())))
                .collect_vec();

            Ok(MatInstance::from_cols(args).into())
        } else {
            Err(E::Builtin(
                "matrix constructor expects float or vector of float arguments",
            ))
        }
    }
}

/// `matCxR()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#mat2x2-builtin>
pub fn mat(c: usize, r: usize, args: &[Instance]) -> Result<Instance, E> {
    // overload 1: mat conversion constructor
    if let [Instance::Mat(m)] = args {
        if m.c() != c || m.r() != r {
            let ty2 = Type::Mat(c as u8, r as u8, m.inner_ty().into());
            return Err(E::Conversion(m.ty(), ty2));
        }
        // note: `matCxR(e: matCxR<S>) -> matCxR<S>` is no-op
        Ok(m.clone().into())
    } else {
        let tys = args.iter().map(|a| a.ty()).collect_vec();
        let ty = convert_all_ty(&tys).ok_or(E::Builtin("matrix components are incompatible"))?;
        let mut inner_ty = ty.inner_ty();

        if inner_ty.is_abstract_int() {
            // force conversion from AbstractInt to AbstractFloat
            inner_ty = Type::AbstractFloat;
        } else if !inner_ty.is_float() {
            return Err(E::Builtin(
                "matrix constructor expects float or vector of float arguments",
            ));
        }

        let args = convert_all_inner_to(args, &inner_ty)
            .ok_or(E::Builtin("matrix components are incompatible"))?;

        // overload 2: mat from column vectors
        if ty.is_vec() {
            if args.len() != c {
                return Err(E::ParamCount(format!("mat{c}x{r}"), c, args.len()));
            }

            Ok(MatInstance::from_cols(args).into())
        }
        // overload 3: mat from float values
        else if ty.is_float() || ty.is_abstract_int() {
            if args.len() != c * r {
                return Err(E::ParamCount(format!("mat{c}x{r}"), c * r, args.len()));
            }
            let args = args
                .chunks(r)
                .map(|v| Instance::Vec(VecInstance::new(v.to_vec())))
                .collect_vec();

            Ok(MatInstance::from_cols(args).into())
        } else {
            Err(E::Builtin(
                "matrix constructor expects float or vector of float arguments",
            ))
        }
    }
}

/// `vecN<T>()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#vec2-builtin>
pub fn vec_t(
    n: usize,
    tplt_ty: &Type,
    args: &[Instance],
    stage: ShaderStage,
) -> Result<Instance, E> {
    // overload 1: vec init from single scalar value
    if let [Instance::Literal(l)] = args {
        let val = l
            .convert_to(tplt_ty)
            .map(Instance::Literal)
            .ok_or_else(|| E::ParamType(tplt_ty.clone(), l.ty()))?;
        let comps = (0..n).map(|_| val.clone()).collect_vec();
        Ok(VecInstance::new(comps).into())
    }
    // overload 2: vec conversion constructor
    else if let [Instance::Vec(v)] = args {
        let ty = Type::Vec(n as u8, Box::new(tplt_ty.clone()));
        if v.n() != n {
            return Err(E::Conversion(v.ty(), ty));
        }

        let conv_fn = match ty.inner_ty() {
            Type::Bool => |n, _| bool(n),
            Type::I32 => |n, _| i32(n),
            Type::U32 => |n, _| u32(n),
            Type::F32 => |n, stage| f32(n, stage),
            Type::F16 => |n, stage| f16(n, stage),
            _ => return Err(E::Builtin("vector type must be a scalar")),
        };

        let comps = v
            .iter()
            .map(|n| conv_fn(n, stage))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(VecInstance::new(comps).into())
    }
    // overload 3: vec init from component values
    else {
        // flatten vecN args
        let args = args
            .iter()
            .flat_map(|a| -> Box<dyn Iterator<Item = &Instance>> {
                match a {
                    Instance::Vec(v) => Box::new(v.iter()),
                    _ => Box::new(std::iter::once(a)),
                }
            })
            .collect_vec();
        if args.len() != n {
            return Err(E::ParamCount(format!("vec{n}"), n, args.len()));
        }

        let comps = args
            .iter()
            .map(|a| {
                a.convert_inner_to(tplt_ty)
                    .ok_or_else(|| E::ParamType(tplt_ty.clone(), a.ty()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(VecInstance::new(comps).into())
    }
}

/// `vecN()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#vec2-builtin>
pub fn vec(n: usize, args: &[Instance]) -> Result<Instance, E> {
    // overload 1: vec init from single scalar value
    if let [Instance::Literal(l)] = args {
        let ty = l.ty();
        if !ty.is_scalar() {
            return Err(E::Builtin("vec constructor expects scalar arguments"));
        }
        let val = Instance::Literal(*l);
        let comps = (0..n).map(|_| val.clone()).collect_vec();
        Ok(VecInstance::new(comps).into())
    }
    // overload 2: vec conversion constructor
    else if let [Instance::Vec(v)] = args {
        if v.n() != n {
            let ty = v.ty();
            let ty2 = Type::Vec(n as u8, ty.inner_ty().into());
            return Err(E::Conversion(ty, ty2));
        }
        // note: `vecN(e: vecN<S>) -> vecN<S>` is no-op
        Ok(v.clone().into())
    }
    // overload 3: vec init from component values
    else if !args.is_empty() {
        // flatten vecN args
        let args = args
            .iter()
            .flat_map(|a| -> Box<dyn Iterator<Item = &Instance>> {
                match a {
                    Instance::Vec(v) => Box::new(v.iter()),
                    _ => Box::new(std::iter::once(a)),
                }
            })
            .cloned()
            .collect_vec();
        if args.len() != n {
            return Err(E::ParamCount(format!("vec{n}"), n, args.len()));
        }

        let comps = convert_all(&args).ok_or(E::Builtin("vector components are incompatible"))?;

        if !comps.first().unwrap(/* SAFETY: len() checked above */).ty().is_scalar() {
            return Err(E::Builtin("vec constructor expects scalar arguments"));
        }
        Ok(VecInstance::new(comps).into())
    }
    // overload 3: zero-vec
    else {
        VecInstance::zero_value(n as u8, &Type::AbstractInt).map(Into::into)
    }
}

/// User-defined struct constructor.
pub fn struct_ctor(struct_ty: &StructType, args: &[Instance]) -> Result<StructInstance, E> {
    if args.is_empty() {
        return StructInstance::zero_value(struct_ty);
    }

    if args.len() != struct_ty.members.len() {
        return Err(E::ParamCount(
            struct_ty.name.clone(),
            struct_ty.members.len(),
            args.len(),
        ));
    }

    let members = struct_ty
        .members
        .iter()
        .zip(args)
        .map(|(m_ty, inst)| {
            let inst = inst
                .convert_to(&m_ty.ty)
                .ok_or_else(|| E::ParamType(m_ty.ty.clone(), inst.ty()))?;
            Ok(inst)
        })
        .collect::<Result<Vec<_>, E>>()?;

    Ok(StructInstance::new(struct_ty.clone(), members))
}

/// Check a struct constructor call signature.
///
/// Validates the type and number of arguments passed.
pub fn typecheck_struct_ctor(struct_ty: &StructType, args: &[Type]) -> Result<(), E> {
    if args.is_empty() {
        // zero-value constructor
        return Ok(());
    }

    if args.len() != struct_ty.members.len() {
        return Err(E::ParamCount(
            struct_ty.name.clone(),
            struct_ty.members.len(),
            args.len(),
        ));
    }

    for (m_ty, a_ty) in struct_ty.members.iter().zip(args) {
        if !a_ty.is_convertible_to(&m_ty.ty) {
            return Err(E::ParamType(m_ty.ty.clone(), a_ty.ty()));
        }
    }

    Ok(())
}

// -----------------
// CONSTRUCTOR TYPES
// -----------------

/// Return type of `array<T,N>()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#array-builtin>
fn array_ctor_ty_t(tplt_ty: &Type, tplt_n: usize, args: &[Type]) -> Result<Type, E> {
    if let Some(arg) = args.iter().find(|arg| !arg.is_convertible_to(tplt_ty)) {
        Err(E::Conversion(arg.clone(), tplt_ty.clone()))
    } else {
        Ok(Type::Array(Box::new(tplt_ty.clone()), Some(tplt_n)))
    }
}

/// Return type of `array()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#array-builtin>
fn array_ctor_ty(args: &[Type]) -> Result<Type, E> {
    let ty = convert_all_ty(args).ok_or(E::Builtin("array elements are incompatible"))?;
    Ok(Type::Array(Box::new(ty.clone()), Some(args.len())))
}

/// Return type of `matCxR<T>()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#mat2x2-builtin>
fn mat_ctor_ty_t(c: u8, r: u8, tplt_ty: &Type, args: &[Type]) -> Result<Type, E> {
    // overload 1: mat conversion constructor
    if let [ty @ Type::Mat(c2, r2, _)] = args {
        // note: this is an explicit conversion, not automatic conversion
        if *c2 != c || *r2 != r {
            return Err(E::Conversion(
                ty.clone(),
                Type::Mat(c, r, Box::new(tplt_ty.clone())),
            ));
        }
    } else {
        if args.is_empty() {
            return Err(E::Builtin("matrix constructor expects arguments"));
        }
        let ty = convert_all_ty(args).ok_or(E::Builtin("matrix components are incompatible"))?;
        let ty = ty
            .convert_inner_to(tplt_ty)
            .ok_or(E::Conversion(ty.inner_ty(), tplt_ty.clone()))?;

        // overload 2: mat from column vectors
        if ty.is_vec() {
            if args.len() != c as usize {
                return Err(E::ParamCount(format!("mat{c}x{r}"), c as usize, args.len()));
            }
        }
        // overload 3: mat from float values
        else if ty.is_float() {
            let n = c as usize * r as usize;
            if args.len() != n {
                return Err(E::ParamCount(format!("mat{c}x{r}"), n, args.len()));
            }
        } else {
            return Err(E::Builtin(
                "matrix constructor expects float or vector of float arguments",
            ));
        }
    }

    Ok(Type::Mat(c, r, Box::new(tplt_ty.clone())))
}

/// Return type of `matCxR()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#mat2x2-builtin>
fn mat_ctor_ty(c: u8, r: u8, args: &[Type]) -> Result<Type, E> {
    // overload 1: mat conversion constructor
    if let [ty @ Type::Mat(c2, r2, ty2)] = args {
        // note: this is an explicit conversion, not automatic conversion
        if *c2 != c || *r2 != r {
            return Err(E::Conversion(ty.clone(), Type::Mat(c, r, ty2.clone())));
        }
        Ok(ty.clone())
    } else {
        let ty = convert_all_ty(args).ok_or(E::Builtin("matrix components are incompatible"))?;
        let mut inner_ty = ty.inner_ty();

        if inner_ty.is_abstract_int() {
            // force conversion from AbstractInt to a float type
            inner_ty = Type::AbstractFloat;
        } else if !inner_ty.is_float() {
            return Err(E::Builtin(
                "matrix constructor expects float or vector of float arguments",
            ));
        }

        // overload 2: mat from column vectors
        if ty.is_vec() {
            if args.len() != c as usize {
                return Err(E::ParamCount(format!("mat{c}x{r}"), c as usize, args.len()));
            }
        }
        // overload 3: mat from float values
        else if ty.is_float() || ty.is_abstract_int() {
            let n = c as usize * r as usize;
            if args.len() != n {
                return Err(E::ParamCount(format!("mat{c}x{r}"), n, args.len()));
            }
        } else {
            return Err(E::Builtin(
                "matrix constructor expects float or vector of float arguments",
            ));
        }

        Ok(Type::Mat(c, r, inner_ty.into()))
    }
}

/// Return type of `vecN<T>()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#vec2-builtin>
fn vec_ctor_ty_t(n: u8, tplt_ty: &Type, args: &[Type]) -> Result<Type, E> {
    if let [arg] = args {
        // overload 1: vec init from single scalar value
        if arg.is_scalar() {
            if !arg.is_convertible_to(tplt_ty) {
                return Err(E::Conversion(arg.clone(), tplt_ty.clone()));
            }
        }
        // overload 2: vec conversion constructor
        else if arg.is_vec() {
            // note: this is an explicit conversion, not automatic conversion
        } else {
            return Err(E::Conversion(arg.clone(), tplt_ty.clone()));
        }
    }
    // overload 3: vec init from component values
    else {
        // flatten vecN args
        let n2 = args
            .iter()
            .try_fold(0, |acc, arg| match arg {
                ty if ty.is_scalar() => ty.is_convertible_to(tplt_ty).then_some(acc + 1),
                Type::Vec(n, ty) => ty.is_convertible_to(tplt_ty).then_some(acc + n),
                _ => None,
            })
            .ok_or(E::Builtin(
                "vector constructor expects scalar or vector arguments",
            ))?;
        if n2 != n {
            return Err(E::ParamCount(format!("vec{n}"), n as usize, args.len()));
        }
    }

    Ok(Type::Vec(n, Box::new(tplt_ty.clone())))
}

/// Return type of `vecN()` constructor.
///
/// Reference: <https://www.w3.org/TR/WGSL/#vec2-builtin>
fn vec_ctor_ty(n: u8, args: &[Type]) -> Result<Type, E> {
    if let [arg] = args {
        // overload 1: vec init from single scalar value
        if arg.is_scalar() {
        }
        // overload 2: vec conversion constructor
        else if arg.is_vec() {
            // note: `vecN(e: vecN<S>) -> vecN<S>` is no-op
        } else {
            return Err(E::Builtin(
                "vector constructor expects scalar or vector arguments",
            ));
        }
        Ok(Type::Vec(n, arg.inner_ty().into()))
    }
    // overload 3: vec init from component values
    else if !args.is_empty() {
        // flatten vecN args
        let n2 = args
            .iter()
            .try_fold(0, |acc, arg| match arg {
                ty if ty.is_scalar() => Some(acc + 1),
                Type::Vec(n, _) => Some(acc + n),
                _ => None,
            })
            .ok_or(E::Builtin(
                "vector constructor expects scalar or vector arguments",
            ))?;
        if n2 != n {
            return Err(E::ParamCount(format!("vec{n}"), n as usize, args.len()));
        }

        let tys = args.iter().map(|arg| arg.inner_ty()).collect_vec();
        let ty = convert_all_ty(&tys).ok_or(E::Builtin("vector components are incompatible"))?;

        Ok(Type::Vec(n, ty.clone().into()))
    }
    // overload 3: zero-vec
    else {
        Ok(Type::Vec(n, Type::AbstractInt.into()))
    }
}

/// Compute the return type of calling a built-in constructor function.
///
/// The arguments must be [loaded][Type::loaded].
///
/// Includes built-in constructors and zero-value constructors, *but not* the struct
/// constructors, since they require knowledge of the struct type.
/// You can type-check a struct constructor call with [`typecheck_struct_ctor`].
pub fn type_ctor(name: &str, tplt: Option<&[TpltParam]>, args: &[Type]) -> Result<Type, E> {
    match (name, tplt, args) {
        ("array", Some(t), []) => Ok(ArrayTemplate::parse(t)?.ty()),
        ("array", Some(t), a) => {
            let tplt = ArrayTemplate::parse(t)?;
            array_ctor_ty_t(
                &tplt.inner_ty(),
                tplt.n().ok_or(E::TemplateArgs("array"))?,
                a,
            )
        }
        ("array", None, _) => array_ctor_ty(args),
        ("bool", None, []) => Ok(Type::Bool),
        ("bool", None, [a]) if a.is_scalar() => Ok(Type::Bool),
        ("i32", None, []) => Ok(Type::I32),
        ("i32", None, [a]) if a.is_scalar() => Ok(Type::I32),
        ("u32", None, []) => Ok(Type::U32),
        ("u32", None, [a]) if a.is_scalar() => Ok(Type::U32),
        ("f32", None, []) => Ok(Type::F32),
        ("f32", None, [a]) if a.is_scalar() => Ok(Type::F32),
        ("f16", None, []) => Ok(Type::F16),
        ("f16", None, [a]) if a.is_scalar() => Ok(Type::F16),
        ("mat2x2", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(2, 2)),
        ("mat2x2", Some(t), _) => mat_ctor_ty_t(2, 2, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat2x2", None, _) => mat_ctor_ty(2, 2, args),
        ("mat2x3", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(2, 3)),
        ("mat2x3", Some(t), _) => mat_ctor_ty_t(2, 3, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat2x3", None, _) => mat_ctor_ty(2, 3, args),
        ("mat2x4", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(2, 4)),
        ("mat2x4", Some(t), _) => mat_ctor_ty_t(2, 4, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat2x4", None, _) => mat_ctor_ty(2, 4, args),
        ("mat3x2", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(3, 2)),
        ("mat3x2", Some(t), _) => mat_ctor_ty_t(3, 2, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat3x2", None, _) => mat_ctor_ty(3, 2, args),
        ("mat3x3", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(3, 3)),
        ("mat3x3", Some(t), _) => mat_ctor_ty_t(3, 3, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat3x3", None, _) => mat_ctor_ty(3, 3, args),
        ("mat3x4", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(3, 4)),
        ("mat3x4", Some(t), _) => mat_ctor_ty_t(3, 4, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat3x4", None, _) => mat_ctor_ty(3, 4, args),
        ("mat4x2", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(4, 2)),
        ("mat4x2", Some(t), _) => mat_ctor_ty_t(4, 2, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat4x2", None, _) => mat_ctor_ty(4, 2, args),
        ("mat4x3", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(4, 3)),
        ("mat4x3", Some(t), _) => mat_ctor_ty_t(4, 3, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat4x3", None, _) => mat_ctor_ty(4, 3, args),
        ("mat4x4", Some(t), []) => Ok(MatTemplate::parse(t)?.ty(4, 4)),
        ("mat4x4", Some(t), _) => mat_ctor_ty_t(4, 4, MatTemplate::parse(t)?.inner_ty(), args),
        ("mat4x4", None, _) => mat_ctor_ty(4, 4, args),
        ("vec2", Some(t), []) => Ok(VecTemplate::parse(t)?.ty(2)),
        ("vec2", Some(t), _) => vec_ctor_ty_t(2, VecTemplate::parse(t)?.inner_ty(), args),
        ("vec2", None, _) => vec_ctor_ty(2, args),
        ("vec3", Some(t), []) => Ok(VecTemplate::parse(t)?.ty(3)),
        ("vec3", Some(t), _) => vec_ctor_ty_t(3, VecTemplate::parse(t)?.inner_ty(), args),
        ("vec3", None, _) => vec_ctor_ty(3, args),
        ("vec4", Some(t), []) => Ok(VecTemplate::parse(t)?.ty(4)),
        ("vec4", Some(t), _) => vec_ctor_ty_t(4, VecTemplate::parse(t)?.inner_ty(), args),
        ("vec4", None, _) => vec_ctor_ty(4, args),
        #[cfg(feature = "naga-ext")]
        ("i64", None, []) => Ok(Type::I64),
        #[cfg(feature = "naga-ext")]
        ("i64", None, [a]) if a.is_scalar() => Ok(Type::I64),
        #[cfg(feature = "naga-ext")]
        ("u64", None, []) => Ok(Type::U64),
        #[cfg(feature = "naga-ext")]
        ("u64", None, [a]) if a.is_scalar() => Ok(Type::U64),
        #[cfg(feature = "naga-ext")]
        ("f64", None, []) => Ok(Type::F64),
        #[cfg(feature = "naga-ext")]
        ("f64", None, [a]) if a.is_scalar() => Ok(Type::F64),
        _ => Err(E::Signature(CallSignature {
            name: name.to_string(),
            tplt: tplt.map(|t| t.to_vec()),
            args: args.to_vec(),
        })),
    }
}

// -----------
// ZERO VALUES
// -----------
// reference: <https://www.w3.org/TR/WGSL/#zero-value>

impl Instance {
    /// Zero-value initialize an instance of a given type.
    pub fn zero_value(ty: &Type) -> Result<Self, E> {
        match ty {
            Type::Bool => Ok(LiteralInstance::Bool(false).into()),
            Type::AbstractInt => Ok(LiteralInstance::AbstractInt(0).into()),
            Type::AbstractFloat => Ok(LiteralInstance::AbstractFloat(0.0).into()),
            Type::I32 => Ok(LiteralInstance::I32(0).into()),
            Type::U32 => Ok(LiteralInstance::U32(0).into()),
            Type::F32 => Ok(LiteralInstance::F32(0.0).into()),
            Type::F16 => Ok(LiteralInstance::F16(f16::zero()).into()),
            Type::Struct(s) => StructInstance::zero_value(s).map(Into::into),
            Type::Array(a_ty, Some(n)) => ArrayInstance::zero_value(*n, a_ty).map(Into::into),
            Type::Array(_, None) => Err(E::NotConstructible(ty.clone())),
            Type::Vec(n, v_ty) => VecInstance::zero_value(*n, v_ty).map(Into::into),
            Type::Mat(c, r, m_ty) => MatInstance::zero_value(*c, *r, m_ty).map(Into::into),
            Type::Atomic(_)
            | Type::Ptr(_, _, _)
            | Type::Ref(_, _, _)
            | Type::Texture(_)
            | Type::Sampler(_) => Err(E::NotConstructible(ty.clone())),
            #[cfg(feature = "naga-ext")]
            Type::I64 => Ok(LiteralInstance::I64(0).into()),
            #[cfg(feature = "naga-ext")]
            Type::U64 => Ok(LiteralInstance::U64(0).into()),
            #[cfg(feature = "naga-ext")]
            Type::F64 => Ok(LiteralInstance::F64(0.0).into()),
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(_, _) => Err(E::NotConstructible(ty.clone())),
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(_) => Err(E::NotConstructible(ty.clone())),
            #[cfg(feature = "naga-ext")]
            Type::AccelerationStructure(_) => Err(E::NotConstructible(ty.clone())),
        }
    }
}

impl LiteralInstance {
    /// The zero-value constructor.
    pub fn zero_value(ty: &Type) -> Result<Self, E> {
        match ty {
            Type::Bool => Ok(LiteralInstance::Bool(false)),
            Type::AbstractInt => Ok(LiteralInstance::AbstractInt(0)),
            Type::AbstractFloat => Ok(LiteralInstance::AbstractFloat(0.0)),
            Type::I32 => Ok(LiteralInstance::I32(0)),
            Type::U32 => Ok(LiteralInstance::U32(0)),
            Type::F32 => Ok(LiteralInstance::F32(0.0)),
            Type::F16 => Ok(LiteralInstance::F16(f16::zero())),
            #[cfg(feature = "naga-ext")]
            Type::I64 => Ok(LiteralInstance::I64(0)),
            #[cfg(feature = "naga-ext")]
            Type::U64 => Ok(LiteralInstance::U64(0)),
            #[cfg(feature = "naga-ext")]
            Type::F64 => Ok(LiteralInstance::F64(0.0)),
            _ => Err(E::NotScalar(ty.clone())),
        }
    }
}

impl StructInstance {
    /// Zero-value initialize a `struct` instance.
    pub fn zero_value(s: &StructType) -> Result<Self, E> {
        let members = s
            .members
            .iter()
            .map(|mem| {
                let val = Instance::zero_value(&mem.ty)?;
                Ok(val)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(StructInstance::new(s.clone(), members))
    }
}

impl ArrayInstance {
    /// Zero-value initialize an `array` instance.
    pub fn zero_value(n: usize, ty: &Type) -> Result<Self, E> {
        let zero = Instance::zero_value(ty)?;
        let comps = (0..n).map(|_| zero.clone()).collect_vec();
        Ok(ArrayInstance::new(comps, false))
    }
}

impl VecInstance {
    /// Zero-value initialize a `vec` instance.
    pub fn zero_value(n: u8, ty: &Type) -> Result<Self, E> {
        let zero = Instance::Literal(LiteralInstance::zero_value(ty)?);
        let comps = (0..n).map(|_| zero.clone()).collect_vec();
        Ok(VecInstance::new(comps))
    }
}

impl MatInstance {
    /// Zero-value initialize a `mat` instance.
    pub fn zero_value(c: u8, r: u8, ty: &Type) -> Result<Self, E> {
        let zero = Instance::Literal(LiteralInstance::zero_value(ty)?);
        let zero_col = Instance::Vec(VecInstance::new((0..r).map(|_| zero.clone()).collect_vec()));
        let comps = (0..c).map(|_| zero_col.clone()).collect_vec();
        Ok(MatInstance::from_cols(comps))
    }
}
