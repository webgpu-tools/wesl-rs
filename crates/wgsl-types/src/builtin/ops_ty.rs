//! Type-checking of operators.

use crate::{
    Error,
    conv::{Convert, convert_ty},
    syntax::{AddressSpace, BinaryOperator, UnaryOperator},
    ty::{Ty, Type},
};

type E = Error;

/// Compute the return type of a unary operator expression.
pub fn type_unary_op(operator: UnaryOperator, operand: &Type) -> Result<Type, E> {
    match operator {
        UnaryOperator::LogicalNegation => operand.op_not(),
        UnaryOperator::Negation => operand.op_neg(),
        UnaryOperator::BitwiseComplement => operand.op_bitnot(),
        UnaryOperator::AddressOf => operand.op_ref(),
        UnaryOperator::Indirection => operand.op_deref(),
    }
}

/// Compute the return type of a binary operator expression.
pub fn type_binary_op(op: BinaryOperator, lhs: &Type, rhs: &Type) -> Result<Type, E> {
    match op {
        BinaryOperator::ShortCircuitOr => lhs.op_or(rhs),
        BinaryOperator::ShortCircuitAnd => lhs.op_and(rhs),
        BinaryOperator::Addition => lhs.op_add(rhs),
        BinaryOperator::Subtraction => lhs.op_sub(rhs),
        BinaryOperator::Multiplication => lhs.op_mul(rhs),
        BinaryOperator::Division => lhs.op_div(rhs),
        BinaryOperator::Remainder => lhs.op_rem(rhs),
        BinaryOperator::Equality => lhs.op_eq(rhs),
        BinaryOperator::Inequality => lhs.op_ne(rhs),
        BinaryOperator::LessThan => lhs.op_lt(rhs),
        BinaryOperator::LessThanEqual => lhs.op_le(rhs),
        BinaryOperator::GreaterThan => lhs.op_gt(rhs),
        BinaryOperator::GreaterThanEqual => lhs.op_ge(rhs),
        BinaryOperator::BitwiseOr => lhs.op_bitor(rhs),
        BinaryOperator::BitwiseAnd => lhs.op_bitand(rhs),
        BinaryOperator::BitwiseXor => lhs.op_bitxor(rhs),
        BinaryOperator::ShiftLeft => lhs.op_shl(rhs),
        BinaryOperator::ShiftRight => lhs.op_shr(rhs),
    }
}

// -------------------
// LOGICAL EXPRESSIONS
// -------------------
// reference: https://www.w3.org/TR/WGSL/#logical-expr

impl Type {
    pub fn op_not(&self) -> Result<Self, E> {
        match self {
            Self::Bool | Self::Vec(_, _) => Ok(self.clone()),
            _ => Err(E::Unary(UnaryOperator::LogicalNegation, self.clone())),
        }
    }
    pub fn op_or(&self, rhs: &Type) -> Result<Self, E> {
        match (self, rhs) {
            (Type::Bool, Type::Bool) => Ok(Type::Bool),
            _ => Err(E::Binary(
                BinaryOperator::ShortCircuitOr,
                self.ty(),
                rhs.ty(),
            )),
        }
    }
    pub fn op_and(&self, rhs: &Type) -> Result<Self, E> {
        match (self, rhs) {
            (Type::Bool, Type::Bool) => Ok(Type::Bool),
            _ => Err(E::Binary(
                BinaryOperator::ShortCircuitAnd,
                self.ty(),
                rhs.ty(),
            )),
        }
    }
}

// ----------------------
// ARITHMETIC EXPRESSIONS
// ----------------------
// reference: https://www.w3.org/TR/WGSL/#arithmetic-expr

impl Type {
    /// Valid operands:
    /// * `-S`, S: scalar
    pub fn op_neg(&self) -> Result<Self, E> {
        if self.is_scalar() || matches!(self, Type::Vec(_, inner_ty) if inner_ty.is_scalar()) {
            Ok(self.clone())
        } else {
            Err(E::Unary(UnaryOperator::Negation, self.ty()))
        }
    }

