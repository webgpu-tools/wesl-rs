use wgsl_types::inst::LiteralInstance;

use super::syntax::*;
use crate::span::Spanned;

impl TranslationUnit {
    /// New empty [`TranslationUnit`]
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove all [`GlobalDeclaration::Void`] and [`Statement::Void`]
    pub fn remove_voids(&mut self) {
        self.global_declarations
            .retain_mut(|decl| match decl.node() {
                GlobalDeclaration::Void => false,
                _ => {
                    decl.remove_voids();
                    true
                }
            })
    }
}

#[cfg(feature = "imports")]
impl ModulePath {
    /// Create a new module path from components.
    ///
    /// Precondition: the path components must be valid WGSL identifiers.
    pub fn new(origin: PathOrigin, components: Vec<String>) -> Self {
        Self { origin, components }
    }

    /// Create a module path that refers to the root module, i.e. `package`.
    ///
    /// Technically `import package;` is not a valid import statement in WESL code.
    /// However adding an item to the path, such as `import package::foo;` points at
    /// declaration `foo` in the root module.
    pub fn new_root() -> Self {
        Self::new(PathOrigin::Absolute, vec![])
    }

    /// Create a new module path from a filesystem path.
    ///
    /// * Paths with a root (leading `/` on Unix) produce `package::` paths.
    /// * Relative paths (starting with `.` or `..`) produce `self::` or `super::` paths.
    /// * The file extension is ignored.
    ///
    /// # Panics
    ///
    /// * The path must not start with a prefix, like C:\ on windows.
    /// * The path must contain at least one named component.
    /// * Named components must be valid module names.
    ///   (Module names are WGSL identifiers + certain reserved names, see wesl-spec#127)
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Self {
        use std::path::Component;
        let path = path.as_ref().with_extension("");
        let mut parts = path.components().peekable();

        let origin = match parts.next() {
            Some(Component::Prefix(_)) => panic!("path starts with a Windows prefix"),
            Some(Component::RootDir) => PathOrigin::Absolute,
            Some(Component::CurDir) => PathOrigin::Relative(0),
            Some(Component::ParentDir) => {
                let mut n = 1;
                while let Some(&Component::ParentDir) = parts.peek() {
                    n += 1;
                    parts.next().unwrap();
                }
                PathOrigin::Relative(n)
            }
            Some(Component::Normal(name)) => {
                PathOrigin::Package(name.to_string_lossy().to_string())
            }
            None => panic!("path is empty"),
        };

        let mut components = Vec::new();
        for part in parts {
            match part {
                Component::Prefix(_) | Component::RootDir => panic!("unexpected path component"),
                Component::CurDir => {}
                Component::ParentDir => {
                    components.pop().expect("path escapes its origin");
                    // TODO: if we escape the root we should insert a `super::`
                }
                Component::Normal(name) => {
                    components.push(name.to_string_lossy().to_string());
                }
            }
        }

        Self { origin, components }
    }

    /// Create a `PathBuf` from a `ModulePath`.
    ///
    /// * `package::` paths are rooted (start with `/`).
    /// * self::` or `super::` are relative (starting with `.` or `..`)`.
    /// * There is no file extension.
    pub fn to_path_buf(&self) -> std::path::PathBuf {
        use std::path::PathBuf;
        let mut fs_path = match &self.origin {
            PathOrigin::Absolute => PathBuf::from("/"),
            PathOrigin::Relative(0) => PathBuf::from("."),
            PathOrigin::Relative(n) => PathBuf::from_iter((0..*n).map(|_| "..")),
            PathOrigin::Package(name) => PathBuf::from(name),
        };
        fs_path.extend(&self.components);
        fs_path
    }

    /// Append a component to the path.
    ///
    /// Precondition: the `item` must be a valid WGSL identifier.
    pub fn push(&mut self, item: &str) {
        self.components.push(item.to_string());
    }

    /// Get the first component of the module path.
    pub fn first(&self) -> Option<&str> {
        self.components.first().map(String::as_str)
    }

