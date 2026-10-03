use std::iter::zip;

use crate::{
    Error, Instance, ShaderStage,
    conv::{Convert, convert, convert_inner},
    inst::{LiteralInstance, MatInstance, MemView, PtrInstance, RefInstance, VecInstance},
    syntax::{AddressSpace, BinaryOperator, UnaryOperator},
    ty::{Ty, Type},
    ty_ctx::TyContext,
};

use num_traits::{WrappingNeg, WrappingShl};

type E = Error;

pub trait Compwise: Clone + Sized {
    fn compwise_unary_mut<F>(&mut self, f: F) -> Result<(), E>
    where
        F: Fn(&LiteralInstance) -> Result<LiteralInstance, E>;
    fn compwise_binary_mut<F>(&mut self, rhs: &Self, f: F) -> Result<(), E>
    where
        F: Fn(&LiteralInstance, &LiteralInstance) -> Result<LiteralInstance, E>;
    fn compwise_unary<F>(&self, f: F) -> Result<Self, E>
    where
        F: Fn(&LiteralInstance) -> Result<LiteralInstance, E>,
    {
        let mut res = self.clone();
        res.compwise_unary_mut(f)?;
        Ok(res)
    }

    fn compwise_binary<F>(&self, rhs: &Self, f: F) -> Result<Self, E>
    where
        F: Fn(&LiteralInstance, &LiteralInstance) -> Result<LiteralInstance, E>,
    {
        let mut res = self.clone();
        res.compwise_binary_mut(rhs, f)?;
        Ok(res)
    }
}

impl Compwise for VecInstance {
    fn compwise_unary_mut<F>(&mut self, f: F) -> Result<(), E>
    where
        F: Fn(&LiteralInstance) -> Result<LiteralInstance, E>,
    {
        self.iter_mut().try_for_each(|c| {
            match c {
                Instance::Literal(c) => *c = f(c)?,
                _ => unreachable!("vec must contain literal instances"),
            };
            Ok(())
        })
    }

    fn compwise_binary_mut<F>(&mut self, rhs: &Self, f: F) -> Result<(), E>
    where
        F: Fn(&LiteralInstance, &LiteralInstance) -> Result<LiteralInstance, E>,
    {
        if self.n() != rhs.n() {
            return Err(E::CompwiseBinary(self.ty(), rhs.ty()));
        }
        zip(self.iter_mut(), rhs.iter()).try_for_each(|(a, b)| {
            match (a, b) {
                (Instance::Literal(a), Instance::Literal(b)) => *a = f(a, b)?,
                _ => unreachable!("vec must contain literal instances"),
            };
            Ok(())
        })
    }
}

impl Compwise for MatInstance {
    fn compwise_unary_mut<F>(&mut self, f: F) -> Result<(), E>
    where
        F: Fn(&LiteralInstance) -> Result<LiteralInstance, E>,
    {
        self.iter_cols_mut()
            .flat_map(|col| col.unwrap_vec_mut().iter_mut())
            .try_for_each(|c| {
                match c {
                    Instance::Literal(c) => *c = f(c)?,
                    _ => unreachable!("mat must contain literal instances"),
                };
                Ok(())
            })
    }

    fn compwise_binary_mut<F>(&mut self, rhs: &Self, f: F) -> Result<(), E>
    where
        F: Fn(&LiteralInstance, &LiteralInstance) -> Result<LiteralInstance, E>,
    {
        if self.c() != rhs.c() || self.r() != rhs.r() {
            return Err(E::CompwiseBinary(self.ty(), rhs.ty()));
        }
        zip(
            self.iter_cols_mut()
                .flat_map(|col| col.unwrap_vec_mut().iter_mut()),
            rhs.iter_cols().flat_map(|col| col.unwrap_vec_ref().iter()),
        )
        .try_for_each(|(a, b)| {
            match (a, b) {
                (Instance::Literal(a), Instance::Literal(b)) => *a = f(a, b)?,
                _ => unreachable!("mat must contain literal instances"),
            };
            Ok(())
        })
    }
}

macro_rules! both {
    ($enum:ident::$var:ident, $lhs:ident, $rhs:ident) => {
        ($enum::$var($lhs), $enum::$var($rhs))
    };
    ($enum1:ident::$var1:ident, $enum2:ident::$var2:ident, $lhs:ident, $rhs:ident) => {
        ($enum1::$var1($lhs), $enum2::$var2($rhs)) | ($enum2::$var2($rhs), $enum1::$var1($lhs))
    };
}

// -------------------
// LOGICAL EXPRESSIONS
// -------------------
// reference: https://www.w3.org/TR/WGSL/#logical-expr

// logical and/or are part of bitwise and/or.
// short circuiting and/or is implemented in eval() because it needs context.

impl LiteralInstance {
    pub fn op_not(&self) -> Result<Self, E> {
        match self {
            Self::Bool(b) => Ok(Self::Bool(!b)),
            _ => Err(E::Unary(UnaryOperator::LogicalNegation, self.ty())),
        }
    }
}

impl VecInstance {
    pub fn op_not(&self) -> Result<Self, E> {
        self.compwise_unary(|c| c.op_not())
    }
}

impl Instance {
    pub fn op_not(&self) -> Result<Self, E> {
        match self {
            Self::Literal(lit) => lit.op_not().map(Into::into),
            Self::Vec(v) => v.op_not().map(Into::into),
            _ => Err(E::Unary(UnaryOperator::LogicalNegation, self.ty())),
        }
    }
}

