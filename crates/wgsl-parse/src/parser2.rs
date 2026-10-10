use parser::{ParseEntryPoint, parse_entrypoint_with_capabilities};
use syntax::{AstNode, HasAttributes, HasName, ast};

use crate::{
    Error,
    span::{Span, Spanned},
    syntax::{
        Attribute, Function, GlobalDeclaration, GlobalDirective, Ident, ImportStatement,
        TranslationUnit,
    },
};

/// Parse a string into a syntax tree ([`TranslationUnit`]).
///
/// Identical to [`TranslationUnit::from_str`].
pub fn parse_str(source: &str) -> Result<TranslationUnit, Error> {
    let parsed = syntax::Parse::new(parse_entrypoint_with_capabilities(
        source,
        ParseEntryPoint::File,
        parser::Edition::Wesl2025Unstable,
        parser::Capabilities {
            shader_int64: true,
            early_depth_test: true,
        },
    ));

    if !parsed.errors().is_empty() {
        return Err(Error {
            error: crate::error::ErrorKind::InvalidToken,
            span: crate::span::Span::new(0..0),
        });
    }

    gen_translation_unit(&parsed.tree())
}

fn gen_translation_unit(ast: &ast::SourceFile) -> Result<TranslationUnit, Error> {
    Ok(TranslationUnit {
        imports: gen_imports(ast.items())?,
        global_directives: gen_directives(ast.directives())?,
        global_declarations: gen_global_declarations(ast.items())?,
    })
}

fn gen_attributes(
    item: &dyn HasAttributes,
) -> Result<Vec<Spanned<crate::syntax::Attribute>>, Error> {
    let Some(attributes) = item.attributes() else {
        return Ok(Vec::new());
    };
    attributes
        .map(|attribute| gen_attribute(&attribute))
        .collect()
}

fn gen_attribute(attribute: &ast::Attribute) -> Result<Spanned<Attribute>, Error> {
    let result = match attribute.kind() {
        ast::AttributeKind::Align => Attribute::Align(todo!()),
        ast::AttributeKind::Binding => Attribute::Binding(todo!()),
        ast::AttributeKind::BlendSrc => todo!(),
        ast::AttributeKind::Builtin => todo!(),
        ast::AttributeKind::Const => todo!(),
        ast::AttributeKind::Diagnostic => todo!(),
        ast::AttributeKind::Group => todo!(),
        ast::AttributeKind::Id => todo!(),
        ast::AttributeKind::Interpolate => todo!(),
        ast::AttributeKind::Invariant => todo!(),
        ast::AttributeKind::Location => todo!(),
        ast::AttributeKind::MustUse => todo!(),
        ast::AttributeKind::Size => todo!(),
        ast::AttributeKind::SubgroupSize => todo!(),
        ast::AttributeKind::WorkgroupSize => todo!(),
        ast::AttributeKind::Entrypoint(entrypoint_attribute_kind) => todo!(),
        ast::AttributeKind::Conditional(conditional_attribute_kind) => todo!(),
        ast::AttributeKind::Other => todo!(),
    };

    Ok(Spanned::new(result, attribute.span()))
}

fn gen_directives(
    directives: syntax::AstChildren<ast::Directive>,
) -> Result<Vec<GlobalDirective>, Error> {
    todo!()
}

fn gen_imports(items: syntax::AstChildren<ast::Item>) -> Result<Vec<ImportStatement>, Error> {
    items
        .filter_map(|item| match item {
            ast::Item::ImportStatement(import_statement) => Some(import_statement),
            _ => None,
        })
        .map(|import_statement| gen_import_statement(&import_statement))
        .collect()
}

fn gen_import_statement(import_statement: &ast::ImportStatement) -> Result<ImportStatement, Error> {
    todo!()
}

fn gen_global_declarations(
    items: syntax::AstChildren<ast::Item>,
) -> Result<Vec<Spanned<GlobalDeclaration>>, Error> {
    items
        .filter_map(|item| gen_global_declaration(&item))
        .collect()
}

fn gen_global_declaration(item: &ast::Item) -> Option<Result<Spanned<GlobalDeclaration>, Error>> {
    let declaration = match item {
        ast::Item::ImportStatement(_) => return None,
        ast::Item::FunctionDeclaration(function_declaration) => {
            GlobalDeclaration::Function(match gen_function(function_declaration) {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            })
        }
        ast::Item::VariableDeclaration(variable_declaration) => todo!(),
        ast::Item::ConstantDeclaration(constant_declaration) => todo!(),
        ast::Item::OverrideDeclaration(override_declaration) => todo!(),
        ast::Item::TypeAliasDeclaration(type_alias_declaration) => todo!(),
        ast::Item::StructDeclaration(struct_declaration) => todo!(),
        ast::Item::AssertStatement(assert_statement) => todo!(),
        ast::Item::GlobalCompoundDeclaration(global_compound_declaration) => todo!(),
    };
    Some(Ok(Spanned::new(declaration, item.span())))
}

fn gen_function(function_declaration: &ast::FunctionDeclaration) -> Result<Function, Error> {
    Ok(Function {
        attributes: gen_attributes(function_declaration)?,
        visibility: todo!(),
        ident: gen_ident(function_declaration.name(), function_declaration)?,
        parameters: todo!(),
        return_attributes: todo!(),
        return_type: todo!(),
        body: todo!(),
    })
}

trait WithSpan {
    fn span(&self) -> Span;
}
impl<T: ?Sized + AstNode> WithSpan for T {
    fn span(&self) -> Span {
        let range = self.syntax().text_range();
        let start = usize::from(range.start());
        let end = usize::from(range.end());
        Span { start, end }
    }
}

fn gen_ident(name: Option<ast::Name>, node: &dyn AstNode) -> Result<Ident, Error> {
    match name {
        Some(name) => Ok(Ident::new(name.text().to_owned())),
        None => Err(Error {
            error: crate::error::ErrorKind::InvalidToken,
            span: node.span(),
        }),
    }
}
