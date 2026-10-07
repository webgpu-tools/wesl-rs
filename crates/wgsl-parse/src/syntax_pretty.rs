//! Converts the syntax tree to [`pretty`] documents, which are rendered to text.
//!
//! Every spanned node annotates its document with its span, so that rendering can tell where the
//! text of each node ended up. [`Display`] renders the documents and ignores the annotations.

use std::fmt::{self, Display};

use pretty::{Arena, DocAllocator, DocBuilder, Pretty};

use crate::{
    span::{Span, Spanned},
    syntax::*,
};

/// The allocator of the documents of the syntax tree, whose annotations are spans.
pub type Alloc<'a> = Arena<'a, Span>;

/// A document of the syntax tree.
pub type Doc<'a> = DocBuilder<'a, Alloc<'a>, Span>;

/// A syntax node that can be converted to a document.
pub trait ToDoc {
    /// Builds the document of the node in `a`.
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a>;
}

impl<T: ToDoc + ?Sized> ToDoc for &T {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        (**self).to_doc(a)
    }
}

impl<T: ToDoc> ToDoc for Spanned<T> {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.node().to_doc(a).annotate(self.span())
    }
}

impl<'a, T: ToDoc> Pretty<'a, Alloc<'a>, Span> for &Spanned<T> {
    fn pretty(self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.to_doc(a)
    }
}

impl<T: ToDoc> Display for Spanned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render_fmt(self, f)
    }
}

/// Renders the document of `node` to `out` without any line breaking other than hard lines.
pub(crate) fn render_fmt<T: ToDoc + ?Sized>(node: &T, out: &mut dyn fmt::Write) -> fmt::Result {
    let arena = Alloc::new();
    node.to_doc(&arena).into_doc().render_fmt(usize::MAX, out)
}

/// Implements [`Pretty`] and [`Display`] for the nodes that are not wrapped in a [`Spanned`].
macro_rules! impl_traits {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl<'a> Pretty<'a, Alloc<'a>, Span> for &$ty {
                fn pretty(self, a: &'a Alloc<'a>) -> Doc<'a> {
                    self.to_doc(a)
                }
            }

            impl Display for $ty {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    render_fmt(self, f)
                }
            }
        )+
    };
}

/// Returns the documents of `items` separated by `separator`.
fn join<'a>(
    a: &'a Alloc<'a>,
    items: impl IntoIterator<Item: ToDoc>,
    separator: Doc<'a>,
) -> Doc<'a> {
    a.intersperse(items.into_iter().map(|item| item.to_doc(a)), separator)
}

/// Returns the documents of `items` separated by commas.
fn comma_separated<'a>(a: &'a Alloc<'a>, items: impl IntoIterator<Item: ToDoc>) -> Doc<'a> {
    join(a, items, a.text(", "))
}

/// Returns `items`, separated by `separator`, with every line indented by four spaces.
fn indent<'a>(a: &'a Alloc<'a>, items: Vec<Doc<'a>>, separator: Doc<'a>) -> Doc<'a> {
    if items.is_empty() {
        a.nil()
    } else {
        a.text("    ")
            .append(a.intersperse(items, separator))
            .nest(4)
    }
}

/// Returns a block of `{` and `}` with the `statements` indented one per line.
fn block<'a>(a: &'a Alloc<'a>, statements: Vec<Doc<'a>>) -> Doc<'a> {
    a.text("{")
        .append(a.hardline())
        .append(indent(a, statements, a.hardline()))
        .append(a.hardline())
        .append(a.text("}"))
}

/// Returns the attributes, followed by a space if they are `inline` and by a new line otherwise.
fn attributes<'a>(a: &'a Alloc<'a>, attributes: &[AttributeNode], inline: bool) -> Doc<'a> {
    let suffix = match (attributes.is_empty(), inline) {
        (true, _) => a.nil(),
        (false, true) => a.text(" "),
        (false, false) => a.hardline(),
    };
    join(a, attributes, a.text(" ")).append(suffix)
}

/// Returns the keyword of `visibility` followed by a space, if it is written.
fn visibility<'a>(a: &'a Alloc<'a>, visibility: Visibility) -> Doc<'a> {
    match visibility {
        Visibility::Public => a.text("public "),
        Visibility::Package => a.nil(),
        Visibility::Private => a.text("private "),
    }
}