// ----------------------
// ARITHMETIC EXPRESSIONS
// ----------------------
// reference: https://www.w3.org/TR/WGSL/#arithmetic-expr

impl LiteralInstance {
    pub fn op_neg(&self) -> Result<Self, E> {
        match self {
            Self::AbstractInt(lhs) => Ok(lhs.wrapping_neg().into()),
            Self::AbstractFloat(lhs) => Ok((-lhs).into()),
            Self::I32(lhs) => Ok(lhs.wrapping_neg().into()),
            Self::F32(lhs) => Ok((-lhs).into()),
            Self::F16(lhs) => Ok((-lhs).into()),
            #[cfg(feature = "naga-ext")]
            Self::I64(lhs) => Ok(LiteralInstance::I64(lhs.wrapping_neg())),
            #[cfg(feature = "naga-ext")]
            Self::F64(lhs) => Ok(LiteralInstance::F64(-lhs)),
            _ => Err(E::Unary(UnaryOperator::Negation, self.ty())),
        }
    }
    pub fn op_or(&self, rhs: &Self) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::ShortCircuitOr, self.ty(), rhs.ty());
        match (self, rhs) {
            (Self::Bool(b1), Self::Bool(b2)) => Ok(Self::Bool(*b1 || *b2)),
            _ => Err(err()),
        }
    }
    pub fn op_and(&self, rhs: &Self) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::ShortCircuitAnd, self.ty(), rhs.ty());
        match (self, rhs) {
            (Self::Bool(b1), Self::Bool(b2)) => Ok(Self::Bool(*b1 && *b2)),
            _ => Err(err()),
        }
    }
    pub fn op_add(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Addition, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::AbstractInt, lhs, rhs) => {
                lhs.checked_add(rhs).ok_or(E::AddOverflow).map(Into::into)
            }
            both!(Self::AbstractFloat, lhs, rhs) => {
                let res = lhs + rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::AddOverflow)
                    .map(Into::into)
            }
            both!(Self::I32, lhs, rhs) => Ok(lhs.wrapping_add(rhs).into()),
            both!(Self::U32, lhs, rhs) => Ok(lhs.wrapping_add(rhs).into()),
            both!(Self::F32, lhs, rhs) => {
                let res = lhs + rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::AddOverflow)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            both!(Self::F16, lhs, rhs) => {
                let res = lhs + rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::AddOverflow)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(Self::I64(lhs.wrapping_add(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(Self::U64(lhs.wrapping_add(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => {
                let res = lhs + rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::AddOverflow)
                        .map(LiteralInstance::F64)
                } else {
                    Ok(res.into())
                }
            }
            _ => Err(err()),
        }
    }
    pub fn op_sub(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Subtraction, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::AbstractInt, lhs, rhs) => {
                lhs.checked_sub(rhs).ok_or(E::SubOverflow).map(Into::into)
            }
            both!(Self::AbstractFloat, lhs, rhs) => {
                let res = lhs - rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::SubOverflow)
                    .map(Into::into)
            }
            both!(Self::I32, lhs, rhs) => Ok(lhs.wrapping_sub(rhs).into()),
            both!(Self::U32, lhs, rhs) => Ok(lhs.wrapping_sub(rhs).into()),
            both!(Self::F32, lhs, rhs) => {
                let res = lhs - rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::SubOverflow)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            both!(Self::F16, lhs, rhs) => {
                let res = lhs - rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::SubOverflow)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(Self::I64(lhs.wrapping_sub(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(Self::U64(lhs.wrapping_sub(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => {
                let res = lhs - rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::SubOverflow)
                        .map(LiteralInstance::F64)
                } else {
                    Ok(res.into())
                }
            }
            _ => Err(err()),
        }
    }
    pub fn op_mul(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::AbstractInt, lhs, rhs) => {
                lhs.checked_mul(rhs).ok_or(E::MulOverflow).map(Into::into)
            }
            both!(Self::AbstractFloat, lhs, rhs) => {
                let res = lhs * rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::MulOverflow)
                    .map(Into::into)
            }
            both!(Self::I32, lhs, rhs) => Ok(lhs.wrapping_mul(rhs).into()),
            both!(Self::U32, lhs, rhs) => Ok(lhs.wrapping_mul(rhs).into()),
            both!(Self::F32, lhs, rhs) => {
                let res = lhs * rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::MulOverflow)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            both!(Self::F16, lhs, rhs) => {
                let res = lhs * rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::MulOverflow)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(Self::I64(lhs.wrapping_mul(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(Self::U64(lhs.wrapping_mul(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => {
                let res = lhs * rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::MulOverflow)
                        .map(LiteralInstance::F64)
                } else {
                    Ok(res.into())
                }
            }
            _ => Err(err()),
        }
    }
    pub fn op_div(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Division, self.ty(), rhs.ty());
        let res = match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::AbstractInt, lhs, rhs) => {
                lhs.checked_div(rhs).ok_or(E::DivByZero).map(Into::into)
            }
            both!(Self::AbstractFloat, lhs, rhs) => {
                let res = lhs / rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::DivByZero)
                    .map(Into::into)
            }
            both!(Self::I32, lhs, rhs) => lhs.checked_div(rhs).ok_or(E::DivByZero).map(Into::into),
            both!(Self::U32, lhs, rhs) => lhs.checked_div(rhs).ok_or(E::DivByZero).map(Into::into),
            both!(Self::F32, lhs, rhs) => {
                let res = lhs / rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::DivByZero)
                    .map(Into::into)
            }
            both!(Self::F16, lhs, rhs) => {
                let res = lhs / rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::DivByZero)
                    .map(Into::into)
            }
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(Self::I64(lhs.wrapping_div(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(Self::U64(lhs.wrapping_div(rhs))),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => {
                let res = lhs / rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::DivByZero)
                    .map(Self::F64)
            }
            _ => Err(err()),
        };
        if stage == ShaderStage::Exec {
            // runtime expressions return lhs when operation fails
            Ok(res.unwrap_or(*self))
        } else {
            res
        }
    }
    pub fn op_rem(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Remainder, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::AbstractInt, lhs, rhs) => {
                if stage == ShaderStage::Const {
                    lhs.checked_rem(rhs).ok_or(E::RemZeroDiv).map(Into::into)
                } else {
                    Ok(lhs.checked_rem(rhs).unwrap_or(0).into())
                }
            }
            both!(Self::AbstractFloat, lhs, rhs) => {
                let res = lhs % rhs;
                res.is_finite()
                    .then_some(res)
                    .ok_or(E::RemZeroDiv)
                    .map(Into::into)
            }
            both!(Self::I32, lhs, rhs) => {
                if stage == ShaderStage::Const {
                    lhs.checked_rem(rhs).ok_or(E::RemZeroDiv).map(Into::into)
                } else {
                    Ok(lhs.checked_rem(rhs).unwrap_or(0).into())
                }
            }
            both!(Self::U32, lhs, rhs) => {
                if stage == ShaderStage::Const {
                    lhs.checked_rem(rhs).ok_or(E::RemZeroDiv).map(Into::into)
                } else {
                    Ok(lhs.checked_rem(rhs).unwrap_or(0).into())
                }
            }
            both!(Self::F32, lhs, rhs) => {
                let res = lhs % rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::RemZeroDiv)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            both!(Self::F16, lhs, rhs) => {
                let res = lhs % rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::RemZeroDiv)
                        .map(Into::into)
                } else {
                    Ok(res.into())
                }
            }
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => {
                if stage == ShaderStage::Const {
                    lhs.checked_rem(rhs).ok_or(E::RemZeroDiv).map(Self::I64)
                } else {
                    Ok(Self::I64(lhs.checked_rem(rhs).unwrap_or(0)))
                }
            }
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => {
                if stage == ShaderStage::Const {
                    lhs.checked_rem(rhs).ok_or(E::RemZeroDiv).map(Self::U64)
                } else {
                    Ok(Self::U64(lhs.checked_rem(rhs).unwrap_or(0)))
                }
            }
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => {
                let res = lhs % rhs;
                if stage == ShaderStage::Const {
                    res.is_finite()
                        .then_some(res)
                        .ok_or(E::RemZeroDiv)
                        .map(Self::F64)
                } else {
                    Ok(Self::F64(res))
                }
            }
            _ => Err(err()),
        }
    }
    pub fn op_add_vec(
        &self,
        rhs: &VecInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<VecInstance, E> {
        rhs.op_add_sca(self, stage, context)
    }
    pub fn op_sub_vec(
        &self,
        rhs: &VecInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<VecInstance, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Subtraction, self.ty(), rhs.ty()))?;
        rhs.compwise_unary(|r| lhs.op_sub(r, stage, context))
    }
    pub fn op_mul_vec(
        &self,
        rhs: &VecInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<VecInstance, E> {
        rhs.op_mul_sca(self, stage, context)
    }
    pub fn op_div_vec(
        &self,
        rhs: &VecInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<VecInstance, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Division, self.ty(), rhs.ty()))?;
        rhs.compwise_unary(|r| lhs.op_div(r, stage, context))
    }
    pub fn op_rem_vec(
        &self,
        rhs: &VecInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<VecInstance, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Remainder, self.ty(), rhs.ty()))?;
        rhs.compwise_unary(|r| lhs.op_rem(r, stage, context))
    }
    pub fn op_mul_mat(
        &self,
        rhs: &MatInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<MatInstance, E> {
        rhs.op_mul_sca(self, stage, context)
    }
}

