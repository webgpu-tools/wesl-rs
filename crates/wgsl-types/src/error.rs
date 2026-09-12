use crate::{
    CallSignature, Type,
    inst::LiteralInstance,
    syntax::{BinaryOperator, UnaryOperator},
};

/// The global error struct.
#[derive(Clone, Debug)]
pub enum Error {
    Todo(String),
    Unreachable,

    // types & templates
    NotScalar(Type),
    NotConstructible(Type),
    SampledType(Type),
    UnknownType(String),
    UnexpectedTemplate(String),
    MissingTemplate(&'static str),

    // references
    WriteRefType(Type, Type),
    NotWrite,
    NotRead,
    NotReadWrite,
    PtrHandle,
    PtrVecComp,

    // conversions
    Conversion(Type, Type),
    ConvOverflow(LiteralInstance, Type),

    // indexing
    Component(Type, String),
    NotIndexable(Type),
    OutOfBounds(usize, Type, usize),

    // arithmetic
    Unary(UnaryOperator, Type),
    Binary(BinaryOperator, Type, Type),
    CompwiseBinary(Type, Type),
    AddOverflow,
    SubOverflow,
    MulOverflow,
    DivByZero,
    RemZeroDiv,
    ShlOverflow(u32, LiteralInstance),
    ShrOverflow(u32, LiteralInstance),

    // functions
    Signature(CallSignature),
    Builtin(&'static str),
    TemplateArgs(&'static str),
    ParamCount(String, usize, usize),
    ParamType(Type, Type),
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Todo(v) => write!(fmt, "not implemented: `{v}`"),
            Error::Unreachable => write!(fmt, "unreachable code detected"),
            Error::NotScalar(ty) => write!(fmt, "expected a scalar type, got `{ty}`"),
            Error::NotConstructible(ty) => write!(fmt, "`{ty}` is not constructible"),
            Error::SampledType(ty) => write!(
                fmt,
                "invalid sampled type, expected `i32`, `u32` or `f32`, got `{ty}`"
            ),
            Error::UnknownType(ty) => {
                write!(fmt, "unknown type `{ty}`")
            }
            Error::UnexpectedTemplate(ty) => {
                write!(fmt, "type `{ty}` does not take any template arguments")
            }
            Error::MissingTemplate(ty) => write!(fmt, "missing template arguments for type `{ty}`"),
            Error::WriteRefType(new_ty, ty) => {
                write!(fmt, "cannot write a `{new_ty}` to a reference to `{ty}`")
            }
            Error::NotWrite => write!(fmt, "attempt to write to a read-only reference"),
            Error::NotRead => write!(fmt, "attempt to read a write-only reference"),
            Error::NotReadWrite => write!(fmt, "reference is not read-write"),
            Error::PtrHandle => write!(fmt, "cannot create a pointer in `handle` address space"),
            Error::PtrVecComp => write!(fmt, "cannot create a pointer to a vector component"),
            Error::Conversion(from_ty, to_ty) => {
                write!(fmt, "cannot convert from `{from_ty}` to `{to_ty}`")
            }
            Error::ConvOverflow(literal, ty) => {
                write!(fmt, "overflow while converting `{literal}` to `{ty}`")
            }
            Error::Component(ty, name) => write!(fmt, "`{ty}` has no component `{name}`"),
            Error::NotIndexable(ty) => write!(fmt, "`{ty}` cannot be indexed"),
            Error::OutOfBounds(index, ty, num_components) => write!(
                fmt,
                "index `{index}` is out-of-bounds for `{ty}` of `{num_components}` components"
            ),
            Error::Unary(op, ty) => write!(fmt, "cannot use unary operator `{op}` on type `{ty}`"),
            Error::Binary(op, left_ty, right_ty) => write!(
                fmt,
                "cannot use binary operator `{op}` with operands `{left_ty}` and `{right_ty}`"
            ),
            Error::CompwiseBinary(ty_1, ty_2) => write!(
                fmt,
                "cannot apply component-wise binary operation on operands `{ty_1}` and `{ty_2}`"
            ),
            Error::AddOverflow => write!(fmt, "attempt to add with overflow"),
            Error::SubOverflow => write!(fmt, "attempt to subtract with overflow"),
            Error::MulOverflow => write!(fmt, "attempt to multiply with overflow"),
            Error::DivByZero => write!(fmt, "attempt to divide by zero"),
            Error::RemZeroDiv => write!(
                fmt,
                "attempt to calculate the remainder with a divisor of zero"
            ),
            Error::ShlOverflow(num, ty) => write!(
                fmt,
                "attempt to shift left by `{num}`, which would overflow `{ty}`"
            ),
            Error::ShrOverflow(num, ty) => write!(
                fmt,
                "attempt to shift right by `{num}`, which would overflow `{ty}`"
            ),
            Error::Signature(call_signature) => {
                write!(fmt, "invalid function call signature: `{call_signature}`")
            }
            Error::Builtin(name) => write!(fmt, "{name}"),
            Error::TemplateArgs(name) => write!(fmt, "invalid template arguments to `{name}`"),
            Error::ParamCount(name, expected_count, actual_count) => write!(
                fmt,
                "incorrect number of arguments to `{name}`, expected `{expected_count}`, got `{actual_count}`"
            ),
            Error::ParamType(expected_ty, actual_ty) => write!(
                fmt,
                "invalid parameter type, expected `{expected_ty}`, got `{actual_ty}`"
            ),
        }
    }
}