/// Returns the attributes and the visibility that start a declaration.
fn prefix<'a>(a: &'a Alloc<'a>, attrs: &[AttributeNode], vis: Visibility) -> Doc<'a> {
    attributes(a, attrs, false).append(visibility(a, vis))
}

/// Returns the template arguments of a type, if there are any.
fn template<'a>(a: &'a Alloc<'a>, args: &Option<Vec<TemplateArg>>) -> Doc<'a> {
    match args {
        Some(args) => a
            .text("<")
            .append(comma_separated(a, args))
            .append(a.text(">")),
        None => a.nil(),
    }
}

/// Returns `name(argument)`.
fn call<'a>(a: &'a Alloc<'a>, name: &'static str, argument: Doc<'a>) -> Doc<'a> {
    a.text(name)
        .append(a.text("("))
        .append(argument)
        .append(a.text(")"))
}

/// Returns the semicolon that ends a statement, if `semicolon` is set.
fn end<'a>(a: &'a Alloc<'a>, semicolon: bool) -> Doc<'a> {
    if semicolon {
        a.text(";")
    } else {
        a.nil()
    }
}

/// Returns the statements of a body, without the empty ones.
fn statements<'a>(a: &'a Alloc<'a>, body: &CompoundStatement) -> Vec<Doc<'a>> {
    body.statements
        .iter()
        .filter(|stmt| !matches!(stmt.node(), Statement::Void))
        .map(|stmt| stmt.to_doc(a))
        .collect()
}

/// Returns a statement of the header of a `for` loop, which has no semicolon.
fn header<'a>(a: &'a Alloc<'a>, statement: &StatementNode) -> Doc<'a> {
    let doc = match statement.node() {
        Statement::Void => a.nil(),
        Statement::Assignment(s) => s.doc(a, false),
        Statement::Increment(s) => s.doc(a, false),
        Statement::Decrement(s) => s.doc(a, false),
        Statement::FunctionCall(s) => s.doc(a, false),
        Statement::Declaration(s) => s.doc(a, false),
        other => other.to_doc(a),
    };
    doc.annotate(statement.span())
}

impl ToDoc for TranslationUnit {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let imports = self.imports.iter().fold(a.nil(), |doc, import| {
            doc.append(import.to_doc(a))
                .append(a.hardline())
                .append(a.hardline())
        });
        let directives = if self.global_directives.is_empty() {
            a.nil()
        } else {
            join(a, &self.global_directives, a.hardline())
                .append(a.hardline())
                .append(a.hardline())
        };
        let declarations = self
            .global_declarations
            .iter()
            .filter(|decl| !matches!(decl.node(), GlobalDeclaration::Void))
            .map(|decl| decl.to_doc(a));
        let declarations = a.intersperse(declarations, a.hardline().append(a.hardline()));
        imports
            .append(directives)
            .append(declarations)
            .append(a.hardline())
    }
}

impl ToDoc for Ident {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        a.as_string(self.name())
    }
}

impl ToDoc for Visibility {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            Visibility::Private => a.text("private"),
            Visibility::Package => a.text("package"),
            Visibility::Public => a.text("public"),
        }
    }
}

impl ToDoc for ImportStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let path = match &self.path {
            Some(path) => path.to_doc(a).append(a.text("::")),
            None => a.nil(),
        };
        prefix(a, &self.attributes, self.visibility)
            .append(a.text("import "))
            .append(path)
            .append(self.content.to_doc(a))
            .append(a.text(";"))
    }
}

impl ToDoc for ModulePath {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let origin = match &self.origin {
            PathOrigin::Absolute => "package".to_string(),
            PathOrigin::Relative(0) => "self".to_string(),
            PathOrigin::Relative(n) => vec!["super"; *n].join("::"),
            PathOrigin::Package(p) => p.to_string(),
        };
        let components = self
            .components
            .iter()
            .fold(String::new(), |path, name| format!("{path}::{name}"));
        a.text(origin + &components)
    }
}

impl ToDoc for Import {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let path = self.path.iter().fold(a.nil(), |doc, name| {
            doc.append(a.as_string(name)).append(a.text("::"))
        });
        path.append(self.content.to_doc(a))
    }
}