impl VecInstance {
    pub fn op_neg(&self) -> Result<Self, E> {
        self.compwise_unary(|c| c.op_neg())
    }
    pub fn op_add(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Addition, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_add(r, stage, context))
    }
    pub fn op_sub(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Subtraction, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_sub(r, stage, context))
    }
    pub fn op_mul(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_mul(r, stage, context))
    }
    pub fn op_div(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Division, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_div(r, stage, context))
    }
    pub fn op_rem(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Remainder, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_rem(r, stage, context))
    }
    pub fn op_add_sca(
        &self,
        rhs: &LiteralInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<Self, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Addition, self.ty(), rhs.ty()))?;
        lhs.compwise_unary(|l| l.op_add(&rhs, stage, context))
    }
    pub fn op_sub_sca(
        &self,
        rhs: &LiteralInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<Self, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Subtraction, self.ty(), rhs.ty()))?;
        lhs.compwise_unary(|l| l.op_sub(&rhs, stage, context))
    }
    pub fn op_mul_sca(
        &self,
        rhs: &LiteralInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<Self, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty()))?;
        lhs.compwise_unary(|l| l.op_mul(&rhs, stage, context))
    }
    pub fn op_div_sca(
        &self,
        rhs: &LiteralInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<Self, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Division, self.ty(), rhs.ty()))?;
        lhs.compwise_unary(|l| l.op_div(&rhs, stage, context))
    }
    pub fn op_rem_sca(
        &self,
        rhs: &LiteralInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<Self, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Remainder, self.ty(), rhs.ty()))?;
        lhs.compwise_unary(|l| l.op_rem(&rhs, stage, context))
    }
    pub fn op_mul_mat(
        &self,
        rhs: &MatInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<Self, E> {
        let (vec, mat) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty()))?;
        let mat = mat.transpose();

        zip(vec.iter(), mat.iter_cols())
            .map(|(s, v)| {
                v.unwrap_vec_ref()
                    .op_mul_sca(s.unwrap_literal_ref(), stage, context)
            })
            .reduce(|a, b| a?.op_add(&b?, stage, context))
            .unwrap()
    }
}