    /// Valid operands:
    /// * `T + T`, T: scalar or vec
    /// * `S + V` or `V + S`, S: scalar, V: `vec<S>`
    /// * `M + M`, M: mat
    pub fn op_add(&self, rhs: &Type) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Addition, self.ty(), rhs.ty());
        match (self, rhs) {
            (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() || lhs.is_vec() && rhs.is_vec() => {
                let ty = convert_ty(self, rhs).ok_or_else(err)?;
                Ok(ty.clone())
            }
            (scalar_ty, Type::Vec(n, vec_ty)) | (Type::Vec(n, vec_ty), scalar_ty)
                if scalar_ty.is_scalar() =>
            {
                let inner_ty = convert_ty(scalar_ty, vec_ty).ok_or_else(err)?;
                Ok(Type::Vec(*n, Box::new(inner_ty.clone())))
            }
            (Type::Mat(c1, r1, lhs), Type::Mat(c2, r2, rhs)) if c1 == c2 && r1 == r2 => {
                let inner_ty = convert_ty(lhs, rhs).ok_or_else(err)?;
                Ok(Type::Mat(*c1, *c2, Box::new(inner_ty.clone())))
            }
            _ => Err(err()),
        }
    }

    /// Valid operands:
    /// * `T - T`, T: scalar or vec
    /// * `S - V` or `V - S`, S: scalar, V: `vec<S>`
    /// * `M - M`, M: mat
    pub fn op_sub(&self, rhs: &Type) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Subtraction, self.ty(), rhs.ty());
        match (self, rhs) {
            (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() || lhs.is_vec() && rhs.is_vec() => {
                let ty = convert_ty(self, rhs).ok_or_else(err)?;
                Ok(ty.clone())
            }
            (scalar_ty, Type::Vec(n, vec_ty)) | (Type::Vec(n, vec_ty), scalar_ty)
                if scalar_ty.is_scalar() =>
            {
                let inner_ty = convert_ty(scalar_ty, vec_ty).ok_or_else(err)?;
                Ok(Type::Vec(*n, Box::new(inner_ty.clone())))
            }
            (Type::Mat(c1, r1, lhs), Type::Mat(c2, r2, rhs)) if c1 == c2 && r1 == r2 => {
                let inner_ty = convert_ty(lhs, rhs).ok_or_else(err)?;
                Ok(Type::Mat(*c1, *c2, Box::new(inner_ty.clone())))
            }
            _ => Err(err()),
        }
    }

    /// Valid operands:
    /// * `T * T`, T: scalar or vec
    /// * `S * V` or `V * S`, S: scalar, V: `vec<S>`
    /// * `S * M` or `M * S`, S: float, M: `mat<S>`
    /// * `V * M` or `M * V`, S: float, V: `vec<S>`, M: `mat<S>`
    /// * `M1 * M1`, M1: `matKxR`, M2: `matCxK`
    pub fn op_mul(&self, rhs: &Type) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Multiplication, self.ty(), rhs.ty());
        match (self, rhs) {
            (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() || lhs.is_vec() && rhs.is_vec() => {
                let ty = convert_ty(self, rhs).ok_or_else(err)?;
                Ok(ty.clone())
            }
            // component-wise scaling
            (scalar_ty, Type::Vec(n, vec_ty)) | (Type::Vec(n, vec_ty), scalar_ty)
                if scalar_ty.is_scalar() =>
            {
                let inner_ty = convert_ty(scalar_ty, vec_ty).ok_or_else(err)?;
                Ok(Type::Vec(*n, Box::new(inner_ty.clone())))
            }
            // component-wise scaling
            (scalar_ty, Type::Mat(c, r, mat_ty)) | (Type::Mat(c, r, mat_ty), scalar_ty)
                if scalar_ty.is_scalar() =>
            {
                let inner_ty = convert_ty(scalar_ty, mat_ty).ok_or_else(err)?;
                Ok(Type::Mat(*c, *r, Box::new(inner_ty.clone())))
            }
            // linear algebra row-vector-matrix product
            (Type::Vec(n1, vec_ty), Type::Mat(n, n2, mat_ty))
            // linear algebra matrix-column-vector product
            | (Type::Mat(n1, n, mat_ty), Type::Vec(n2, vec_ty))
             if n1 == n2 => {
                let inner_ty = convert_ty(vec_ty, mat_ty).ok_or_else(err)?;
                Ok(Type::Vec(*n, Box::new(inner_ty.clone())))
            }
            // linear algebra matrix product
            (Type::Mat(k1, r, lhs), Type::Mat(c, k2, rhs)) if k1 == k2 => {
                let inner_ty = convert_ty(lhs, rhs).ok_or_else(err)?;
                Ok(Type::Mat(*c, *r, Box::new(inner_ty.clone())))
            }
            _ => Err(err()),
        }
    }

    /// Valid operands:
    /// * `T / T`, T: scalar or vec
    /// * `S / V` or `V / S`, S: scalar, V: `vec<S>`
    pub fn op_div(&self, rhs: &Type) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Division, self.ty(), rhs.ty());
        match (self, rhs) {
            (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() || lhs.is_vec() && rhs.is_vec() => {
                let ty = convert_ty(self, rhs).ok_or_else(err)?;
                Ok(ty.clone())
            }
            (scalar_ty, Type::Vec(n, vec_ty)) | (Type::Vec(n, vec_ty), scalar_ty)
                if scalar_ty.is_scalar() =>
            {
                let inner_ty = convert_ty(scalar_ty, vec_ty).ok_or_else(err)?;
                Ok(Type::Vec(*n, Box::new(inner_ty.clone())))
            }
            _ => Err(err()),
        }
    }

    /// Valid operands:
    /// * `T % T`, T: scalar or vec
    /// * `S % V` or `V % S`, S: scalar, V: `vec<S>`
    pub fn op_rem(&self, rhs: &Type) -> Result<Self, E> {
        let err = || E::Binary(BinaryOperator::Remainder, self.ty(), rhs.ty());
        match (self, rhs) {
            (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() || lhs.is_vec() && rhs.is_vec() => {
                let ty = convert_ty(self, rhs).ok_or_else(err)?;
                Ok(ty.clone())
            }
            (scalar_ty, Type::Vec(n, vec_ty)) | (Type::Vec(n, vec_ty), scalar_ty)
                if scalar_ty.is_scalar() =>
            {
                let inner_ty = convert_ty(scalar_ty, vec_ty).ok_or_else(err)?;
                Ok(Type::Vec(*n, Box::new(inner_ty.clone())))
            }
            _ => Err(err()),
        }
    }
}