impl ToDoc for ImportContent {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            ImportContent::Item(item) => {
                let rename = match &item.rename {
                    Some(rename) => a.text(" as ").append(rename.to_doc(a)),
                    None => a.nil(),
                };
                item.ident.to_doc(a).append(rename)
            }
            ImportContent::Collection(imports) => a
                .text("{ ")
                .append(comma_separated(a, imports))
                .append(a.text(" }")),
        }
    }
}

impl ToDoc for GlobalDirective {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            GlobalDirective::Diagnostic(print) => print.to_doc(a),
            GlobalDirective::Enable(print) => print.to_doc(a),
            GlobalDirective::Requires(print) => print.to_doc(a),
        }
    }
}

impl ToDoc for DiagnosticDirective {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false).append(a.text(format!(
            "diagnostic ({}, {});",
            self.severity, self.rule_name
        )))
    }
}

impl ToDoc for EnableDirective {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("enable "))
            .append(a.intersperse(
                self.extensions.iter().map(|ext| a.text(ext.clone())),
                a.text(", "),
            ))
            .append(a.text(";"))
    }
}

impl ToDoc for RequiresDirective {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("requires "))
            .append(a.intersperse(
                self.extensions.iter().map(|ext| a.text(ext.clone())),
                a.text(", "),
            ))
            .append(a.text(";"))
    }
}

impl ToDoc for GlobalDeclaration {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            GlobalDeclaration::Void => a.text(";"),
            GlobalDeclaration::Declaration(print) => print.to_doc(a),
            GlobalDeclaration::TypeAlias(print) => print.to_doc(a),
            GlobalDeclaration::Struct(print) => print.to_doc(a),
            GlobalDeclaration::Function(print) => print.to_doc(a),
            GlobalDeclaration::ConstAssert(print) => print.to_doc(a),
            GlobalDeclaration::Compound(print) => print.to_doc(a),
        }
    }
}

impl Declaration {
    /// Builds the document of the declaration, which ends with a semicolon if `semicolon` is set.
    fn doc<'a>(&self, a: &'a Alloc<'a>, semicolon: bool) -> Doc<'a> {
        let ty = match &self.ty {
            Some(ty) => a.text(": ").append(ty.to_doc(a)),
            None => a.nil(),
        };
        let initializer = match &self.initializer {
            Some(init) => a.text(" = ").append(init.to_doc(a)),
            None => a.nil(),
        };
        prefix(a, &self.attributes, self.visibility)
            .append(self.kind.to_doc(a))
            .append(a.text(" "))
            .append(self.ident.to_doc(a))
            .append(ty)
            .append(initializer)
            .append(end(a, semicolon))
    }
}

impl ToDoc for Declaration {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.doc(a, true)
    }
}

impl ToDoc for DeclarationKind {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            Self::Const => a.text("const"),
            Self::Override => a.text("override"),
            Self::Let => a.text("let"),
            Self::Var(None) => a.text("var"),
            Self::Var(Some((a_s, None))) => a.text(format!("var<{a_s}>")),
            Self::Var(Some((a_s, Some(a_m)))) => a.text(format!("var<{a_s}, {a_m}>")),
        }
    }
}

impl ToDoc for TypeAlias {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        prefix(a, &self.attributes, self.visibility)
            .append(a.text("alias "))
            .append(self.ident.to_doc(a))
            .append(a.text(" = "))
            .append(self.ty.to_doc(a))
            .append(a.text(";"))
    }
}

impl ToDoc for Struct {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let members = self.members.iter().map(|member| member.to_doc(a)).collect();
        prefix(a, &self.attributes, self.visibility)
            .append(a.text("struct "))
            .append(self.ident.to_doc(a))
            .append(a.text(" {"))
            .append(a.hardline())
            .append(indent(a, members, a.text(",").append(a.hardline())))
            .append(a.hardline())
            .append(a.text("}"))
    }
}

impl ToDoc for StructMember {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(self.ident.to_doc(a))
            .append(a.text(": "))
            .append(self.ty.to_doc(a))
    }
}

impl ToDoc for Function {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let return_type = match &self.return_type {
            Some(ty) => a
                .text("-> ")
                .append(attributes(a, &self.return_attributes, true))
                .append(ty.to_doc(a))
                .append(a.text(" ")),
            None => a.nil(),
        };
        prefix(a, &self.attributes, self.visibility)
            .append(a.text("fn "))
            .append(self.ident.to_doc(a))
            .append(a.text("("))
            .append(comma_separated(a, &self.parameters))
            .append(a.text(") "))
            .append(return_type)
            .append(self.body.to_doc(a))
    }
}