impl MatInstance {
    pub fn op_add(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Addition, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_add(r, stage, context))
    }

    pub fn op_sub(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Subtraction, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_sub(r, stage, context))
    }

    pub fn op_mul_sca(
        &self,
        rhs: &LiteralInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<Self, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty()))?;
        lhs.compwise_unary(|l| l.op_mul(&rhs, stage, context))
    }

    pub fn op_mul_vec(
        &self,
        rhs: &VecInstance,
        stage: ShaderStage,
        context: &TyContext,
    ) -> Result<VecInstance, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty()))?;

        zip(lhs.iter_cols(), rhs.iter())
            .map(|(l, r)| {
                l.unwrap_vec_ref()
                    .op_mul_sca(r.unwrap_literal_ref(), stage, context)
            })
            .reduce(|l, r| l?.op_add(&r?, stage, context))
            .unwrap()
    }

    pub fn op_mul(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert_inner(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty()))?;
        let lhs = lhs.transpose();

        Ok(MatInstance::from_cols(
            rhs.iter_cols()
                .map(|col| {
                    Ok(VecInstance::new(
                        lhs.iter_cols()
                            .map(|r| {
                                col.unwrap_vec_ref()
                                    .dot(r.unwrap_vec_ref(), stage, context)
                                    .map(Into::into)
                            })
                            .collect::<Result<_, _>>()?,
                    )
                    .into())
                })
                .collect::<Result<_, _>>()?,
        ))
    }
}

impl Instance {
    pub fn op_neg(&self) -> Result<Self, E> {
        match self {
            Self::Literal(lhs) => lhs.op_neg().map(Into::into),
            Self::Vec(lhs) => lhs.op_neg().map(Into::into),
            _ => Err(E::Unary(UnaryOperator::Negation, self.ty())),
        }
    }
    pub fn op_or(&self, rhs: &Self) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::ShortCircuitOr, self.ty(), rhs.ty());
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => (lhs.op_or(rhs)).map(Into::into),
            _ => Err(err()),
        }
    }
    pub fn op_and(&self, rhs: &Self) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::ShortCircuitAnd, self.ty(), rhs.ty());
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => (lhs.op_and(rhs)).map(Into::into),
            _ => Err(err()),
        }
    }
    pub fn op_add(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => (lhs.op_add(rhs, stage, context)).map(Into::into),
            (Self::Vec(lhs), Self::Literal(rhs)) => {
                lhs.op_add_sca(rhs, stage, context).map(Into::into)
            }
            (Self::Literal(lhs), Self::Vec(rhs)) => {
                lhs.op_add_vec(rhs, stage, context).map(Into::into)
            }
            both!(Self::Vec, lhs, rhs) => lhs.op_add(rhs, stage, context).map(Into::into),
            both!(Self::Mat, lhs, rhs) => lhs.op_add(rhs, stage, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::Addition, self.ty(), rhs.ty())),
        }
    }
    pub fn op_sub(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_sub(rhs, stage, context).map(Into::into),
            (Self::Vec(lhs), Self::Literal(rhs)) => {
                lhs.op_sub_sca(rhs, stage, context).map(Into::into)
            }
            (Self::Literal(lhs), Self::Vec(rhs)) => {
                lhs.op_sub_vec(rhs, stage, context).map(Into::into)
            }
            both!(Self::Vec, lhs, rhs) => lhs.op_sub(rhs, stage, context).map(Into::into),
            both!(Self::Mat, lhs, rhs) => lhs.op_sub(rhs, stage, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::Subtraction, self.ty(), rhs.ty())),
        }
    }
    pub fn op_mul(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_mul(rhs, stage, context).map(Into::into),
            (Self::Vec(lhs), Self::Literal(rhs)) => {
                lhs.op_mul_sca(rhs, stage, context).map(Into::into)
            }
            (Self::Literal(lhs), Self::Vec(rhs)) => {
                lhs.op_mul_vec(rhs, stage, context).map(Into::into)
            }
            both!(Self::Vec, lhs, rhs) => lhs.op_mul(rhs, stage, context).map(Into::into),
            (Self::Mat(lhs), Self::Literal(rhs)) => {
                lhs.op_mul_sca(rhs, stage, context).map(Into::into)
            }
            (Self::Literal(lhs), Self::Mat(rhs)) => {
                lhs.op_mul_mat(rhs, stage, context).map(Into::into)
            }
            (Self::Mat(lhs), Self::Vec(rhs)) => lhs.op_mul_vec(rhs, stage, context).map(Into::into),
            (Self::Vec(lhs), Self::Mat(rhs)) => lhs.op_mul_mat(rhs, stage, context).map(Into::into),
            both!(Self::Mat, lhs, rhs) => lhs.op_mul(rhs, stage, context).map(Into::into),
            _ => Err(E::Binary(
                BinaryOperator::Multiplication,
                self.ty(),
                rhs.ty(),
            )),
        }
    }
    pub fn op_div(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_div(rhs, stage, context).map(Into::into),
            (Self::Literal(s), Self::Vec(v)) => v
                .compwise_unary(|k| s.op_div(k, stage, context))
                .map(Into::into),
            (Self::Vec(v), Self::Literal(s)) => v
                .compwise_unary(|k| k.op_div(s, stage, context))
                .map(Into::into),
            both!(Self::Vec, lhs, rhs) => lhs.op_div(rhs, stage, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::Division, self.ty(), rhs.ty())),
        }
    }
    pub fn op_rem(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_rem(rhs, stage, context).map(Into::into),
            (Self::Literal(s), Self::Vec(v)) => v
                .compwise_unary(|k| s.op_rem(k, stage, context))
                .map(Into::into),
            (Self::Vec(v), Self::Literal(s)) => v
                .compwise_unary(|k| k.op_rem(s, stage, context))
                .map(Into::into),
            both!(Self::Vec, lhs, rhs) => lhs.op_rem(rhs, stage, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::Remainder, self.ty(), rhs.ty())),
        }
    }
}

