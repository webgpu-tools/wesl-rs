use wgsl_parse::syntax::*;
use wgsl_types::{
    CallSignature, ShaderStage,
    inst::{Instance, LiteralInstance, MemView},
    ty::Type,
    ty_context::{DisplayWithContext, TyContext},
};

use super::{Flow, ScopeKind};

/// Evaluation and Execution errors.
#[derive(Clone, Debug)]
pub enum EvalError {
    Todo(String),
    Unreachable,

    // types & templates
    NotScalar(Type),
    NotConstructible(Type),
    Type(Type, Type),
    SampledType(Type),
    NotType(String),
    UnknownType(String),
    UnknownStruct(String),
    NotAccessible(String, ShaderStage),
    UnexpectedTemplate(String),
    MissingTemplate(&'static str),

    // references
    View(Type, MemView),
    RefType(Type, Type),
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
    Index(Type),
    NotIndexable(Type),
    Swizzle(String),
    OutOfBounds(usize, Type, usize),

    // arithmetic
    Unary(UnaryOperator, Type),
    Binary(BinaryOperator, Type, Type),
    CompwiseBinary(Type, Type),
    NegOverflow,
    AddOverflow,
    SubOverflow,
    MulOverflow,
    DivByZero,
    RemZeroDiv,
    ShlOverflow(u32, LiteralInstance),
    ShrOverflow(u32, LiteralInstance),

    // functions
    UnknownFunction(String),
    NotCallable(String),
    Signature(CallSignature),
    Builtin(&'static str),
    TemplateArgs(&'static str),
    ParamCount(String, usize, usize),
    ParamType(Type, Type),
    ReturnType(Type, String, Type),
    NoReturn(String, Type),
    UnexpectedReturn(String, Type),
    NotConst(String),
    Void(String),
    MustUse(String),
    NotEntrypoint(String),
    InvalidEntrypointParam(String),
    MissingBuiltinInput(BuiltinValue, String),
    OutputBuiltin(BuiltinValue),
    InputBuiltin(BuiltinValue),
    MissingUserInput(String, u32),

    // declarations
    UnknownDecl(String),
    OverrideInConst,
    OverrideInFn,
    LetInMod,
    UninitConst(String),
    UninitLet(String),
    UninitOverride(String),
    ForbiddenInitializer(AddressSpace),
    DuplicateDecl(String),
    UntypedDecl,
    ForbiddenDecl(DeclarationKind, ScopeKind),
    MissingResource(u32, u32),
    AddressSpace(AddressSpace, AddressSpace),
    AccessMode(AccessMode, AccessMode),

    // attributes
    MissingBindAttr,
    MissingWorkgroupSize,
    NegativeAttr(i64),
    InvalidBlendSrc(u32),

    // statements
    NotRef(Instance),
    AssignType(Type, Type),
    IncrType(Type),
    IncrOverflow,
    DecrType(Type),
    DecrOverflow,
    FlowInContinuing(Flow),
    DiscardInConst,
    ConstAssertFailure(ExpressionNode),
    FlowInFunction(Flow),
    FlowInModule(Flow),
}

impl DisplayWithContext for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, context: &TyContext) -> std::fmt::Result {
        match self {
            EvalError::Todo(value) => write!(f, "not implemented: `{value}`"),
            EvalError::Unreachable => write!(f, "unreachable code detected"),
            EvalError::NotScalar(ty) => {
                write!(f, "expected a scalar type, got `{}`", context.display(ty))
            }
            EvalError::NotConstructible(ty) => {
                write!(f, "`{}` is not constructible", context.display(ty))
            }
            EvalError::Type(expected_ty, actual_ty) => write!(
                f,
                "expected type `{}`, got `{}`",
                context.display(expected_ty),
                context.display(actual_ty)
            ),
            EvalError::SampledType(ty) => write!(
                f,
                "invalid sampled type, expected `i32`, `u32` of `f32`, got `{}`",
                context.display(ty)
            ),
            EvalError::NotType(name) => write!(f, "expected a type, got declaration `{name}`"),
            EvalError::UnknownType(name) => write!(f, "unknown type `{name}`"),
            EvalError::UnknownStruct(name) => write!(f, "unknown struct `{name}`"),
            EvalError::NotAccessible(name, shader_stage) => {
                let stage = match shader_stage {
                    ShaderStage::Const => "shader-module-creation",
                    ShaderStage::Override => "pipeline-creation",
                    ShaderStage::Exec => "shader-execution",
                };
                write!(f, "declaration `{name}` is not accessible at {stage} time")
            }
            EvalError::UnexpectedTemplate(name) => {
                write!(f, "type `{name}` does not take any template arguments")
            }
            EvalError::MissingTemplate(name) => {
                write!(f, "missing template arguments for type `{name}`")
            }
            EvalError::View(ty, mem_view) => write!(
                f,
                "invalid reference to memory view `{}`{mem_view}",
                context.display(ty)
            ),
            EvalError::RefType(actual_ty, expected_ty) => write!(
                f,
                "invalid reference to `{}`, expected reference to `{}`",
                context.display(actual_ty),
                context.display(expected_ty)
            ),
            EvalError::WriteRefType(value_ty, ref_ty) => write!(
                f,
                "cannot write a `{}` to a reference to `{}`",
                context.display(value_ty),
                context.display(ref_ty)
            ),
            EvalError::NotWrite => write!(f, "attempt to write to a read-only reference"),
            EvalError::NotRead => write!(f, "attempt to read a write-only reference"),
            EvalError::NotReadWrite => write!(f, "reference is not read-write"),
            EvalError::PtrHandle => write!(f, "cannot create a pointer in `handle` address space"),
            EvalError::PtrVecComp => write!(f, "cannot create a pointer to a vector component"),
            EvalError::Conversion(from_ty, to_ty) => write!(
                f,
                "cannot convert from `{}` to `{}`",
                context.display(from_ty),
                context.display(to_ty)
            ),
            EvalError::ConvOverflow(literal_instance, ty) => write!(
                f,
                "overflow while converting `{literal_instance}` to `{}`",
                context.display(ty)
            ),
            EvalError::Component(ty, name) => {
                write!(f, "`{}` has no component `{name}`", context.display(ty))
            }
            EvalError::Index(ty) => write!(f, "invalid array index type `{}`", context.display(ty)),
            EvalError::NotIndexable(ty) => write!(f, "`{}` cannot be indexed", context.display(ty)),
            EvalError::Swizzle(name) => write!(f, "invalid vector component or swizzle `{name}`"),
            EvalError::OutOfBounds(index, ty, num_components) => write!(
                f,
                "index `{index}` is out-of-bounds for `{}` of `{num_components}` components",
                context.display(ty)
            ),
            EvalError::Unary(unary_operator, ty) => write!(
                f,
                "cannot use unary operator `{unary_operator}` on type `{}`",
                context.display(ty)
            ),
            EvalError::Binary(binary_operator, left_ty, right_ty) => write!(
                f,
                "cannot use binary operator `{binary_operator}` with operands `{}` and `{}`",
                context.display(left_ty),
                context.display(right_ty)
            ),
            EvalError::CompwiseBinary(left_ty, right_ty) => write!(
                f,
                "cannot apply component-wise binary operation on operands `{}` and `{}`",
                context.display(left_ty),
                context.display(right_ty)
            ),
            EvalError::NegOverflow => write!(f, "attempt to negate with overflow"),
            EvalError::AddOverflow => write!(f, "attempt to add with overflow"),
            EvalError::SubOverflow => write!(f, "attempt to subtract with overflow"),
            EvalError::MulOverflow => write!(f, "attempt to multiply with overflow"),
            EvalError::DivByZero => write!(f, "attempt to divide by zero"),
            EvalError::RemZeroDiv => write!(
                f,
                "attempt to calculate the remainder with a divisor of zero"
            ),
            EvalError::ShlOverflow(amount, literal_instance) => write!(
                f,
                "attempt to shift left by `{amount}`, which would overflow `{literal_instance}`"
            ),
            EvalError::ShrOverflow(amount, literal_instance) => write!(
                f,
                "attempt to shift right by `{amount}`, which would overflow `{literal_instance}`"
            ),
            EvalError::UnknownFunction(name) => write!(f, "unknown function `{name}`"),
            EvalError::NotCallable(name) => write!(f, "declaration `{name}` is not callable"),
            EvalError::Signature(call_signature) => write!(
                f,
                "invalid function call signature: `{}`",
                context.display(call_signature)
            ),
            EvalError::Builtin(name) => write!(f, "{name}"),
            EvalError::TemplateArgs(name) => write!(f, "invalid template arguments to `{name}`"),
            EvalError::ParamCount(name, expected_count, actual_count) => write!(
                f,
                "incorrect number of arguments to `{name}`, expected `{expected_count}`, got `{actual_count}`"
            ),
            EvalError::ParamType(expected_ty, actual_ty) => write!(
                f,
                "invalid parameter type, expected `{}`, got `{}`",
                context.display(expected_ty),
                context.display(actual_ty)
            ),
            EvalError::ReturnType(returned_ty, fn_name, expected_ty) => write!(
                f,
                "returned `{}` from function `{}` that returns `{}`",
                context.display(returned_ty),
                fn_name,
                context.display(expected_ty)
            ),
            EvalError::NoReturn(fn_name, expected_ty) => write!(
                f,
                "call to function `{}` did not return any value, expected `{}`",
                fn_name,
                context.display(expected_ty)
            ),
            EvalError::UnexpectedReturn(fn_name, returned_ty) => write!(
                f,
                "function `{}` has no return type, but it returns `{}`",
                fn_name,
                context.display(returned_ty)
            ),
            EvalError::NotConst(name) => {
                write!(f, "calling non-const function `{name}` in const context")
            }
            EvalError::Void(name) => write!(
                f,
                "expected a value, but function `{name}` has no return type"
            ),
            EvalError::MustUse(name) => write!(
                f,
                "function `{name}` has the `@must_use` attribute, its return value must be used"
            ),
            EvalError::NotEntrypoint(name) => write!(f, "function `{name}` is not an entrypoint"),
            EvalError::InvalidEntrypointParam(name) => write!(
                f,
                "entry point function parameter `{name}` must have a @builtin or @location attribute"
            ),
            EvalError::MissingBuiltinInput(builtin_value, parameter_name) => write!(
                f,
                "missing builtin input `{builtin_value}` bound to parameter `{parameter_name}`"
            ),
            EvalError::OutputBuiltin(builtin_value) => write!(
                f,
                "builtin value `{builtin_value}` is an output, but is used as a function parameter"
            ),
            EvalError::InputBuiltin(builtin_value) => write!(
                f,
                "builtin value `{builtin_value}` is an input, but is used as a function return type"
            ),
            EvalError::MissingUserInput(parameter_name, location) => write!(
                f,
                "missing user-defined input bound to parameter `{parameter_name}` at location `{location}`"
            ),
            EvalError::UnknownDecl(name) => write!(f, "unknown declaration `{name}`"),
            EvalError::OverrideInConst => write!(
                f,
                "override-declarations are not permitted in const contexts"
            ),
            EvalError::OverrideInFn => write!(
                f,
                "override-declarations are not permitted in function bodies"
            ),
            EvalError::LetInMod => {
                write!(f, "let-declarations are not permitted at the module scope")
            }
            EvalError::UninitConst(name) => write!(f, "uninitialized const-declaration `{name}`"),
            EvalError::UninitLet(name) => write!(f, "uninitialized let-declaration `{name}`"),
            EvalError::UninitOverride(name) => write!(
                f,
                "uninitialized override-declaration `{name}` with no override"
            ),
            EvalError::ForbiddenInitializer(address_space) => write!(
                f,
                "initializer are not allowed in `{address_space}` address space"
            ),
            EvalError::DuplicateDecl(name) => {
                write!(f, "duplicate declaration of `{name}` in the current scope")
            }
            EvalError::UntypedDecl => write!(
                f,
                "a declaration must have an explicit type or an initializer"
            ),
            EvalError::ForbiddenDecl(declaration_kind, scope_kind) => write!(
                f,
                "`{declaration_kind}` declarations are forbidden in `{scope_kind}` scope"
            ),
            EvalError::MissingResource(group, binding) => write!(
                f,
                "no resource was bound to `@group({group}) @binding({binding})`"
            ),
            EvalError::AddressSpace(expected, actual) => write!(
                f,
                "incorrect resource address space, expected `{expected}`, got `{actual}`"
            ),
            EvalError::AccessMode(expected, actual) => write!(
                f,
                "incorrect resource access mode, expected `{expected}`, got `{actual}`"
            ),
            EvalError::MissingBindAttr => write!(f, "missing `@group` or `@binding` attributes"),
            EvalError::MissingWorkgroupSize => write!(f, "missing `@workgroup_size` attribute"),
            EvalError::NegativeAttr(value) => write!(
                f,
                "`the attribute must evaluate to a positive integer, got `{value}`"
            ),
            EvalError::InvalidBlendSrc(value) => write!(
                f,
                "the `@blend_src` attribute must evaluate to 0 or 1, got `{value}`"
            ),
            EvalError::NotRef(instance) => write!(
                f,
                "expected a reference, got value `{}`",
                context.display(instance)
            ),
            EvalError::AssignType(value_ty, expected_ty) => write!(
                f,
                "cannot assign a `{}` to a `{}`",
                context.display(value_ty),
                context.display(expected_ty)
            ),
            EvalError::IncrType(ty) => write!(f, "cannot increment a `{}`", context.display(ty)),
            EvalError::IncrOverflow => write!(f, "attempt to increment with overflow"),
            EvalError::DecrType(ty) => write!(f, "cannot decrement a `{}`", context.display(ty)),
            EvalError::DecrOverflow => write!(f, "attempt to decrement with overflow"),
            EvalError::FlowInContinuing(flow) => {
                write!(f, "a continuing body cannot contain a `{flow}` statement")
            }
            EvalError::DiscardInConst => {
                write!(f, "discard statements are not permitted in const contexts")
            }
            EvalError::ConstAssertFailure(spanned) => {
                write!(f, "const assertion failed: `{spanned}` is `false`")
            }
            EvalError::FlowInFunction(flow) => {
                write!(f, "a function body cannot contain a `{flow}` statement")
            }
            EvalError::FlowInModule(flow) => write!(
                f,
                "a global declaration cannot contain a `{flow}` statement"
            ),
        }
    }
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