impl ToDoc for FormalParameter {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, true)
            .append(self.ident.to_doc(a))
            .append(a.text(": "))
            .append(self.ty.to_doc(a))
    }
}

impl ConstAssert {
    /// Builds the document of the assertion, which ends with a semicolon if `semicolon` is set.
    fn doc<'a>(&self, a: &'a Alloc<'a>, semicolon: bool) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("const_assert "))
            .append(self.expression.to_doc(a))
            .append(end(a, semicolon))
    }
}

impl ToDoc for ConstAssert {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.doc(a, true)
    }
}

impl ToDoc for CompoundGlobalDeclaration {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let body = self.body.iter().map(|decl| decl.to_doc(a)).collect();
        attributes(a, &self.attributes, false).append(block(a, body))
    }
}

impl ToDoc for Attribute {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            Attribute::Align(e1) => call(a, "@align", e1.to_doc(a)),
            Attribute::Binding(e1) => call(a, "@binding", e1.to_doc(a)),
            Attribute::BlendSrc(e1) => call(a, "@blend_src", e1.to_doc(a)),
            Attribute::Builtin(e1) => call(a, "@builtin", a.as_string(e1)),
            Attribute::Const => a.text("@const"),
            Attribute::Diagnostic(DiagnosticAttribute { severity, rule }) => {
                call(a, "@diagnostic", a.text(format!("{severity}, {rule}")))
            }
            Attribute::Group(e1) => call(a, "@group", e1.to_doc(a)),
            Attribute::Id(e1) => call(a, "@id", e1.to_doc(a)),
            Attribute::Interpolate(InterpolateAttribute { ty, sampling }) => {
                let args = match sampling {
                    Some(sampling) => format!("{ty}, {sampling}"),
                    None => ty.to_string(),
                };
                call(a, "@interpolate", a.text(args))
            }
            Attribute::Invariant => a.text("@invariant"),
            Attribute::Location(e1) => call(a, "@location", e1.to_doc(a)),
            Attribute::MustUse => a.text("@must_use"),
            Attribute::Size(e1) => call(a, "@size", e1.to_doc(a)),
            Attribute::WorkgroupSize(WorkgroupSizeAttribute { x, y, z }) => call(
                a,
                "@workgroup_size",
                comma_separated(a, std::iter::once(x).chain(y).chain(z)),
            ),
            Attribute::Vertex => a.text("@vertex"),
            Attribute::Fragment => a.text("@fragment"),
            Attribute::Compute => a.text("@compute"),

            // wesl extensions
            Attribute::If(e1) => call(a, "@if", e1.to_doc(a)),
            Attribute::Elif(e1) => call(a, "@elif", e1.to_doc(a)),
            Attribute::Else => a.text("@else"),
            #[cfg(feature = "generics")]
            Attribute::Type(e1) => call(a, "@type", e1.to_doc(a)),

            // naga extensions
            #[cfg(feature = "naga-ext")]
            Attribute::Task => a.text("@task"),
            #[cfg(feature = "naga-ext")]
            Attribute::Payload(p) => call(a, "@payload", p.to_doc(a)),
            #[cfg(feature = "naga-ext")]
            Attribute::Mesh(m) => call(a, "@mesh", m.to_doc(a)),
            #[cfg(feature = "naga-ext")]
            Attribute::RayGeneration => a.text("@ray_generation"),
            #[cfg(feature = "naga-ext")]
            Attribute::AnyHit => a.text("@any_hit"),
            #[cfg(feature = "naga-ext")]
            Attribute::ClosestHit => a.text("@closest_hit"),
            #[cfg(feature = "naga-ext")]
            Attribute::Miss => a.text("@miss"),
            #[cfg(feature = "naga-ext")]
            Attribute::IncomingPayload(p) => call(a, "@incoming_payload", p.to_doc(a)),
            #[cfg(feature = "naga-ext")]
            Attribute::EarlyDepthTest(None) => a.text("@early_depth_test"),
            #[cfg(feature = "naga-ext")]
            Attribute::EarlyDepthTest(Some(e1)) => call(a, "@early_depth_test", a.as_string(e1)),
            Attribute::Custom(custom) => {
                let arguments = match &custom.arguments {
                    Some(args) => a
                        .text("(")
                        .append(comma_separated(a, args))
                        .append(a.text(")")),
                    None => a.nil(),
                };
                a.text(format!("@{}", custom.name)).append(arguments)
            }
        }
    }
}

