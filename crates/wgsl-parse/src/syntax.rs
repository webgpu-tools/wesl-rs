//! A syntax tree for WGSL and WESL files. The root of the tree is [`TranslationUnit`].
//!
//! The syntax tree closely mirrors WGSL spec syntax while allowing language extensions.
//!
//! ## Strictness
//!
//! This syntax tree is rather strict, meaning it cannot represent most syntactically
//! incorrect programs. But it is only syntactic, meaning it doesn't perform many
//! contextual checks: for example, certain attributes can only appear in certain places,
//! or declarations have different constraints depending on where they appear.
//!
//! ## WESL Extensions
//!
//! WESL extensions are enabled with the `imports`, `generics`, `attributes` and `condcomp`. Read more about WESL at <https://wesl-lang.dev>.
//!
//! ## Design considerations
//!
//! The parsing is not designed to be primarily efficient, but flexible and correct.
//! It is made with the ultimate goal to implement spec-compliant language extensions.

use std::sync::{Arc, RwLock, RwLockReadGuard};

use derive_more::{From, IsVariant, Unwrap};

pub use crate::span::{Span, Spanned};

pub use wgsl_types::syntax::*;

#[cfg(feature = "tokrepr")]
use tokrepr::TokRepr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Default, Clone, Debug, PartialEq)]
pub struct TranslationUnit {
    #[cfg(feature = "wesl")]
    pub imports: Vec<ImportStatement>,
    pub global_directives: Vec<GlobalDirective>,
    pub global_declarations: Vec<GlobalDeclarationNode>,
}

/// Identifiers correspond to WGSL `ident` syntax node, except that they have several
/// convenience features:
/// * Can be shared by cloning (they are shared pointers)
/// * Can be [renamed][Self::rename] (with interior mutability)
/// * References to the same Ident can be [counted][Self::use_count]
/// * Equality and Hash compares the reference, NOT the internal string value
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Ident(Arc<RwLock<String>>);

impl Ident {
    /// Create a new Ident
    pub fn new(name: String) -> Ident {
        // TODO: check that the name is a valid ident
        Ident(Arc::new(RwLock::new(name)))
    }
    /// Get the name of the Ident
    pub fn name(&self) -> RwLockReadGuard<'_, String> {
        self.0.read().unwrap()
    }
    /// Rename all shared instances of the ident
    pub fn rename(&mut self, name: String) {
        *self.0.write().unwrap() = name;
    }
    /// Count shared instances of the ident
    pub fn use_count(&self) -> usize {
        Arc::<_>::strong_count(&self.0)
    }
}

impl From<String> for Ident {
    fn from(name: String) -> Self {
        Ident::new(name)
    }
}

/// equality for idents is based on address, NOT internal value
impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// equality for idents is based on address, NOT internal value
impl Eq for Ident {}

/// hash for idents is based on address, NOT internal value
impl std::hash::Hash for Ident {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(&*self.0, state)
    }
}

#[cfg(feature = "wesl")]
#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ImportStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub path: Option<ModulePath>,
    pub content: ImportContent,
}

#[cfg(feature = "wesl")]
#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash, IsVariant)]
pub enum PathOrigin {
    /// Import relative to the current package root, starting with 'package::'.
    Absolute,
    /// Import relative to the current module, starting with 'super::'. The  usize is the number of 'super'.
    Relative(usize),
    /// Import from a package dependency, starting with the extern package name.
    Package(String),
}

#[cfg(feature = "wesl")]
#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModulePath {
    pub origin: PathOrigin,
    pub components: Vec<String>,
}

#[cfg(feature = "wesl")]
#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    pub path: Vec<String>,
    pub content: ImportContent,
}

#[cfg(feature = "wesl")]
#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, IsVariant)]
pub enum ImportContent {
    Item(ImportItem),
    Collection(Vec<Import>),
}

#[cfg(feature = "wesl")]
#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ImportItem {
    pub ident: Ident,
    pub rename: Option<Ident>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, From, IsVariant, Unwrap)]
