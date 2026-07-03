use thiserror::Error;
use wgsl_parse::syntax::*;
use wgsl_types::{
    CallSignature, ShaderStage,
    inst::{Instance, LiteralInstance, MemView},
    ty::Type,
};

use super::{Flow, ScopeKind};

/// Evaluation and Execution errors.
#[derive(Clone, Debug, Error)]
pub enum EvalError {
    #[error("not implemented: `{0}`")]
    Todo(String),
    #[error("unreachable code detected")]
    Unreachable,

    // types & templates
    #[error("expected a scalar type, got `{0}`")]
    NotScalar(Type),
    #[error("`{0}` is not constructible")]
    NotConstructible(Type),
    #[error("expected type `{0}`, got `{1}`")]
    Type(Type, Type),
    #[error("invalid sampled type, expected `i32`, `u32` of `f32`, got `{0}`")]
    SampledType(Type),
    #[error("expected a type, got declaration `{0}`")]
    NotType(String),
    #[error("unknown type `{0}`")]
    UnknownType(String),
    #[error("unknown struct `{0}`")]
    UnknownStruct(String),
    #[error("declaration `{0}` is not accessible at {stage} time", stage = match .1 {
        ShaderStage::Const => "shader-module-creation",
        ShaderStage::Override => "pipeline-creation",
        ShaderStage::Exec => "shader-execution"
    })]
    NotAccessible(String, ShaderStage),
    #[error("type `{0}` does not take any template arguments")]
    UnexpectedTemplate(String),
    #[error("missing template arguments for type `{0}`")]
    MissingTemplate(&'static str),

    // references
    #[error("invalid reference to memory view `{0}{1}`")]
    View(Type, MemView),
    #[error("invalid reference to `{0}`, expected reference to `{1}`")]
    RefType(Type, Type),
    #[error("cannot write a `{0}` to a reference to `{1}`")]
    WriteRefType(Type, Type),
    #[error("attempt to write to a read-only reference")]
    NotWrite,
    #[error("attempt to read a write-only reference")]
    NotRead,
    #[error("reference is not read-write")]
    NotReadWrite,
    #[error("cannot create a pointer in `handle` address space")]
    PtrHandle,
    #[error("cannot create a pointer to a vector component")]
    PtrVecComp,

    // conversions
    #[error("cannot convert from `{0}` to `{1}`")]
    Conversion(Type, Type),
    #[error("overflow while converting `{0}` to `{1}`")]
    ConvOverflow(LiteralInstance, Type),

    // indexing
    #[error("`{0}` has no component `{1}`")]
    Component(Type, String),
    #[error("invalid array index type `{0}`")]
    Index(Type),
    #[error("`{0}` cannot be indexed")]
    NotIndexable(Type),
    #[error("invalid vector component or swizzle `{0}`")]
    Swizzle(String),
    #[error("index `{0}` is out-of-bounds for `{1}` of `{2}` components")]
    OutOfBounds(usize, Type, usize),

    // arithmetic
    #[error("cannot use unary operator `{0}` on type `{1}`")]
    Unary(UnaryOperator, Type),
    #[error("cannot use binary operator `{0}` with operands `{1}` and `{2}`")]
    Binary(BinaryOperator, Type, Type),
    #[error("cannot apply component-wise binary operation on operands `{0}` and `{1}`")]
    CompwiseBinary(Type, Type),
    #[error("attempt to negate with overflow")]
    NegOverflow,
    #[error("attempt to add with overflow")]
    AddOverflow,
    #[error("attempt to subtract with overflow")]
    SubOverflow,
    #[error("attempt to multiply with overflow")]
    MulOverflow,
    #[error("attempt to divide by zero")]
    DivByZero,
    #[error("attempt to calculate the remainder with a divisor of zero")]
    RemZeroDiv,
    #[error("attempt to shift left by `{0}`, which would overflow `{1}`")]
    ShlOverflow(u32, LiteralInstance),
    #[error("attempt to shift right by `{0}`, which would overflow `{1}`")]
    ShrOverflow(u32, LiteralInstance),

    // functions
    #[error("unknown function `{0}`")]
    UnknownFunction(String),
    #[error("declaration `{0}` is not callable")]
    NotCallable(String),
    #[error("invalid function call signature: `{0}`")]
    Signature(CallSignature),
    #[error("{0}")]
    Builtin(&'static str),
    #[error("invalid template arguments to `{0}`")]
    TemplateArgs(&'static str),
    #[error("incorrect number of arguments to `{0}`, expected `{1}`, got `{2}`")]
    ParamCount(String, usize, usize),
    #[error("invalid parameter type, expected `{0}`, got `{1}`")]
    ParamType(Type, Type),
    #[error("returned `{0}` from function `{1}` that returns `{2}`")]
    ReturnType(Type, String, Type),
    #[error("call to function `{0}` did not return any value, expected `{1}`")]
    NoReturn(String, Type),
    #[error("function `{0}` has no return type, but it returns `{1}`")]
    UnexpectedReturn(String, Type),
    #[error("calling non-const function `{0}` in const context")]
    NotConst(String),
    #[error("expected a value, but function `{0}` has no return type")]
    Void(String),
    #[error("function `{0}` has the `@must_use` attribute, its return value must be used")]
    MustUse(String),
    #[error("function `{0}` is not an entrypoint")]
    NotEntrypoint(String),
    #[error("entry point function parameter `{0}` must have a @builtin or @location attribute")]
    InvalidEntrypointParam(String),
    #[error("missing builtin input `{0}` bound to parameter `{0}`")]
    MissingBuiltinInput(BuiltinValue, String),
    #[error("builtin value `{0}` is an output, but is used as a function parameter")]
    OutputBuiltin(BuiltinValue),
    #[error("builtin value `{0}` is an input, but is used as a function return type")]
    InputBuiltin(BuiltinValue),
    #[error("missing user-defined input bound to parameter `{0}` at location `{1}`")]
    MissingUserInput(String, u32),

    // declarations
    #[error("unknown declaration `{0}`")]
    UnknownDecl(String),
    #[error("override-declarations are not permitted in const contexts")]
    OverrideInConst,
    #[error("override-declarations are not permitted in function bodies")]
    OverrideInFn,
    #[error("let-declarations are not permitted at the module scope")]
    LetInMod,
    #[error("uninitialized const-declaration `{0}`")]
    UninitConst(String),
    #[error("uninitialized let-declaration `{0}`")]
    UninitLet(String),
    #[error("uninitialized override-declaration `{0}` with no override")]
    UninitOverride(String),
    #[error("initializer are not allowed in `{0}` address space")]
    ForbiddenInitializer(AddressSpace),
    #[error("duplicate declaration of `{0}` in the current scope")]
    DuplicateDecl(String),
    #[error("a declaration must have an explicit type or an initializer")]
    UntypedDecl,
    #[error("`{0}` declarations are forbidden in `{1}` scope")]
    ForbiddenDecl(DeclarationKind, ScopeKind),
    #[error("no resource was bound to `@group({0}) @binding({1})`")]
    MissingResource(u32, u32),
    #[error("incorrect resource address space, expected `{0}`, got `{1}`")]
    AddressSpace(AddressSpace, AddressSpace),
    #[error("incorrect resource access mode, expected `{0}`, got `{1}`")]
    AccessMode(AccessMode, AccessMode),

    // attributes
    #[error("missing `@group` or `@binding` attributes")]
    MissingBindAttr,
    #[error("missing `@workgroup_size` attribute")]
    MissingWorkgroupSize,
    #[error("`the attribute must evaluate to a positive integer, got `{0}`")]
    NegativeAttr(i64),
    #[error("the `@blend_src` attribute must evaluate to 0 or 1, got `{0}`")]
    InvalidBlendSrc(u32),

    // statements
    #[error("expected a reference, got value `{0}`")]
    NotRef(Instance),
    #[error("cannot assign a `{0}` to a `{1}`")]
    AssignType(Type, Type),
    #[error("cannot increment a `{0}`")]
    IncrType(Type),
    #[error("attempt to increment with overflow")]
    IncrOverflow,
    #[error("cannot decrement a `{0}`")]
    DecrType(Type),
    #[error("attempt to decrement with overflow")]
    DecrOverflow,
    #[error("a continuing body cannot contain a `{0}` statement")]
    FlowInContinuing(Flow),
    #[error("discard statements are not permitted in const contexts")]
    DiscardInConst,
    #[error("const assertion failed: `{0}` is `false`")]
    ConstAssertFailure(ExpressionNode),
    #[error("a function body cannot contain a `{0}` statement")]
    FlowInFunction(Flow),
    #[error("a global declaration cannot contain a `{0}` statement")]
    FlowInModule(Flow),
}

impl From<wgsl_types::Error> for EvalError {
    fn from(value: wgsl_types::Error) -> Self {
        match value {
            wgsl_types::Error::Todo(a) => Self::Todo(a),
            wgsl_types::Error::Unreachable => Self::Unreachable,
            wgsl_types::Error::NotScalar(a) => Self::NotScalar(a),
            wgsl_types::Error::NotConstructible(a) => Self::NotConstructible(a),
            wgsl_types::Error::SampledType(a) => Self::SampledType(a),
            wgsl_types::Error::UnknownType(a) => Self::UnknownType(a),
            wgsl_types::Error::UnexpectedTemplate(a) => Self::UnexpectedTemplate(a),
            wgsl_types::Error::MissingTemplate(a) => Self::MissingTemplate(a),
            wgsl_types::Error::WriteRefType(a, b) => Self::WriteRefType(a, b),
            wgsl_types::Error::NotWrite => Self::NotWrite,
            wgsl_types::Error::NotRead => Self::NotRead,
            wgsl_types::Error::NotReadWrite => Self::NotReadWrite,
            wgsl_types::Error::PtrHandle => Self::PtrHandle,
            wgsl_types::Error::PtrVecComp => Self::PtrVecComp,
            wgsl_types::Error::Conversion(a, b) => Self::Conversion(a, b),
            wgsl_types::Error::ConvOverflow(a, b) => Self::ConvOverflow(a, b),
            wgsl_types::Error::Component(a, b) => Self::Component(a, b),
            wgsl_types::Error::NotIndexable(a) => Self::NotIndexable(a),
            wgsl_types::Error::OutOfBounds(a, b, c) => Self::OutOfBounds(a, b, c),
            wgsl_types::Error::Unary(a, b) => Self::Unary(a, b),
            wgsl_types::Error::Binary(a, b, c) => Self::Binary(a, b, c),
            wgsl_types::Error::CompwiseBinary(a, b) => Self::CompwiseBinary(a, b),
            wgsl_types::Error::AddOverflow => Self::AddOverflow,
            wgsl_types::Error::SubOverflow => Self::SubOverflow,
            wgsl_types::Error::MulOverflow => Self::MulOverflow,
            wgsl_types::Error::DivByZero => Self::DivByZero,
            wgsl_types::Error::RemZeroDiv => Self::RemZeroDiv,
            wgsl_types::Error::ShlOverflow(a, b) => Self::ShlOverflow(a, b),
            wgsl_types::Error::ShrOverflow(a, b) => Self::ShrOverflow(a, b),
            wgsl_types::Error::Signature(a) => Self::Signature(a),
            wgsl_types::Error::Builtin(a) => Self::Builtin(a),
            wgsl_types::Error::TemplateArgs(a) => Self::TemplateArgs(a),
            wgsl_types::Error::ParamCount(a, b, c) => Self::ParamCount(a, b, c),
            wgsl_types::Error::ParamType(a, b) => Self::ParamType(a, b),
        }
    }
}