#[cfg(feature = "generics")]
impl ToDoc for TypeConstraint {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.ident
            .to_doc(a)
            .append(a.text(", "))
            .append(join(a, &self.variants, a.text(" | ")))
    }
}

impl ToDoc for Expression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            Expression::Literal(print) => print.to_doc(a),
            Expression::Parenthesized(print) => print.to_doc(a),
            Expression::NamedComponent(print) => print.to_doc(a),
            Expression::Indexing(print) => print.to_doc(a),
            Expression::Unary(print) => print.to_doc(a),
            Expression::Binary(print) => print.to_doc(a),
            Expression::FunctionCall(print) => print.to_doc(a),
            Expression::TypeOrIdentifier(print) => print.to_doc(a),
        }
    }
}

impl ToDoc for LiteralExpression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            LiteralExpression::Bool(true) => a.text("true"),
            LiteralExpression::Bool(false) => a.text("false"),
            LiteralExpression::AbstractInt(num) => a.as_string(num),
            // the debug format keeps the trailing `.0` of floats that represent integers
            LiteralExpression::AbstractFloat(num) => a.text(format!("{num:?}")),
            LiteralExpression::I32(num) => a.text(format!("{num}i")),
            LiteralExpression::U32(num) => a.text(format!("{num}u")),
            LiteralExpression::F32(num) => a.text(format!("{num}f")),
            LiteralExpression::F16(num) => a.text(format!("{num}h")),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::I64(num) => a.text(format!("{num}li")),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::U64(num) => a.text(format!("{num}lu")),
            #[cfg(feature = "naga-ext")]
            LiteralExpression::F64(num) => a.text(format!("{num}lf")),
        }
    }
}

impl ToDoc for ParenthesizedExpression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        a.text("(")
            .append(self.expression.to_doc(a))
            .append(a.text(")"))
    }
}

impl ToDoc for NamedComponentExpression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.base
            .to_doc(a)
            .append(a.text("."))
            .append(self.component.to_doc(a))
    }
}

impl ToDoc for IndexingExpression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.base
            .to_doc(a)
            .append(a.text("["))
            .append(self.index.to_doc(a))
            .append(a.text("]"))
    }
}

impl ToDoc for UnaryExpression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        a.as_string(self.operator).append(self.operand.to_doc(a))
    }
}

impl ToDoc for BinaryExpression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.left
            .to_doc(a)
            .append(a.text(format!(" {} ", self.operator)))
            .append(self.right.to_doc(a))
    }
}

impl ToDoc for FunctionCall {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.ty
            .to_doc(a)
            .append(a.text("("))
            .append(comma_separated(a, &self.arguments))
            .append(a.text(")"))
    }
}

impl ToDoc for TypeExpression {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let path = match &self.path {
            Some(path) => path.to_doc(a).append(a.text("::")),
            None => a.nil(),
        };
        path.append(self.ident.to_doc(a))
            .append(template(a, &self.template_args))
    }
}

impl ToDoc for TemplateArg {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.expression.to_doc(a)
    }
}

impl ToDoc for Statement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            Statement::Void => a.text(";"),
            Statement::Compound(print) => print.to_doc(a),
            Statement::Assignment(print) => print.to_doc(a),
            Statement::Increment(print) => print.to_doc(a),
            Statement::Decrement(print) => print.to_doc(a),
            Statement::If(print) => print.to_doc(a),
            Statement::Switch(print) => print.to_doc(a),
            Statement::Loop(print) => print.to_doc(a),
            Statement::For(print) => print.to_doc(a),
            Statement::While(print) => print.to_doc(a),
            Statement::Break(print) => print.to_doc(a),
            Statement::Continue(print) => print.to_doc(a),
            Statement::Return(print) => print.to_doc(a),
            Statement::Discard(print) => print.to_doc(a),
            Statement::FunctionCall(print) => print.to_doc(a),
            Statement::ConstAssert(print) => print.to_doc(a),
            Statement::Declaration(print) => print.to_doc(a),
        }
    }
}