pub enum GlobalDirective {
    Diagnostic(DiagnosticDirective),
    Enable(EnableDirective),
    Requires(RequiresDirective),
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticDirective {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub severity: DiagnosticSeverity,
    pub rule_name: String,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct EnableDirective {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub extensions: Vec<String>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct RequiresDirective {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub extensions: Vec<String>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, From, IsVariant, Unwrap)]
pub enum GlobalDeclaration {
    Void,
    Declaration(Declaration),
    TypeAlias(TypeAlias),
    Struct(Struct),
    Function(Function),
    ConstAssert(ConstAssert),
}

pub type GlobalDeclarationNode = Spanned<GlobalDeclaration>;

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub attributes: Attributes,
    pub kind: DeclarationKind,
    pub ident: Ident,
    pub ty: Option<TypeExpression>,
    pub initializer: Option<ExpressionNode>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IsVariant)]
pub enum DeclarationKind {
    Const,
    Override,
    Let,
    Var(Option<(AddressSpace, Option<AccessMode>)>), // "None" corresponds to handle space if it is a module-scope declaration, otherwise function space.
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct TypeAlias {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub ident: Ident,
    pub ty: TypeExpression,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Struct {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub ident: Ident,
    pub members: Vec<StructMemberNode>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct StructMember {
    pub attributes: Attributes,
    pub ident: Ident,
    pub ty: TypeExpression,
}

pub type StructMemberNode = Spanned<StructMember>;

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub attributes: Attributes,
    pub ident: Ident,
    pub parameters: Vec<FormalParameter>,
    pub return_attributes: Attributes,
    pub return_type: Option<TypeExpression>,
    pub body: CompoundStatement,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FormalParameter {
    pub attributes: Attributes,
    pub ident: Ident,
    pub ty: TypeExpression,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ConstAssert {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub expression: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticAttribute {
    pub severity: DiagnosticSeverity,
    pub rule: String,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct InterpolateAttribute {
    pub ty: InterpolationType,
    pub sampling: Option<InterpolationSampling>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct WorkgroupSizeAttribute {
    pub x: ExpressionNode,
    pub y: Option<ExpressionNode>,
    pub z: Option<ExpressionNode>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct CustomAttribute {
    pub name: String,
    pub arguments: Option<Vec<ExpressionNode>>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, From, IsVariant, Unwrap)]
pub enum Attribute {
    Align(ExpressionNode),
    Binding(ExpressionNode),
    BlendSrc(ExpressionNode),
    #[from]
    Builtin(BuiltinValue),
    Const,
    #[from]
    Diagnostic(DiagnosticAttribute),
    Group(ExpressionNode),
    Id(ExpressionNode),
    #[from]
    Interpolate(InterpolateAttribute),
    Invariant,
    Location(ExpressionNode),
    MustUse,
    Size(ExpressionNode),
    #[from]
    WorkgroupSize(WorkgroupSizeAttribute),
    Vertex,
    Fragment,
    Compute,
    #[cfg(feature = "wesl")]
    Publish,
    #[cfg(feature = "wesl")]
    If(ExpressionNode),
    #[cfg(feature = "wesl")]
    Elif(ExpressionNode),
    #[cfg(feature = "wesl")]
    Else,
    #[cfg(feature = "generics")]
    #[from]
    Type(TypeConstraint),
    #[cfg(feature = "naga-ext")]
    EarlyDepthTest(Option<ConservativeDepth>),
    #[from]
    Custom(CustomAttribute),
}

pub type AttributeNode = Spanned<Attribute>;

#[cfg(feature = "generics")]
#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, From)]
pub struct TypeConstraint {
    pub ident: Ident,
    pub variants: Vec<TypeExpression>,
}

pub type Attributes = Vec<AttributeNode>;

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, From, IsVariant, Unwrap)]
pub enum Expression {
    Literal(LiteralExpression),
    Parenthesized(ParenthesizedExpression),
    NamedComponent(NamedComponentExpression),
    Indexing(IndexingExpression),
    Unary(UnaryExpression),
    Binary(BinaryExpression),
    FunctionCall(FunctionCallExpression),
    TypeOrIdentifier(TypeExpression),
}

pub type ExpressionNode = Spanned<Expression>;

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, From, IsVariant, Unwrap)]
pub enum LiteralExpression {
    Bool(bool),
    AbstractInt(i64),
    AbstractFloat(f64),
    I32(i32),
    U32(u32),
    F32(f32),
    #[from(skip)]
    F16(f32),
    #[cfg(feature = "naga-ext")]
    #[from(skip)]
    I64(i64),
    #[cfg(feature = "naga-ext")]
    #[from(skip)]
    U64(u64),
    #[cfg(feature = "naga-ext")]
    #[from(skip)]
    F64(f64),
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ParenthesizedExpression {
    pub expression: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NamedComponentExpression {
    pub base: ExpressionNode,
    pub component: Ident,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct IndexingExpression {
    pub base: ExpressionNode,
    pub index: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct UnaryExpression {
    pub operator: UnaryOperator,
    pub operand: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct BinaryExpression {
    pub operator: BinaryOperator,
    pub left: ExpressionNode,
    pub right: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCall {
    pub ty: TypeExpression,
    pub arguments: Vec<ExpressionNode>,
}

pub type FunctionCallExpression = FunctionCall;

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct TypeExpression {
    #[cfg(feature = "wesl")]
    pub path: Option<ModulePath>,
    pub ident: Ident,
    pub template_args: TemplateArgs,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct TemplateArg {
    pub expression: ExpressionNode,
}
pub type TemplateArgs = Option<Vec<TemplateArg>>;

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, From, IsVariant, Unwrap)]
pub enum Statement {
    Void,
    Compound(CompoundStatement),
    Assignment(AssignmentStatement),
    Increment(IncrementStatement),
    Decrement(DecrementStatement),
    If(IfStatement),
    Switch(SwitchStatement),
    Loop(LoopStatement),
    For(ForStatement),
    While(WhileStatement),
    Break(BreakStatement),
    Continue(ContinueStatement),
    Return(ReturnStatement),
    Discard(DiscardStatement),
    FunctionCall(FunctionCallStatement),
    ConstAssert(ConstAssertStatement),
    Declaration(DeclarationStatement),
}

pub type StatementNode = Spanned<Statement>;

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CompoundStatement {
    pub attributes: Attributes,
    pub statements: Vec<StatementNode>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct AssignmentStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub operator: AssignmentOperator,
    pub lhs: ExpressionNode,
    pub rhs: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct IncrementStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub expression: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct DecrementStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub expression: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct IfStatement {
    pub attributes: Attributes,
    pub if_clause: IfClause,
    pub else_if_clauses: Vec<ElseIfClause>,
    pub else_clause: Option<ElseClause>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct IfClause {
    pub expression: ExpressionNode,
    pub body: CompoundStatement,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ElseIfClause {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub expression: ExpressionNode,
    pub body: CompoundStatement,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ElseClause {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub body: CompoundStatement,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchStatement {
    pub attributes: Attributes,
    pub expression: ExpressionNode,
    pub body_attributes: Attributes,
    pub clauses: Vec<SwitchClause>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchClause {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub case_selectors: Vec<CaseSelector>,
    pub body: CompoundStatement,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, From, IsVariant, Unwrap)]
pub enum CaseSelector {
    Default,
    Expression(ExpressionNode),
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct LoopStatement {
    pub attributes: Attributes,
    pub body: CompoundStatement,
    // a ContinuingStatement can only appear inside a LoopStatement body, therefore it is
    // not part of the StatementNode enum. it appears here instead, but consider it part of
    // body as the last statement of the CompoundStatement.
    pub continuing: Option<ContinuingStatement>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuingStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub body: CompoundStatement,
    // a BreakIfStatement can only appear inside a ContinuingStatement body, therefore it
    // not part of the StatementNode enum. it appears here instead, but consider it part of
    // body as the last statement of the CompoundStatement.
    pub break_if: Option<BreakIfStatement>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct BreakIfStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub expression: ExpressionNode,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ForStatement {
    pub attributes: Attributes,
    pub initializer: Option<StatementNode>,
    pub condition: Option<ExpressionNode>,
    pub update: Option<StatementNode>,
    pub body: CompoundStatement,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct WhileStatement {
    pub attributes: Attributes,
    pub condition: ExpressionNode,
    pub body: CompoundStatement,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct BreakStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ContinueStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ReturnStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub expression: Option<ExpressionNode>,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct DiscardStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
}

#[cfg_attr(feature = "tokrepr", derive(TokRepr))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCallStatement {
    #[cfg(feature = "wesl")]
    pub attributes: Attributes,
    pub call: FunctionCall,
}

pub type ConstAssertStatement = ConstAssert;

pub type DeclarationStatement = Declaration;