// ----------------------
// COMPARISON EXPRESSIONS
// ----------------------
// reference: https://www.w3.org/TR/WGSL/#comparison-expr

impl Type {
    /// Valid operands:
    /// * `T == T`, T: scalar or vec
    pub fn op_eq(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::Equality, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_scalar() => Ok(Type::Bool),
            Type::Vec(n, _) => Ok(Type::Vec(*n, Box::new(Type::Bool))),
            _ => Err(err()),
        }
    }
    /// Valid operands:
    /// * `T != T`, T: scalar or vec
    pub fn op_ne(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::Inequality, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_scalar() => Ok(Type::Bool),
            Type::Vec(n, _) => Ok(Type::Vec(*n, Box::new(Type::Bool))),
            _ => Err(err()),
        }
    }
    /// Valid operands:
    /// * `T < T`, T: scalar or vec
    pub fn op_lt(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::LessThan, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_scalar() => Ok(Type::Bool),
            Type::Vec(n, _) => Ok(Type::Vec(*n, Box::new(Type::Bool))),
            _ => Err(err()),
        }
    }
    /// Valid operands:
    /// * `T <= T`, T: scalar or vec
    pub fn op_le(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::LessThanEqual, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_scalar() => Ok(Type::Bool),
            Type::Vec(n, _) => Ok(Type::Vec(*n, Box::new(Type::Bool))),
            _ => Err(err()),
        }
    }
    /// Valid operands:
    /// * `T > T`, T: scalar or vec
    pub fn op_gt(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::GreaterThan, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_scalar() => Ok(Type::Bool),
            Type::Vec(n, _) => Ok(Type::Vec(*n, Box::new(Type::Bool))),
            _ => Err(err()),
        }
    }
    /// Valid operands:
    /// * `T >= T`, T: scalar or vec
    pub fn op_ge(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::GreaterThanEqual, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_scalar() => Ok(Type::Bool),
            Type::Vec(n, _) => Ok(Type::Vec(*n, Box::new(Type::Bool))),
            _ => Err(err()),
        }
    }
}

// ---------------
// BIT EXPRESSIONS
// ---------------
// reference: https://www.w3.org/TR/WGSL/#bit-expr