impl ToDoc for CompoundStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false).append(block(a, statements(a, self)))
    }
}

impl AssignmentStatement {
    /// Builds the document of the assignment, which ends with a semicolon if `semicolon` is set.
    fn doc<'a>(&self, a: &'a Alloc<'a>, semicolon: bool) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(self.lhs.to_doc(a))
            .append(a.text(format!(" {} ", self.operator)))
            .append(self.rhs.to_doc(a))
            .append(end(a, semicolon))
    }
}

impl ToDoc for AssignmentStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.doc(a, true)
    }
}

impl IncrementStatement {
    /// Builds the document of the increment, which ends with a semicolon if `semicolon` is set.
    fn doc<'a>(&self, a: &'a Alloc<'a>, semicolon: bool) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(self.expression.to_doc(a))
            .append(a.text("++"))
            .append(end(a, semicolon))
    }
}

impl ToDoc for IncrementStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.doc(a, true)
    }
}

impl DecrementStatement {
    /// Builds the document of the decrement, which ends with a semicolon if `semicolon` is set.
    fn doc<'a>(&self, a: &'a Alloc<'a>, semicolon: bool) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(self.expression.to_doc(a))
            .append(a.text("--"))
            .append(end(a, semicolon))
    }
}

impl ToDoc for DecrementStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.doc(a, true)
    }
}

impl ToDoc for IfStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let else_if_clauses = self.else_if_clauses.iter().fold(a.nil(), |doc, clause| {
            doc.append(a.hardline()).append(clause.to_doc(a))
        });
        let else_clause = match &self.else_clause {
            Some(clause) => a.hardline().append(clause.to_doc(a)),
            None => a.nil(),
        };
        attributes(a, &self.attributes, false)
            .append(self.if_clause.to_doc(a))
            .append(else_if_clauses)
            .append(else_clause)
    }
}

impl ToDoc for IfClause {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        a.text("if ")
            .append(self.expression.to_doc(a))
            .append(a.text(" "))
            .append(self.body.to_doc(a))
    }
}

impl ToDoc for ElseIfClause {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("else if "))
            .append(self.expression.to_doc(a))
            .append(a.text(" "))
            .append(self.body.to_doc(a))
    }
}

impl ToDoc for ElseClause {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("else "))
            .append(self.body.to_doc(a))
    }
}

impl ToDoc for SwitchStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let clauses = self.clauses.iter().map(|clause| clause.to_doc(a)).collect();
        attributes(a, &self.attributes, false)
            .append(a.text("switch "))
            .append(self.expression.to_doc(a))
            .append(a.text(" "))
            .append(attributes(a, &self.body_attributes, false))
            .append(block(a, clauses))
    }
}

impl ToDoc for SwitchClause {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("case "))
            .append(comma_separated(a, &self.case_selectors))
            .append(a.text(" "))
            .append(self.body.to_doc(a))
    }
}

impl ToDoc for CaseSelector {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        match self {
            CaseSelector::Default => a.text("default"),
            CaseSelector::Expression(expr) => expr.to_doc(a),
        }
    }
}

impl ToDoc for LoopStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let continuing = match &self.continuing {
            Some(continuing) => {
                indent(a, vec![continuing.to_doc(a)], a.hardline()).append(a.hardline())
            }
            None => a.nil(),
        };
        attributes(a, &self.attributes, false)
            .append(a.text("loop "))
            .append(attributes(a, &self.body.attributes, false))
            .append(a.text("{"))
            .append(a.hardline())
            .append(indent(a, statements(a, &self.body), a.hardline()))
            .append(a.hardline())
            .append(continuing)
            .append(a.text("}"))
    }
}

impl ToDoc for ContinuingStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let break_if = match &self.break_if {
            Some(break_if) => {
                indent(a, vec![break_if.to_doc(a)], a.hardline()).append(a.hardline())
            }
            None => a.nil(),
        };
        attributes(a, &self.attributes, false)
            .append(a.text("continuing "))
            .append(attributes(a, &self.body.attributes, false))
            .append(a.text("{"))
            .append(a.hardline())
            .append(indent(a, statements(a, &self.body), a.hardline()))
            .append(a.hardline())
            .append(break_if)
            .append(a.text("}"))
    }
}