    /// Get the last component of the module path.
    pub fn last(&self) -> Option<&str> {
        self.components.last().map(String::as_str)
    }

    /// Append `suffix` to the module path.
    pub fn join(mut self, suffix: impl IntoIterator<Item = String>) -> Self {
        self.components.extend(suffix);
        self
    }

    /// Append `suffix` to the module path.
    ///
    /// This function produces a `ModulePath` relative to `self`, as if `suffix` was
    /// imported from module `self`.
    ///
    /// * If `suffix` is relative, it appends its components to `self`.
    /// * If `suffix` if absolute or package, it ignores `self` components.
    /// * If both `self` and `suffix` are package paths, then `suffix` imports from a
    ///   sub-package. The package is renamed with a slash separating package names.
    ///   (TODO: this is a hack)
    pub fn join_path(&self, suffix: &Self) -> Self {
        match suffix.origin {
            PathOrigin::Absolute => {
                match self.origin {
                    PathOrigin::Absolute | PathOrigin::Relative(_) => suffix.clone(),
                    PathOrigin::Package(_) => {
                        // absolute import from inside a package is a package import
                        let origin = self.origin.clone();
                        let components = suffix.components.clone();
                        Self { origin, components }
                    }
                }
            }
            PathOrigin::Relative(n) => {
                let to_keep = self.components.len().saturating_sub(n);
                let components = self
                    .components
                    .iter()
                    .take(to_keep)
                    .chain(&suffix.components)
                    .cloned()
                    .collect::<Vec<_>>();
                let origin = match self.origin {
                    PathOrigin::Absolute | PathOrigin::Package(_) => self.origin.clone(),
                    PathOrigin::Relative(m) => {
                        PathOrigin::Relative(m + n.saturating_sub(self.components.len()))
                    }
                };
                Self { origin, components }
            }
            PathOrigin::Package(ref suffix_pkg) => {
                match &self.origin {
                    PathOrigin::Absolute | PathOrigin::Relative(_) => suffix.clone(),
                    PathOrigin::Package(self_pkg) => {
                        if self_pkg.rsplit('/').next() == suffix_pkg.rsplit('/').next() {
                            // Same package - just use the suffix path with the package origin
                            let origin = self.origin.clone();
                            let components = suffix.components.clone();
                            Self { origin, components }
                        } else {
                            // Importing a sub-package. This is a hack: we rename the package to
                            // parent/child, which cannot be spelled in code.
                            let origin = PathOrigin::Package(format!("{self_pkg}/{suffix_pkg}"));
                            let components = suffix.components.clone();
                            Self { origin, components }
                        }
                    }
                }
            }
        }
    }

    /// Whether the module path starts with a `prefix`.
    pub fn starts_with(&self, prefix: &Self) -> bool {
        self.origin == prefix.origin
            && self.components.len() >= prefix.components.len()
            && prefix
                .components
                .iter()
                .zip(&self.components)
                .all(|(a, b)| a == b)
    }

    /// Whether the module path points at the route module.
    ///
    /// See [`Self::new_root`].
    pub fn is_root(&self) -> bool {
        self.origin.is_absolute() && self.components.is_empty()
    }
}