// ----------------------
// COMPARISON EXPRESSIONS
// ----------------------
// reference: https://www.w3.org/TR/WGSL/#comparison-expr

impl LiteralInstance {
    pub fn op_eq(&self, rhs: &Self, context: &TyContext) -> Result<bool, E> {
        let err = || E::Binary(BinaryOperator::Equality, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, lhs, rhs) => Ok(lhs == rhs),
            both!(Self::AbstractInt, lhs, rhs) => Ok(lhs == rhs),
            both!(Self::AbstractFloat, lhs, rhs) => Ok(lhs == rhs),
            both!(Self::I32, lhs, rhs) => Ok(lhs == rhs),
            both!(Self::U32, lhs, rhs) => Ok(lhs == rhs),
            both!(Self::F32, lhs, rhs) => Ok(lhs == rhs),
            both!(Self::F16, lhs, rhs) => Ok(lhs == rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(lhs == rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(lhs == rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => Ok(lhs == rhs),
            _ => Err(err()),
        }
    }
    pub fn op_ne(&self, rhs: &Self, context: &TyContext) -> Result<bool, E> {
        let err = || E::Binary(BinaryOperator::Inequality, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, lhs, rhs) => Ok(lhs != rhs),
            both!(Self::AbstractInt, lhs, rhs) => Ok(lhs != rhs),
            both!(Self::AbstractFloat, lhs, rhs) => Ok(lhs != rhs),
            both!(Self::I32, lhs, rhs) => Ok(lhs != rhs),
            both!(Self::U32, lhs, rhs) => Ok(lhs != rhs),
            both!(Self::F32, lhs, rhs) => Ok(lhs != rhs),
            both!(Self::F16, lhs, rhs) => Ok(lhs != rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(lhs != rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(lhs != rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => Ok(lhs != rhs),
            _ => Err(err()),
        }
    }
    pub fn op_lt(&self, rhs: &Self, context: &TyContext) -> Result<bool, E> {
        let err = || E::Binary(BinaryOperator::LessThan, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, lhs, rhs) => Ok(!lhs & rhs),
            both!(Self::AbstractInt, lhs, rhs) => Ok(lhs < rhs),
            both!(Self::AbstractFloat, lhs, rhs) => Ok(lhs < rhs),
            both!(Self::I32, lhs, rhs) => Ok(lhs < rhs),
            both!(Self::U32, lhs, rhs) => Ok(lhs < rhs),
            both!(Self::F32, lhs, rhs) => Ok(lhs < rhs),
            both!(Self::F16, lhs, rhs) => Ok(lhs < rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(lhs < rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(lhs < rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => Ok(lhs < rhs),
            _ => Err(err()),
        }
    }
    pub fn op_le(&self, rhs: &Self, context: &TyContext) -> Result<bool, E> {
        let err = || E::Binary(BinaryOperator::LessThanEqual, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, lhs, rhs) => Ok(lhs <= rhs),
            both!(Self::AbstractInt, lhs, rhs) => Ok(lhs <= rhs),
            both!(Self::AbstractFloat, lhs, rhs) => Ok(lhs <= rhs),
            both!(Self::I32, lhs, rhs) => Ok(lhs <= rhs),
            both!(Self::U32, lhs, rhs) => Ok(lhs <= rhs),
            both!(Self::F32, lhs, rhs) => Ok(lhs <= rhs),
            both!(Self::F16, lhs, rhs) => Ok(lhs <= rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(lhs <= rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(lhs <= rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => Ok(lhs <= rhs),
            _ => Err(err()),
        }
    }
    pub fn op_gt(&self, rhs: &Self, context: &TyContext) -> Result<bool, E> {
        let err = || E::Binary(BinaryOperator::GreaterThan, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, lhs, rhs) => Ok(lhs & !rhs),
            both!(Self::AbstractInt, lhs, rhs) => Ok(lhs > rhs),
            both!(Self::AbstractFloat, lhs, rhs) => Ok(lhs > rhs),
            both!(Self::I32, lhs, rhs) => Ok(lhs > rhs),
            both!(Self::U32, lhs, rhs) => Ok(lhs > rhs),
            both!(Self::F32, lhs, rhs) => Ok(lhs > rhs),
            both!(Self::F16, lhs, rhs) => Ok(lhs > rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(lhs > rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(lhs > rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => Ok(lhs > rhs),
            _ => Err(err()),
        }
    }
    pub fn op_ge(&self, rhs: &Self, context: &TyContext) -> Result<bool, E> {
        let err = || E::Binary(BinaryOperator::GreaterThanEqual, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, lhs, rhs) => Ok(lhs >= rhs),
            both!(Self::AbstractInt, lhs, rhs) => Ok(lhs >= rhs),
            both!(Self::AbstractFloat, lhs, rhs) => Ok(lhs >= rhs),
            both!(Self::I32, lhs, rhs) => Ok(lhs >= rhs),
            both!(Self::U32, lhs, rhs) => Ok(lhs >= rhs),
            both!(Self::F32, lhs, rhs) => Ok(lhs >= rhs),
            both!(Self::F16, lhs, rhs) => Ok(lhs >= rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, lhs, rhs) => Ok(lhs >= rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, lhs, rhs) => Ok(lhs >= rhs),
            #[cfg(feature = "naga-ext")]
            both!(Self::F64, lhs, rhs) => Ok(lhs >= rhs),
            _ => Err(err()),
        }
    }
}

impl VecInstance {
    pub fn op_eq(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Equality, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_eq(r, context).map(Into::into))
    }
    pub fn op_ne(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::Inequality, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_ne(r, context).map(Into::into))
    }
    pub fn op_lt(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::LessThan, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_lt(r, context).map(Into::into))
    }
    pub fn op_le(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::LessThanEqual, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_le(r, context).map(Into::into))
    }
    pub fn op_gt(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::GreaterThan, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_gt(r, context).map(Into::into))
    }
    pub fn op_ge(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let (lhs, rhs) = convert(self, rhs, context)
            .ok_or_else(|| E::Binary(BinaryOperator::GreaterThanEqual, self.ty(), rhs.ty()))?;
        lhs.compwise_binary(&rhs, |l, r| l.op_ge(r, context).map(Into::into))
    }
}

impl Instance {
    pub fn op_eq(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs
                .op_eq(rhs, context)
                .map(|b| Self::Literal(LiteralInstance::Bool(b))),
            both!(Self::Vec, lhs, rhs) => lhs.op_eq(rhs, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::Equality, self.ty(), rhs.ty())),
        }
    }
    pub fn op_ne(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs
                .op_ne(rhs, context)
                .map(|b| Self::Literal(LiteralInstance::Bool(b))),
            both!(Self::Vec, lhs, rhs) => lhs.op_ne(rhs, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::Inequality, self.ty(), rhs.ty())),
        }
    }
    pub fn op_lt(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs
                .op_lt(rhs, context)
                .map(|b| Self::Literal(LiteralInstance::Bool(b))),
            both!(Self::Vec, lhs, rhs) => lhs.op_lt(rhs, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::LessThan, self.ty(), rhs.ty())),
        }
    }
    pub fn op_le(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs
                .op_le(rhs, context)
                .map(|b| Self::Literal(LiteralInstance::Bool(b))),
            both!(Self::Vec, lhs, rhs) => lhs.op_le(rhs, context).map(Into::into),
            _ => Err(E::Binary(
                BinaryOperator::LessThanEqual,
                self.ty(),
                rhs.ty(),
            )),
        }
    }
    pub fn op_gt(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs
                .op_gt(rhs, context)
                .map(|b| Self::Literal(LiteralInstance::Bool(b))),
            both!(Self::Vec, lhs, rhs) => lhs.op_gt(rhs, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::GreaterThan, self.ty(), rhs.ty())),
        }
    }
    pub fn op_ge(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs
                .op_ge(rhs, context)
                .map(|b| Self::Literal(LiteralInstance::Bool(b))),
            both!(Self::Vec, lhs, rhs) => lhs.op_ge(rhs, context).map(Into::into),
            _ => Err(E::Binary(
                BinaryOperator::GreaterThanEqual,
                self.ty(),
                rhs.ty(),
            )),
        }
    }
}