impl ToDoc for BreakIfStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("break if "))
            .append(self.expression.to_doc(a))
            .append(a.text(";"))
    }
}

impl ToDoc for ForStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let initializer = self
            .initializer
            .as_ref()
            .map_or_else(|| a.nil(), |stmt| header(a, stmt));
        let condition = self
            .condition
            .as_ref()
            .map_or_else(|| a.nil(), |expr| expr.to_doc(a));
        let update = self
            .update
            .as_ref()
            .map_or_else(|| a.nil(), |stmt| header(a, stmt));
        attributes(a, &self.attributes, false)
            .append(a.text("for ("))
            .append(initializer)
            .append(a.text("; "))
            .append(condition)
            .append(a.text("; "))
            .append(update)
            .append(a.text(") "))
            .append(self.body.to_doc(a))
    }
}

impl ToDoc for WhileStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(a.text("while "))
            .append(self.condition.to_doc(a))
            .append(a.text(" "))
            .append(self.body.to_doc(a))
    }
}

impl ToDoc for BreakStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false).append(a.text("break;"))
    }
}

impl ToDoc for ContinueStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false).append(a.text("continue;"))
    }
}

impl ToDoc for ReturnStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        let expression = match &self.expression {
            Some(expr) => a.text(" ").append(expr.to_doc(a)),
            None => a.nil(),
        };
        attributes(a, &self.attributes, false)
            .append(a.text("return"))
            .append(expression)
            .append(a.text(";"))
    }
}

impl ToDoc for DiscardStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        attributes(a, &self.attributes, false).append(a.text("discard;"))
    }
}

impl FunctionCallStatement {
    /// Builds the document of the call, which ends with a semicolon if `semicolon` is set.
    fn doc<'a>(&self, a: &'a Alloc<'a>, semicolon: bool) -> Doc<'a> {
        attributes(a, &self.attributes, false)
            .append(self.call.to_doc(a))
            .append(end(a, semicolon))
    }
}

impl ToDoc for FunctionCallStatement {
    fn to_doc<'a>(&self, a: &'a Alloc<'a>) -> Doc<'a> {
        self.doc(a, true)
    }
}

impl_traits!(
    TranslationUnit,
    Ident,
    Visibility,
    ImportStatement,
    ModulePath,
    Import,
    ImportContent,
    GlobalDirective,
    DiagnosticDirective,
    EnableDirective,
    RequiresDirective,
    GlobalDeclaration,
    Declaration,
    DeclarationKind,
    TypeAlias,
    Struct,
    StructMember,
    Function,
    FormalParameter,
    ConstAssert,
    CompoundGlobalDeclaration,
    Attribute,
    Expression,
    LiteralExpression,
    ParenthesizedExpression,
    NamedComponentExpression,
    IndexingExpression,
    UnaryExpression,
    BinaryExpression,
    FunctionCall,
    TypeExpression,
    TemplateArg,
    Statement,
    CompoundStatement,
    AssignmentStatement,
    IncrementStatement,
    DecrementStatement,
    IfStatement,
    IfClause,
    ElseIfClause,
    ElseClause,
    SwitchStatement,
    SwitchClause,
    CaseSelector,
    LoopStatement,
    ContinuingStatement,
    BreakIfStatement,
    ForStatement,
    WhileStatement,
    BreakStatement,
    ContinueStatement,
    ReturnStatement,
    DiscardStatement,
    FunctionCallStatement,
);

#[cfg(feature = "generics")]
impl_traits!(TypeConstraint);

#[cfg(test)]
mod test {
    use crate::syntax::ModulePath;
    use crate::syntax::{Ident, TypeExpression};

    #[test]
    fn type_expression_display() {
        let expr = TypeExpression {
            path: None,
            ident: Ident::new("foo".into()),
            template_args: None,
        };

        assert_eq!(expr.to_string(), "foo");

        let expr = TypeExpression {
            path: Some(ModulePath::new(
                crate::syntax::PathOrigin::Absolute,
                vec!["bar".into(), "qux".into()],
            )),
            ident: Ident::new("foo".into()),
            template_args: None,
        };

        assert_eq!(expr.to_string(), "package::bar::qux::foo");
    }
}