#[cfg(feature = "imports")]
#[test]
fn test_module_path_join() {
    use std::str::FromStr;
    let cases = [
        ("package::m1", "package::foo", "package::foo"),
        ("package::m1", "self::foo", "package::m1::foo"),
        ("package::m1", "super::foo", "package::foo"),
        ("pkg::m1::m2", "package::foo", "pkg::foo"),
        ("pkg::m1::m2", "self::foo", "pkg::m1::m2::foo"),
        ("pkg::m1::m2", "super::foo", "pkg::m1::foo"),
        ("pkg::m1", "super::super::foo", "pkg::foo"),
        ("lygia::m1", "lygia::math", "lygia::math"),
        ("lygia::m1", "pkg/lygia", "lygia"),
        ("pkg/lygia::m1", "lygia", "pkg/lygia"),
        ("pkg/lygia::m1", "lygia::math", "pkg/lygia::math"),
        ("pkg1/lygia::m1", "pkg2/lygia::math", "pkg1/lygia::math"),
        ("super", "super::foo", "super::super::foo"),
        ("super::m1::m2::m3", "super::super::m4", "super::m1::m4"),
        ("super", "self::foo", "super::foo"),
        ("self", "super::foo", "super::foo"),
    ];

    for (parent, child, expect) in cases {
        let parent = ModulePath::from_str(parent).unwrap();
        let child = ModulePath::from_str(child).unwrap();
        let expect = ModulePath::from_str(expect).unwrap();
        println!("testing ModulePath::join_path({parent}, {child}) -> {expect}");
        assert_eq!(parent.join_path(&child), expect);
    }
}

#[cfg(feature = "imports")]
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum ModulePathParseError {
    #[error("module name cannot be empty")]
    Empty,
    #[error("`package` must be a prefix of the module path")]
    MisplacedPackage,
    #[error("`self` must be a prefix of the module path")]
    MisplacedSelf,
    #[error("`super` must be a prefix of the module path")]
    MisplacedSuper,
}

#[cfg(feature = "imports")]
impl std::str::FromStr for ModulePath {
    type Err = ModulePathParseError;

    /// Parse a WGSL string into a module path.
    ///
    /// Preconditions:
    /// * The path components must be valid WESL module names.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split("::").peekable();

        let origin = match parts.next() {
            Some("package") => PathOrigin::Absolute,
            Some("self") => PathOrigin::Relative(0),
            Some("super") => {
                let mut n = 1;
                while let Some(&"super") = parts.peek() {
                    n += 1;
                    parts.next().unwrap();
                }
                PathOrigin::Relative(n)
            }
            Some("") | None => return Err(ModulePathParseError::Empty),
            Some(name) => PathOrigin::Package(name.to_string()),
        };