// ---------------
// BIT EXPRESSIONS
// ---------------
// reference: https://www.w3.org/TR/WGSL/#bit-expr

impl LiteralInstance {
    pub fn op_bitnot(&self) -> Result<Self, E> {
        match self {
            Self::AbstractInt(n) => Ok(Self::AbstractInt(!n)),
            Self::I32(n) => Ok(Self::I32(!n)),
            Self::U32(n) => Ok(Self::U32(!n)),
            #[cfg(feature = "naga-ext")]
            Self::I64(n) => Ok(Self::I64(!n)),
            #[cfg(feature = "naga-ext")]
            Self::U64(n) => Ok(Self::U64(!n)),
            _ => Err(E::Unary(UnaryOperator::BitwiseComplement, self.ty())),
        }
    }
    /// Note: this is both the "bitwise OR" and "logical OR" operator.
    pub fn op_bitor(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::BitwiseOr, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, rhs, lhs) => Ok(Self::Bool(lhs | rhs)),
            both!(Self::AbstractInt, rhs, lhs) => Ok(Self::AbstractInt(lhs | rhs)),
            both!(Self::I32, rhs, lhs) => Ok(Self::I32(lhs | rhs)),
            both!(Self::U32, rhs, lhs) => Ok(Self::U32(lhs | rhs)),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, rhs, lhs) => Ok(Self::I64(lhs | rhs)),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, rhs, lhs) => Ok(Self::U64(lhs | rhs)),
            _ => Err(err()),
        }
    }
    /// Note: this is both the "bitwise AND" and "logical AND" operator.
    pub fn op_bitand(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::BitwiseAnd, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::Bool, rhs, lhs) => Ok(Self::Bool(lhs & rhs)),
            both!(Self::AbstractInt, rhs, lhs) => Ok(Self::AbstractInt(lhs & rhs)),
            both!(Self::I32, rhs, lhs) => Ok(Self::I32(lhs & rhs)),
            both!(Self::U32, rhs, lhs) => Ok(Self::U32(lhs & rhs)),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, rhs, lhs) => Ok(Self::I64(lhs & rhs)),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, rhs, lhs) => Ok(Self::U64(lhs & rhs)),
            _ => Err(err()),
        }
    }
    pub fn op_bitxor(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::BitwiseXor, self.ty(), rhs.ty());
        match convert(self, rhs, context).ok_or_else(err)? {
            both!(Self::AbstractInt, rhs, lhs) => Ok(Self::AbstractInt(lhs ^ rhs)),
            both!(Self::I32, rhs, lhs) => Ok(Self::I32(lhs ^ rhs)),
            both!(Self::U32, rhs, lhs) => Ok(Self::U32(lhs ^ rhs)),
            #[cfg(feature = "naga-ext")]
            both!(Self::I64, rhs, lhs) => Ok(Self::I64(lhs ^ rhs)),
            #[cfg(feature = "naga-ext")]
            both!(Self::U64, rhs, lhs) => Ok(Self::U64(lhs ^ rhs)),
            _ => Err(err()),
        }
    }
    pub fn op_shl(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::ShiftLeft, self.ty(), rhs.ty());
        let r = rhs
            .convert_to(&Type::U32, context)
            .ok_or_else(err)?
            .unwrap_u32();
        let stage = stage == ShaderStage::Const || stage == ShaderStage::Override;

        // in const and override expressions, shr operation must not overflow (all discarded bits
        // must be 0 in positive expressions and 1 in negative expressions).
        // only abstract types can be shifted by more than the bit width of the operand.
        match self {
            Self::AbstractInt(l) => {
                if r == 0 {
                    // shift by 0 is no-op
                    return Ok(*self);
                } else if r > 63 {
                    // shifting that much always returns 0
                    return Ok(0i64.into());
                }
                let msb_mask = (!0u64) << (63 - r);
                let msb_bits = *l as u64 & msb_mask;
                if stage && (*l >= 0 && msb_bits != 0 || *l < 0 && msb_bits != msb_mask) {
                    Err(E::ShlOverflow(r, *self))
                } else {
                    Ok(l.wrapping_shl(r).into())
                }
            }
            Self::I32(l) => {
                let r = r % 32; // "the number of bits to shift is the value of e2, modulo the bit width of e1"
                if r == 0 {
                    // shift by 0 is no-op
                    return Ok(*self);
                }
                let msb_mask = (!0u32) << (31 - r);
                let msb_bits = *l as u32 & msb_mask;
                if stage && (*l >= 0 && msb_bits != 0 || *l < 0 && msb_bits != msb_mask) {
                    Err(E::ShlOverflow(r, *self))
                } else if stage {
                    Ok(l.checked_shl(r).ok_or(E::ShlOverflow(r, *self))?.into())
                } else {
                    Ok(l.wrapping_shl(r).into())
                }
            }
            Self::U32(l) => {
                let r = r % 32; // "the number of bits to shift is the value of e2, modulo the bit width of e1"
                if r == 0 {
                    // shift by 0 is no-op
                    return Ok(*self);
                }
                let msb_mask = (!0u32) << (32 - r);
                let msb_bits = *l & msb_mask;
                if stage && msb_bits != 0 {
                    Err(E::ShlOverflow(r, *self))
                } else if stage {
                    Ok(l.checked_shl(r).ok_or(E::ShlOverflow(r, *self))?.into())
                } else {
                    Ok(l.wrapping_shl(r).into())
                }
            }
            #[cfg(feature = "naga-ext")]
            Self::I64(l) => {
                let r = r % 64; // "the number of bits to shift is the value of e2, modulo the bit width of e1"
                if r == 0 {
                    // shift by 0 is no-op
                    return Ok(*self);
                }
                let msb_mask = (!0u64) << (31 - r);
                let msb_bits = *l as u64 & msb_mask;
                if stage && (*l >= 0 && msb_bits != 0 || *l < 0 && msb_bits != msb_mask) {
                    Err(E::ShlOverflow(r, *self))
                } else if stage {
                    Ok(Self::I64(l.checked_shl(r).ok_or(E::ShlOverflow(r, *self))?))
                } else {
                    Ok(Self::I64(l.wrapping_shl(r)))
                }
            }
            #[cfg(feature = "naga-ext")]
            Self::U64(l) => {
                let r = r % 64; // "the number of bits to shift is the value of e2, modulo the bit width of e1"
                if r == 0 {
                    // shift by 0 is no-op
                    return Ok(*self);
                }
                let msb_mask = (!0u64) << (64 - r);
                let msb_bits = *l & msb_mask;
                if stage && msb_bits != 0 {
                    Err(E::ShlOverflow(r, *self))
                } else if stage {
                    Ok(Self::U64(l.checked_shl(r).ok_or(E::ShlOverflow(r, *self))?))
                } else {
                    Ok(Self::U64(l.wrapping_shl(r)))
                }
            }
            _ => Err(err()),
        }
    }
    pub fn op_shr(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::ShiftRight, self.ty(), rhs.ty());
        let r = rhs
            .convert_to(&Type::U32, context)
            .ok_or_else(err)?
            .unwrap_u32();
        let stage = stage == ShaderStage::Const || stage == ShaderStage::Override;

        // shift by 0 is no-op
        if r == 0 {
            return Ok(*self);
        }

        // contrary to shl, it is not an error to overflow (discard non-zero bits). But it is an
        // error to shift more than the bit width.
        match self {
            Self::I32(l) => Ok(if stage {
                l.checked_shr(r).ok_or(E::ShrOverflow(r, *self))?.into()
            } else {
                l.wrapping_shr(r).into()
            }),
            Self::U32(l) => Ok(if stage {
                l.checked_shr(r).ok_or(E::ShrOverflow(r, *self))?.into()
            } else {
                l.wrapping_shr(r).into()
            }),
            Self::AbstractInt(l) => {
                // we shr twice because x >> 64 is panic(overflow) and wrapping_shr only allow x >> 63.
                Ok((l >> 1).wrapping_shr(r - 1).into())
            }
            #[cfg(feature = "naga-ext")]
            Self::I64(l) => Ok(if stage {
                Self::I64(l.checked_shr(r).ok_or(E::ShrOverflow(r, *self))?)
            } else {
                Self::I64(l.wrapping_shr(r))
            }),
            #[cfg(feature = "naga-ext")]
            Self::U64(l) => Ok(if stage {
                Self::U64(l.checked_shr(r).ok_or(E::ShrOverflow(r, *self))?)
            } else {
                Self::U64(l.wrapping_shr(r))
            }),
            _ => Err(E::Binary(BinaryOperator::ShiftRight, self.ty(), rhs.ty())),
        }
    }
}