impl Type {
    /// Valid operands:
    /// * `~T`, I: integer, T: I or `vec<I>`
    pub fn op_bitnot(&self) -> Result<Type, E> {
        match self {
            ty if ty.is_integer() => Ok(self.clone()),
            Type::Vec(_, ty) if ty.is_integer() => Ok(self.clone()),
            _ => Err(E::Unary(UnaryOperator::BitwiseComplement, self.ty())),
        }
    }

    /// Note: this is both the "bitwise OR" and "logical OR" operator.
    ///
    /// Valid operands:
    /// * `T | T`, T: integer, or `vec<integer>` (bitwise OR)
    /// * `V | V`, V: `vec<bool>` (logical OR)
    pub fn op_bitor(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::BitwiseOr, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_integer() => Ok(self.clone()),
            Type::Vec(_, ty) if ty.is_integer() => Ok(self.clone()),
            Type::Vec(_, ty) if ty.is_bool() => Ok(self.clone()),
            _ => Err(err()),
        }
    }

    /// Note: this is both the "bitwise AND" and "logical AND" operator.
    ///
    /// Valid operands:
    /// * `T & T`, T: integer or `vec<integer>` (bitwise AND)
    /// * `V & V`, V: `vec<bool>` (logical AND)
    pub fn op_bitand(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::BitwiseAnd, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_integer() => Ok(self.clone()),
            Type::Vec(_, ty) if ty.is_integer() => Ok(self.clone()),
            Type::Vec(_, ty) if ty.is_bool() => Ok(self.clone()),
            _ => Err(err()),
        }
    }

    /// Valid operands:
    /// * `T ^ T`, T: integer or `vec<integer>`
    pub fn op_bitxor(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::BitwiseXor, self.ty(), rhs.ty());
        match convert_ty(self, rhs).ok_or_else(err)? {
            ty if ty.is_integer() => Ok(self.clone()),
            Type::Vec(_, ty) if ty.is_integer() => Ok(self.clone()),
            _ => Err(err()),
        }
    }

    /// Valid operands:
    /// * `integer << u32`
    /// * `vec<integer> << vec<u32>`
    pub fn op_shl(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::ShiftLeft, self.ty(), rhs.ty());
        let rhs = rhs.convert_inner_to(&Type::U32).ok_or_else(err)?;
        match (self, rhs) {
            (lhs, Type::U32) if lhs.is_integer() => Ok(lhs.clone()),
            (lhs, Type::Vec(_, _)) => Ok(lhs.clone()),
            _ => Err(err()),
        }
    }

    /// Valid operands:
    /// * `integer >> u32`
    /// * `vec<integer> >> vec<u32>`
    pub fn op_shr(&self, rhs: &Type) -> Result<Type, E> {
        let err = || E::Binary(BinaryOperator::ShiftRight, self.ty(), rhs.ty());
        let rhs = rhs.convert_inner_to(&Type::U32).ok_or_else(err)?;
        match (self, rhs) {
            (lhs, Type::U32) if lhs.is_integer() => Ok(lhs.clone()),
            (lhs, Type::Vec(_, _)) => Ok(lhs.clone()),
            _ => Err(err()),
        }
    }
}

// -------------------
// POINTER EXPRESSIONS
// -------------------
// reference: https://www.w3.org/TR/WGSL/#address-of-expr
// reference: https://www.w3.org/TR/WGSL/#indirection-expr

impl Type {
    pub fn op_ref(&self) -> Result<Type, E> {
        match self {
            Type::Ref(a_s, ty, a_m) => {
                if *a_s == AddressSpace::Handle {
                    // "It is a shader-creation error if AS is the handle address space."
                    Err(E::PtrHandle)
                } else if false {
                    // TODO: We do not yet have enough information to check this:
                    // "It is a shader-creation error if r is a reference to a vector component."
                    Err(E::PtrVecComp)
                } else {
                    Ok(Type::Ptr(*a_s, ty.clone(), *a_m))
                }
            }
            _ => Err(E::Unary(UnaryOperator::AddressOf, self.ty())),
        }
    }

    pub fn op_deref(&self) -> Result<Type, E> {
        match self {
            Type::Ptr(a_s, ty, a_m) => Ok(Type::Ref(*a_s, ty.clone(), *a_m)),
            _ => Err(E::Unary(UnaryOperator::Indirection, self.ty())),
        }
    }
}