        let components = parts
            .map(|part| match part {
                "package" => Err(ModulePathParseError::MisplacedPackage),
                "self" => Err(ModulePathParseError::MisplacedSelf),
                "super" => Err(ModulePathParseError::MisplacedSuper),
                _ => Ok(part.to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { origin, components })
    }
}

#[cfg(feature = "imports")]
#[test]
fn test_module_path_fromstr() {
    use std::str::FromStr;

    let ok_cases = [
        ("self", ModulePath::new(PathOrigin::Relative(0), vec![])),
        ("super", ModulePath::new(PathOrigin::Relative(1), vec![])),
        ("package", ModulePath::new(PathOrigin::Absolute, vec![])),
        (
            "a",
            ModulePath::new(PathOrigin::Package("a".to_string()), vec![]),
        ),
        (
            "super::super::a",
            ModulePath::new(PathOrigin::Relative(2), vec!["a".to_string()]),
        ),
    ];
    let err_cases = [
        ("", ModulePathParseError::Empty),
        ("a::super", ModulePathParseError::MisplacedSuper),
        ("super::self", ModulePathParseError::MisplacedSelf),
        ("self::package", ModulePathParseError::MisplacedPackage),
    ];

    for (s, m) in ok_cases {
        assert_eq!(ModulePath::from_str(s), Ok(m))
    }
    for (s, e) in err_cases {
        assert_eq!(ModulePath::from_str(s), Err(e))
    }
}

impl GlobalDeclaration {
    /// Remove all [`Statement::Void`]
    pub fn remove_voids(&mut self) {
        if let GlobalDeclaration::Function(decl) = self {
            decl.body.remove_voids();
        }
    }
}

impl TypeAlias {
    pub fn new(ident: Ident, ty: TypeExpression) -> Self {
        Self {
            #[cfg(feature = "attributes")]
            attributes: Default::default(),
            ident,
            ty,
        }
    }
}

impl Struct {
    pub fn new(ident: Ident) -> Self {
        Self {
            #[cfg(feature = "attributes")]
            attributes: Default::default(),
            ident,
            members: Default::default(),
        }
    }
}

impl StructMember {
    pub fn new(ident: Ident, ty: TypeExpression) -> Self {
        Self {
            attributes: Default::default(),
            ident,
            ty,
        }
    }
}

impl Function {
    pub fn new(ident: Ident) -> Self {
        Self {
            attributes: Default::default(),
            ident,
            parameters: Default::default(),
            return_attributes: Default::default(),
            return_type: Default::default(),
            body: Default::default(),
        }
    }
}

impl FormalParameter {
    pub fn new(ident: Ident, ty: TypeExpression) -> Self {
        Self {
            attributes: Default::default(),
            ident,
            ty,
        }
    }
}

impl ConstAssert {
    pub fn new(expression: Expression) -> Self {
        Self {
            #[cfg(feature = "attributes")]
            attributes: Default::default(),
            expression: expression.into(),
        }
    }
}

impl TypeExpression {
    /// New [`TypeExpression`] with no template.
    pub fn new(ident: Ident) -> Self {
        Self {
            #[cfg(feature = "imports")]
            path: None,
            ident,
            template_args: None,
        }
    }
}

impl CompoundStatement {
    /// Remove all [`Statement::Void`]
    pub fn remove_voids(&mut self) {
        self.statements.retain_mut(|stmt| match stmt.node_mut() {
            Statement::Void => false,
            _ => {
                stmt.remove_voids();
                true
            }
        })
    }
}

impl Statement {
    /// Remove all [`Statement::Void`]
    pub fn remove_voids(&mut self) {
        match self {
            Statement::Compound(stmt) => {
                stmt.remove_voids();
            }
            Statement::If(stmt) => {
                stmt.if_clause.body.remove_voids();
                for clause in &mut stmt.else_if_clauses {
                    clause.body.remove_voids();
                }
                if let Some(clause) = &mut stmt.else_clause {
                    clause.body.remove_voids();
                }
            }
            Statement::Switch(stmt) => stmt
                .clauses
                .iter_mut()
                .for_each(|clause| clause.body.remove_voids()),
            Statement::Loop(stmt) => stmt.body.remove_voids(),
            Statement::For(stmt) => stmt.body.remove_voids(),
            Statement::While(stmt) => stmt.body.remove_voids(),
            _ => (),
        }
    }
}

impl From<LiteralExpression> for LiteralInstance {
    fn from(lit: LiteralExpression) -> Self {
        match lit {
            LiteralExpression::Bool(l) => LiteralInstance::Bool(l),
            LiteralExpression::AbstractInt(l) => LiteralInstance::AbstractInt(l),
            LiteralExpression::AbstractFloat(l) => LiteralInstance::AbstractFloat(l),
            LiteralExpression::I32(l) => LiteralInstance::I32(l),
            LiteralExpression::U32(l) => LiteralInstance::U32(l),
            LiteralExpression::F32(l) => LiteralInstance::F32(l),
            LiteralExpression::F16(l) => LiteralInstance::F16(wgsl_types::inst::f16::from_f32(l)),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::I64(l) => LiteralInstance::I64(l),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::U64(l) => LiteralInstance::U64(l),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::F64(l) => LiteralInstance::F64(l),
        }
    }
}

impl From<Ident> for TypeExpression {
    fn from(ident: Ident) -> Self {
        Self::new(ident)
    }
}

impl From<ExpressionNode> for ReturnStatement {
    fn from(expression: ExpressionNode) -> Self {
        Self {
            #[cfg(feature = "attributes")]
            attributes: Default::default(),
            expression: Some(expression),
        }
    }
}
impl From<Expression> for ReturnStatement {
    fn from(expression: Expression) -> Self {
        Self::from(ExpressionNode::from(expression))
    }
}

impl From<FunctionCall> for FunctionCallStatement {
    fn from(call: FunctionCall) -> Self {
        Self {
            #[cfg(feature = "attributes")]
            attributes: Default::default(),
            call,
        }
    }
}

// Transitive `From` implementations.
// They have to be implemented manually unfortunately.

macro_rules! impl_transitive_from {
    ($from:ident => $middle:ident => $into:ident) => {
        impl From<$from> for $into {
            fn from(value: $from) -> Self {
                $into::from($middle::from(value))
            }
        }
    };
}

impl_transitive_from!(bool => LiteralExpression => Expression);
impl_transitive_from!(i64 => LiteralExpression => Expression);
impl_transitive_from!(f64 => LiteralExpression => Expression);
impl_transitive_from!(i32 => LiteralExpression => Expression);
impl_transitive_from!(u32 => LiteralExpression => Expression);
impl_transitive_from!(f32 => LiteralExpression => Expression);
impl_transitive_from!(Ident => TypeExpression => Expression);

/// Trait implemented for all syntax node types.
///
/// This trait is useful for generic implementations over different syntax node types.
/// Node types that do not have a span, an ident, or attributes return `None`.
pub trait SyntaxNode {
    /// Span of a syntax node.
    fn span(&self) -> Option<Span> {
        None
    }