impl VecInstance {
    pub fn op_bitnot(&self) -> Result<Self, E> {
        self.compwise_unary(LiteralInstance::op_bitnot)
    }
    /// Note: this is both the "bitwise OR" and "logical OR" operator.
    pub fn op_bitor(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        self.compwise_binary(rhs, |l, r| l.op_bitor(r, context))
    }
    /// Note: this is both the "bitwise AND" and "logical AND" operator.
    pub fn op_bitand(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        self.compwise_binary(rhs, |l, r| l.op_bitand(r, context))
    }
    pub fn op_bitxor(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        self.compwise_binary(rhs, |l, r| l.op_bitxor(r, context))
    }
    pub fn op_shl(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        self.compwise_binary(rhs, |l, r| l.op_shl(r, stage, context))
    }
    pub fn op_shr(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        self.compwise_binary(rhs, |l, r| l.op_shr(r, stage, context))
    }
}

impl Instance {
    pub fn op_bitnot(&self) -> Result<Self, E> {
        match self {
            Instance::Literal(l) => l.op_bitnot().map(Into::into),
            Instance::Vec(v) => v.op_bitnot().map(Into::into),
            _ => Err(E::Unary(UnaryOperator::BitwiseComplement, self.ty())),
        }
    }
    /// Note: this is both the "bitwise OR" and "logical OR" operator.
    pub fn op_bitor(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_bitor(rhs, context).map(Into::into),
            both!(Self::Vec, lhs, rhs) => lhs.op_bitor(rhs, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::BitwiseOr, self.ty(), rhs.ty())),
        }
    }
    /// Note: this is both the "bitwise AND" and "logical AND" operator.
    pub fn op_bitand(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_bitand(rhs, context).map(Into::into),
            both!(Self::Vec, lhs, rhs) => lhs.op_bitand(rhs, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::BitwiseAnd, self.ty(), rhs.ty())),
        }
    }
    pub fn op_bitxor(&self, rhs: &Self, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_bitxor(rhs, context).map(Into::into),
            both!(Self::Vec, lhs, rhs) => lhs.op_bitxor(rhs, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::BitwiseXor, self.ty(), rhs.ty())),
        }
    }
    pub fn op_shl(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_shl(rhs, stage, context).map(Into::into),
            both!(Self::Vec, lhs, rhs) => lhs.op_shl(rhs, stage, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::ShiftLeft, self.ty(), rhs.ty())),
        }
    }
    pub fn op_shr(&self, rhs: &Self, stage: ShaderStage, context: &TyContext) -> Result<Self, E> {
        match (self, rhs) {
            both!(Self::Literal, lhs, rhs) => lhs.op_shr(rhs, stage, context).map(Into::into),
            both!(Self::Vec, lhs, rhs) => lhs.op_shr(rhs, stage, context).map(Into::into),
            _ => Err(E::Binary(BinaryOperator::ShiftRight, self.ty(), rhs.ty())),
        }
    }
}

// -------------------
// POINTER EXPRESSIONS
// -------------------
// reference: https://www.w3.org/TR/WGSL/#address-of-expr
// reference: https://www.w3.org/TR/WGSL/#indirection-expr

impl Instance {
    pub fn op_ref(&self) -> Result<Instance, E> {
        match self {
            Instance::Ref(r) => {
                if r.space == AddressSpace::Handle {
                    // "It is a shader-creation error if AS is the handle address space."
                    Err(E::PtrHandle)
                } else if r.ptr.borrow().ty().is_vec() && r.view != MemView::Whole {
                    // "It is a shader-creation error if r is a reference to a vector component."
                    Err(E::PtrVecComp)
                } else {
                    Ok(PtrInstance::from(r.clone()).into())
                }
            }
            _ => Err(E::Unary(UnaryOperator::AddressOf, self.ty())),
        }
    }

    pub fn op_deref(&self) -> Result<Instance, E> {
        match self {
            Instance::Ptr(p) => Ok(RefInstance::from(p.clone()).into()),
            _ => Err(E::Unary(UnaryOperator::Indirection, self.ty())),
        }
    }
}