    /// Identifier, if the syntax node is a declaration.
    fn ident(&self) -> Option<Ident> {
        None
    }

    /// List all attributes of a syntax node.
    fn attributes(&self) -> &[AttributeNode] {
        &[]
    }
    /// List all attributes of a syntax node.
    fn attributes_mut(&mut self) -> &mut [AttributeNode] {
        &mut []
    }
    /// Whether the node contains an attribute.
    fn contains_attribute(&self, attribute: &Attribute) -> bool {
        self.attributes().iter().any(|v| v.node() == attribute)
    }
    /// Remove attributes with predicate.
    fn retain_attributes_mut<F>(&mut self, _predicate: F)
    where
        F: FnMut(&mut Attribute) -> bool,
    {
    }
}

impl<T: SyntaxNode> SyntaxNode for Spanned<T> {
    fn span(&self) -> Option<Span> {
        Some(self.span())
    }

    fn ident(&self) -> Option<Ident> {
        self.node().ident()
    }

    fn attributes(&self) -> &[AttributeNode] {
        self.node().attributes()
    }

    fn attributes_mut(&mut self) -> &mut [AttributeNode] {
        self.node_mut().attributes_mut()
    }

    fn retain_attributes_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Attribute) -> bool,
    {
        self.node_mut().retain_attributes_mut(|v| f(v))
    }
}

macro_rules! impl_attrs_struct {
    () => {
        fn attributes(&self) -> &[AttributeNode] {
            &self.attributes
        }
        fn attributes_mut(&mut self) -> &mut [AttributeNode] {
            &mut self.attributes
        }
        fn retain_attributes_mut<F>(&mut self, mut f: F)
        where
            F: FnMut(&mut Attribute) -> bool,
        {
            self.attributes.retain_mut(|v| f(v))
        }
    };
}

macro_rules! impl_attrs_enum {
    ($($variant: path),* $(,)?) => {
        fn attributes(&self) -> &[AttributeNode] {
            match self {
                $(
                    $variant(x) => &x.attributes,
                )*
                #[allow(unreachable_patterns)]
                _ => &[]
            }
        }
        fn attributes_mut(&mut self) -> &mut [AttributeNode] {
            match self {
                $(
                    $variant(x) => &mut x.attributes,
                )*
                #[allow(unreachable_patterns)]
                _ => &mut []
            }
        }
        fn retain_attributes_mut<F>(&mut self, mut f: F)
        where
            F: FnMut(&mut Attribute) -> bool,
        {
            match self {
                $(
                    $variant(x) => x.attributes.retain_mut(|v| f(v)),
                )*
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
    };
}

#[cfg(feature = "imports")]
impl SyntaxNode for ImportStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for GlobalDirective {
    #[cfg(feature = "attributes")]
    impl_attrs_enum! {
        GlobalDirective::Diagnostic,
        GlobalDirective::Enable,
        GlobalDirective::Requires
    }
}

impl SyntaxNode for DiagnosticDirective {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for EnableDirective {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for RequiresDirective {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for GlobalDeclaration {
    fn ident(&self) -> Option<Ident> {
        match self {
            GlobalDeclaration::Void => None,
            GlobalDeclaration::Declaration(decl) => Some(decl.ident.clone()),
            GlobalDeclaration::TypeAlias(decl) => Some(decl.ident.clone()),
            GlobalDeclaration::Struct(decl) => Some(decl.ident.clone()),
            GlobalDeclaration::Function(decl) => Some(decl.ident.clone()),
            GlobalDeclaration::ConstAssert(_) => None,
        }
    }

    #[cfg(feature = "attributes")]
    impl_attrs_enum! {
        GlobalDeclaration::Declaration,
        GlobalDeclaration::TypeAlias,
        GlobalDeclaration::Struct,
        GlobalDeclaration::Function,
        GlobalDeclaration::ConstAssert,
    }
}

impl SyntaxNode for Declaration {
    fn ident(&self) -> Option<Ident> {
        Some(self.ident.clone())
    }

    impl_attrs_struct! {}
}

impl SyntaxNode for TypeAlias {
    fn ident(&self) -> Option<Ident> {
        Some(self.ident.clone())
    }

    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for Struct {
    fn ident(&self) -> Option<Ident> {
        Some(self.ident.clone())
    }

    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for StructMember {
    fn ident(&self) -> Option<Ident> {
        Some(self.ident.clone())
    }

    impl_attrs_struct! {}
}

impl SyntaxNode for Function {
    fn ident(&self) -> Option<Ident> {
        Some(self.ident.clone())
    }

    impl_attrs_struct! {}
}

impl SyntaxNode for FormalParameter {
    fn ident(&self) -> Option<Ident> {
        Some(self.ident.clone())
    }

    impl_attrs_struct! {}
}

impl SyntaxNode for ConstAssert {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for Expression {}
impl SyntaxNode for LiteralExpression {}
impl SyntaxNode for ParenthesizedExpression {}
impl SyntaxNode for NamedComponentExpression {}
impl SyntaxNode for IndexingExpression {}
impl SyntaxNode for UnaryExpression {}
impl SyntaxNode for BinaryExpression {}
impl SyntaxNode for FunctionCall {}
impl SyntaxNode for TypeExpression {}

impl SyntaxNode for Statement {
    #[cfg(feature = "attributes")]
    impl_attrs_enum! {
        Statement::Compound,
        Statement::Assignment,
        Statement::Increment,
        Statement::Decrement,
        Statement::If,
        Statement::Switch,
        Statement::Loop,
        Statement::For,
        Statement::While,
        Statement::Break,
        Statement::Continue,
        Statement::Return,
        Statement::Discard,
        Statement::FunctionCall,
        Statement::ConstAssert,
        Statement::Declaration,
    }
}

impl SyntaxNode for CompoundStatement {
    impl_attrs_struct! {}
}

impl SyntaxNode for AssignmentStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for IncrementStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for DecrementStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for IfStatement {
    impl_attrs_struct! {}
}

impl SyntaxNode for IfClause {}

impl SyntaxNode for ElseIfClause {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for ElseClause {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for SwitchStatement {
    impl_attrs_struct! {}
}

impl SyntaxNode for SwitchClause {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for LoopStatement {
    impl_attrs_struct! {}
}

impl SyntaxNode for ContinuingStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for BreakIfStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for ForStatement {
    impl_attrs_struct! {}
}

impl SyntaxNode for WhileStatement {
    impl_attrs_struct! {}
}

impl SyntaxNode for BreakStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for ContinueStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for ReturnStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for DiscardStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}

impl SyntaxNode for FunctionCallStatement {
    #[cfg(feature = "attributes")]
    impl_attrs_struct! {}
}
