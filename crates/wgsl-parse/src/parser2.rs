//! Converts wgsl-analyzer syntax tree into a strongly typed syntax tree.
//! Unwrapping is used, since we bail out on parser errors immediately.
use std::str::FromStr;

use syntax::{AstNode, HasAttributes, HasName, HasTemplateParameters, ast};
use wgsl_types::syntax::{AssignmentOperator, DiagnosticSeverity, UnaryOperator};

use crate::{
    Error,
    error::ErrorKind,
    parser_support::{parse_attribute, parse_var_template},
    span::{Span, Spanned},
    syntax::{
        AssignmentStatement, Attribute, BinaryExpression, BreakIfStatement, BreakStatement,
        CaseSelector, CompoundGlobalDeclaration, CompoundStatement, ConstAssert, ContinueStatement,
        ContinuingStatement, CustomAttribute, Declaration, DeclarationKind, DiagnosticDirective,
        DiscardStatement, ElseClause, ElseIfClause, EnableDirective, Expression, ForStatement,
        FormalParameter, Function, FunctionCall, FunctionCallStatement, GlobalDeclaration,
        GlobalDirective, Ident, IfClause, IfStatement, Import, ImportContent, ImportItem,
        ImportStatement, IndexingExpression, LiteralExpression, LoopStatement, ModulePath,
        NamedComponentExpression, ParenthesizedExpression, PathOrigin, RequiresDirective,
        ReturnStatement, Statement, Struct, StructMember, SwitchClause, SwitchStatement,
        TemplateArg, TranslationUnit, TypeAlias, TypeExpression, UnaryExpression, Visibility,
        WhileStatement,
    },
};

/// Parse a string into a syntax tree ([`TranslationUnit`]).
///
/// Identical to [`TranslationUnit::from_str`].
pub fn parse_str(source: &str) -> (TranslationUnit, Diagnostics) {
    let parsed = syntax::parse(source, syntax::Edition::Wesl2025Unstable);

    if !parsed.errors().is_empty() {
        let errors = parsed
            .errors()
            .iter()
            .map(|diagnostic| {
                eprintln!("Parser error: {:?}", diagnostic);
                Error {
                    error: ErrorKind::InvalidToken,
                    span: Span::from(diagnostic.range),
                }
            })
            .collect::<Vec<_>>();
        return (TranslationUnit::default(), Diagnostics { errors });
    }

    let mut diagnostics = Diagnostics { errors: Vec::new() };

    let result = parsed.tree().gen_ast(&mut diagnostics);
    (result, diagnostics)
}

pub fn parse_str_statement(source: &str) -> (Spanned<Statement>, Diagnostics) {
    let parsed = syntax::parse_entrypoint(
        source,
        syntax::Edition::Wesl2025Unstable,
        syntax::ParseEntryPoint::Statement,
    );

    if !parsed.errors().is_empty() {
        return (
            Spanned::new(Statement::Void, Span::default()),
            Diagnostics {
                errors: [Error {
                    error: ErrorKind::InvalidToken,
                    span: Span::default(),
                }]
                .to_vec(),
            },
        );
    }

    let mut diagnostics = Diagnostics { errors: Vec::new() };

    let source_file = parsed.tree();
    let statement = source_file
        .syntax()
        .children()
        .find_map(ast::Statement::cast)
        .unwrap();
    let result = statement.gen_ast(&mut diagnostics);
    (result, diagnostics)
}

pub fn parse_str_expression(source: &str) -> (Spanned<Expression>, Diagnostics) {
    let parsed = syntax::parse_entrypoint(
        source,
        syntax::Edition::Wesl2025Unstable,
        syntax::ParseEntryPoint::Expression,
    );

    if !parsed.errors().is_empty() {
        return (
            Spanned::new(
                Expression::Literal(LiteralExpression::Bool(false)),
                Span::default(),
            ),
            Diagnostics {
                errors: [Error {
                    error: ErrorKind::InvalidToken,
                    span: Span::default(),
                }]
                .to_vec(),
            },
        );
    }

    let mut diagnostics = Diagnostics { errors: Vec::new() };

    let source_file = parsed.tree();
    let expression = source_file
        .syntax()
        .children()
        .find_map(ast::Expression::cast)
        .unwrap();
    let result = expression.gen_ast(&mut diagnostics);
    (result, diagnostics)
}

pub struct Diagnostics {
    pub errors: Vec<Error>,
}

impl ToStrictAst for ast::SourceFile {
    type Ast = TranslationUnit;
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        TranslationUnit {
            imports: gen_imports(self.items(), diagnostics),
            global_directives: self.directives().gen_ast(diagnostics),
            global_declarations: gen_global_declarations(self.items(), diagnostics),
        }
    }
}

impl ToStrictAst for Option<syntax::AstChildren<ast::Attribute>> {
    type Ast = Vec<Spanned<Attribute>>;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        match self {
            Some(children) => children.gen_ast(diagnostics),
            None => Vec::new(),
        }
    }
}

impl ToStrictAst for ast::Attribute {
    type Ast = Spanned<Attribute>;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let name = self.name().unwrap().text().to_string();
        let arguments = self
            .arguments()
            .map(|arguments| arguments.arguments().gen_ast(diagnostics));

        let attribute = match parse_attribute(name, arguments) {
            Ok(attribute) => attribute,
            Err(error) => {
                let error = if let crate::error::ParseError::Attribute(name, message) = error {
                    Error {
                        error: ErrorKind::Attribute(name, message),
                        span: self.span(),
                    }
                } else {
                    Error {
                        error: ErrorKind::InvalidToken,
                        span: self.span(),
                    }
                };
                diagnostics.errors.push(error);
                Attribute::Custom(CustomAttribute {
                    name: "wesl_parsing_error".to_string(),
                    arguments: None,
                })
            }
        };
        Spanned::new(attribute, self.span())
    }
}

impl ToStrictAst for ast::Directive {
    type Ast = GlobalDirective;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        match self {
            ast::Directive::DiagnosticDirective(diagnostic_directive) => {
                GlobalDirective::Diagnostic(diagnostic_directive.gen_ast(diagnostics))
            }
            ast::Directive::EnableDirective(enable_directive) => {
                GlobalDirective::Enable(enable_directive.gen_ast(diagnostics))
            }
            ast::Directive::RequiresDirective(requires_directive) => {
                GlobalDirective::Requires(requires_directive.gen_ast(diagnostics))
            }
        }
    }
}

impl ToStrictAst for ast::DiagnosticDirective {
    type Ast = DiagnosticDirective;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let severity = DiagnosticSeverity::from_str(
            self.parameters()
                .unwrap()
                .severity_control_name()
                .unwrap()
                .text()
                .as_str(),
        )
        .unwrap_or_else(|_| {
            diagnostics.errors.push(Error {
                error: ErrorKind::DiagnosticSeverity,
                span: self.span(),
            });
            DiagnosticSeverity::Off
        });
        let rule_name = self
            .parameters()
            .unwrap()
            .diagnostic_rule_name()
            .unwrap()
            .ident_tokens();

        let rule_name = match rule_name {
            (None, None) | (None, Some(_)) => {
                diagnostics.errors.push(Error {
                    error: ErrorKind::InvalidToken,
                    span: self.span(),
                });
                "".to_string()
            }
            (Some(rule), None) => rule.text().to_string(),
            (Some(rule_start), Some(rule_end)) => {
                format!("{}.{}", rule_start.text(), rule_end.text())
            }
        };

        DiagnosticDirective {
            attributes: self.attributes().gen_ast(diagnostics),
            severity,
            rule_name,
        }
    }
}

impl ToStrictAst for ast::EnableDirective {
    type Ast = EnableDirective;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let extensions = self
            .enable_extensions()
            .map(|extension| extension.text().to_string())
            .collect::<Vec<_>>();
        EnableDirective {
            attributes: self.attributes().gen_ast(diagnostics),
            extensions: extensions,
        }
    }
}

impl ToStrictAst for ast::RequiresDirective {
    type Ast = RequiresDirective;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let extensions = self
            .require_extensions()
            .map(|extension| extension.text().to_string())
            .collect::<Vec<_>>();
        RequiresDirective {
            attributes: self.attributes().gen_ast(diagnostics),
            extensions: extensions,
        }
    }
}

fn gen_imports(
    items: syntax::AstChildren<ast::Item>,
    diagnostics: &mut Diagnostics,
) -> Vec<ImportStatement> {
    items
        .filter_map(|item| match item {
            ast::Item::ImportStatement(import_statement) => Some(import_statement),
            _ => None,
        })
        .map(|import_statement| import_statement.gen_ast(diagnostics))
        .collect()
}

impl ToStrictAst for ast::ImportStatement {
    type Ast = ImportStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let origin = match self.relative() {
            Some(ast::ImportRelative::ImportPackageRelative(_)) => Some(PathOrigin::Absolute),
            Some(ast::ImportRelative::ImportSuperRelative(super_relative)) => Some(
                PathOrigin::Relative(usize::from(super_relative.super_count())),
            ),
            None => None,
        };

        let mut content = gen_import_tree(self.item().unwrap(), Vec::new(), diagnostics);
        let path = match origin {
            Some(origin) => Some(ModulePath {
                origin,
                components: content.path,
            }),
            None => {
                if content.path.len() > 0 {
                    Some(ModulePath {
                        origin: PathOrigin::Package(content.path.remove(0)),
                        components: content.path,
                    })
                } else {
                    // panic!("import foo as bar; and import {{ ... }}; are unsupported");
                    None
                }
            }
        };

        ImportStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: self.visibility().gen_ast(diagnostics),
            path,
            content: content.content,
        }
    }
}

fn gen_import_tree(
    tree: ast::ImportTree,
    mut path_segments: Vec<String>,
    diagnostics: &mut Diagnostics,
) -> Import {
    match tree {
        ast::ImportTree::ImportPath(import_path) => {
            path_segments.push(import_path.name().unwrap().text().to_owned());
            gen_import_tree(import_path.item().unwrap(), path_segments, diagnostics)
        }
        ast::ImportTree::ImportItem(import_item) => Import {
            path: path_segments,
            content: ImportContent::Item(ImportItem {
                ident: import_item.name().unwrap().gen_ast(diagnostics),
                rename: import_item.alias().map(|alias| alias.gen_ast(diagnostics)),
            }),
        },
        ast::ImportTree::ImportCollection(import_collection) => {
            let items = import_collection
                .items()
                .map(|item| gen_import_tree(item, Vec::new(), diagnostics))
                .collect();
            Import {
                path: path_segments,
                content: ImportContent::Collection(items),
            }
        }
    }
}

fn gen_global_declarations(
    items: syntax::AstChildren<ast::Item>,
    diagnostics: &mut Diagnostics,
) -> Vec<Spanned<GlobalDeclaration>> {
    items
        .filter(|item| match item {
            ast::Item::ImportStatement(_) => false,
            _ => true,
        })
        .map(|item| item.gen_ast(diagnostics))
        .collect()
}

impl ToStrictAst for ast::Item {
    type Ast = Spanned<GlobalDeclaration>;

    /// # Panics
    ///
    /// Panics if the given item is not a global declaration (an import statement).
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let span = self.span();
        let declaration = match self {
            ast::Item::ImportStatement(_) => panic!("Not a global declaration"),
            ast::Item::FunctionDeclaration(function_declaration) => {
                GlobalDeclaration::Function(function_declaration.gen_ast(diagnostics))
            }
            ast::Item::VariableDeclaration(variable_declaration) => {
                GlobalDeclaration::Declaration(variable_declaration.gen_ast(diagnostics))
            }
            ast::Item::ConstantDeclaration(constant_declaration) => {
                GlobalDeclaration::Declaration(constant_declaration.gen_ast(diagnostics))
            }
            ast::Item::OverrideDeclaration(override_declaration) => {
                GlobalDeclaration::Declaration(override_declaration.gen_ast(diagnostics))
            }
            ast::Item::TypeAliasDeclaration(type_alias_declaration) => {
                GlobalDeclaration::TypeAlias(type_alias_declaration.gen_ast(diagnostics))
            }
            ast::Item::StructDeclaration(struct_declaration) => {
                GlobalDeclaration::Struct(struct_declaration.gen_ast(diagnostics))
            }
            ast::Item::AssertStatement(assert_statement) => {
                GlobalDeclaration::ConstAssert(assert_statement.gen_ast(diagnostics))
            }
            ast::Item::GlobalCompoundDeclaration(global_compound_declaration) => {
                GlobalDeclaration::Compound(global_compound_declaration.gen_ast(diagnostics))
            }
        };
        Spanned::new(declaration, span)
    }
}

impl ToStrictAst for ast::FunctionDeclaration {
    type Ast = Function;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let return_attributes = self
            .return_type()
            .map(|rt| rt.attributes().gen_ast(diagnostics))
            .unwrap_or_default();
        let return_type = self
            .return_type()
            .map(|rt| rt.r#type().unwrap().gen_ast(diagnostics));
        Function {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: self.visibility().gen_ast(diagnostics),
            ident: self.name().unwrap().gen_ast(diagnostics),
            parameters: self
                .parameter_list()
                .unwrap()
                .parameters()
                .gen_ast(diagnostics),
            return_attributes,
            return_type,
            body: self.body().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::Parameter {
    type Ast = FormalParameter;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        FormalParameter {
            attributes: self.attributes().gen_ast(diagnostics),
            ident: self.name().unwrap().gen_ast(diagnostics),
            ty: self.r#type().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for Option<ast::Visibility> {
    type Ast = Visibility;

    fn gen_ast(self, _diagnostics: &mut Diagnostics) -> Self::Ast {
        match self {
            Some(visibility) => match visibility.kind().unwrap() {
                ast::VisibilityKind::Public => Visibility::Public,
                ast::VisibilityKind::Private => Visibility::Private,
            },
            None => Visibility::Package,
        }
    }
}

impl ToStrictAst for ast::VariableDeclaration {
    type Ast = Declaration;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let var_template = parse_var_template(self.template_parameters().gen_ast(diagnostics))
            .unwrap_or_else(|_| {
                diagnostics.errors.push(Error {
                    error: ErrorKind::InvalidToken,
                    span: self.span(),
                });
                None
            });
        Declaration {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: self.visibility().gen_ast(diagnostics),
            kind: DeclarationKind::Var(var_template),
            ident: self.name().unwrap().gen_ast(diagnostics),
            ty: self.r#type().map(|ty| ty.gen_ast(diagnostics)),
            initializer: self.init().map(|init| init.gen_ast(diagnostics)),
        }
    }
}

impl ToStrictAst for ast::LetDeclaration {
    type Ast = Declaration;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        Declaration {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: Visibility::Package,
            kind: DeclarationKind::Let,
            ident: self.name().unwrap().gen_ast(diagnostics),
            ty: self.r#type().map(|ty| ty.gen_ast(diagnostics)),
            initializer: self.init().map(|init| init.gen_ast(diagnostics)),
        }
    }
}

impl ToStrictAst for ast::ConstantDeclaration {
    type Ast = Declaration;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        Declaration {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: self.visibility().gen_ast(diagnostics),
            kind: DeclarationKind::Const,
            ident: self.name().unwrap().gen_ast(diagnostics),
            ty: self.r#type().map(|ty| ty.gen_ast(diagnostics)),
            initializer: self.init().map(|init| init.gen_ast(diagnostics)),
        }
    }
}

impl ToStrictAst for ast::OverrideDeclaration {
    type Ast = Declaration;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        Declaration {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: self.visibility().gen_ast(diagnostics),
            kind: DeclarationKind::Override,
            ident: self.name().unwrap().gen_ast(diagnostics),
            ty: self.r#type().map(|ty| ty.gen_ast(diagnostics)),
            initializer: self.init().map(|init| init.gen_ast(diagnostics)),
        }
    }
}
impl ToStrictAst for ast::TypeAliasDeclaration {
    type Ast = TypeAlias;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        TypeAlias {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: self.visibility().gen_ast(diagnostics),
            ident: self.name().unwrap().gen_ast(diagnostics),
            ty: self.type_declaration().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::StructDeclaration {
    type Ast = Struct;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let members = self
            .body()
            .unwrap()
            .fields()
            .map(|field| field.gen_ast(diagnostics))
            .collect();
        Struct {
            attributes: self.attributes().gen_ast(diagnostics),
            visibility: self.visibility().gen_ast(diagnostics),
            ident: self.name().unwrap().gen_ast(diagnostics),
            members,
        }
    }
}

impl ToStrictAst for ast::StructMember {
    type Ast = Spanned<StructMember>;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let result = StructMember {
            attributes: self.attributes().gen_ast(diagnostics),
            ident: self.name().unwrap().gen_ast(diagnostics),
            ty: self.r#type().unwrap().gen_ast(diagnostics),
        };
        Spanned::new(result, self.span())
    }
}

impl ToStrictAst for ast::AssertStatement {
    type Ast = ConstAssert;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        ConstAssert {
            attributes: self.attributes().gen_ast(diagnostics),
            expression: self.expression().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::GlobalCompoundDeclaration {
    type Ast = CompoundGlobalDeclaration;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        CompoundGlobalDeclaration {
            attributes: self.attributes().gen_ast(diagnostics),
            body: self.items().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::Statement {
    type Ast = Spanned<Statement>;
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let span = self.span();
        let result = match self {
            ast::Statement::IfStatement(if_statement) => {
                Statement::If(if_statement.gen_ast(diagnostics))
            }
            ast::Statement::SwitchStatement(switch_statement) => {
                Statement::Switch(switch_statement.gen_ast(diagnostics))
            }
            ast::Statement::LoopStatement(loop_statement) => {
                Statement::Loop(loop_statement.gen_ast(diagnostics))
            }
            ast::Statement::ForStatement(for_statement) => {
                Statement::For(for_statement.gen_ast(diagnostics))
            }
            ast::Statement::WhileStatement(while_statement) => {
                Statement::While(while_statement.gen_ast(diagnostics))
            }
            ast::Statement::CompoundStatement(compound_statement) => {
                Statement::Compound(compound_statement.gen_ast(diagnostics))
            }
            ast::Statement::FunctionCallStatement(function_call_statement) => {
                Statement::FunctionCall(function_call_statement.gen_ast(diagnostics))
            }
            ast::Statement::VariableDeclaration(variable_declaration) => {
                Statement::Declaration(variable_declaration.gen_ast(diagnostics))
            }
            ast::Statement::LetDeclaration(let_declaration) => {
                Statement::Declaration(let_declaration.gen_ast(diagnostics))
            }
            ast::Statement::ConstantDeclaration(constant_declaration) => {
                Statement::Declaration(constant_declaration.gen_ast(diagnostics))
            }
            ast::Statement::AssignmentStatement(assignment_statement) => {
                Statement::Assignment(assignment_statement.gen_ast(diagnostics))
            }
            ast::Statement::CompoundAssignmentStatement(compound_assignment_statement) => {
                Statement::Assignment(compound_assignment_statement.gen_ast(diagnostics))
            }
            ast::Statement::PhonyAssignmentStatement(phony_assignment_statement) => {
                Statement::Assignment(phony_assignment_statement.gen_ast(diagnostics))
            }
            ast::Statement::IncrementDecrementStatement(increment_decrement_statement) => {
                match increment_decrement_statement.increment_decrement().unwrap() {
                    ast::IncrementDecrement::Increment => {
                        Statement::Increment(crate::syntax::IncrementStatement {
                            attributes: increment_decrement_statement
                                .attributes()
                                .gen_ast(diagnostics),
                            expression: increment_decrement_statement
                                .expression()
                                .unwrap()
                                .gen_ast(diagnostics),
                        })
                    }
                    ast::IncrementDecrement::Decrement => {
                        Statement::Decrement(crate::syntax::DecrementStatement {
                            attributes: increment_decrement_statement
                                .attributes()
                                .gen_ast(diagnostics),
                            expression: increment_decrement_statement
                                .expression()
                                .unwrap()
                                .gen_ast(diagnostics),
                        })
                    }
                }
            }
            ast::Statement::AssertStatement(assert_statement) => {
                Statement::ConstAssert(assert_statement.gen_ast(diagnostics))
            }
            ast::Statement::BreakStatement(break_statement) => {
                Statement::Break(break_statement.gen_ast(diagnostics))
            }
            ast::Statement::ContinueStatement(continue_statement) => {
                Statement::Continue(continue_statement.gen_ast(diagnostics))
            }
            ast::Statement::DiscardStatement(discard_statement) => {
                Statement::Discard(discard_statement.gen_ast(diagnostics))
            }
            ast::Statement::ReturnStatement(return_statement) => {
                Statement::Return(return_statement.gen_ast(diagnostics))
            }
            ast::Statement::EmptyStatement(_) => Statement::Void,
            ast::Statement::ContinuingStatement(_) => Statement::Void,
            ast::Statement::BreakIfStatement(_) => Statement::Void,
        };
        Spanned::new(result, span)
    }
}

impl ToStrictAst for ast::IfStatement {
    type Ast = IfStatement;
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        IfStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            if_clause: self.if_block().unwrap().gen_ast(diagnostics),
            else_if_clauses: self.else_if_blocks().gen_ast(diagnostics),
            else_clause: self
                .else_block()
                .map(|else_block| else_block.gen_ast(diagnostics)),
        }
    }
}

impl ToStrictAst for ast::IfClause {
    type Ast = IfClause;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        IfClause {
            expression: self.condition().unwrap().gen_ast(diagnostics),
            body: self.block().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::ElseIfClause {
    type Ast = ElseIfClause;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        ElseIfClause {
            expression: self.condition().unwrap().gen_ast(diagnostics),
            body: self.block().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::ElseClause {
    type Ast = ElseClause;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        ElseClause {
            body: self.block().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::SwitchStatement {
    type Ast = SwitchStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let body = self.block().unwrap();
        SwitchStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            expression: self.expression().unwrap().gen_ast(diagnostics),
            // TODO: Attributes
            body_attributes: Vec::new(),
            clauses: body.cases().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::SwitchBodyCase {
    type Ast = SwitchClause;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        SwitchClause {
            // TODO: Attributes
            attributes: Vec::new(),
            // TODO: fix wgsl-analyzer to properly type the .selectors. It should return an enum of "none | default | selectors"
            case_selectors: self
                .selectors()
                .map(|selectors| selectors.exprs().gen_ast(diagnostics))
                .unwrap_or_default(),
            body: self.block().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::SwitchCaseSelector {
    type Ast = CaseSelector;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        match self {
            ast::SwitchCaseSelector::Expression(expression) => {
                CaseSelector::Expression(expression.gen_ast(diagnostics))
            }
            ast::SwitchCaseSelector::SwitchDefaultSelector(_) => CaseSelector::Default,
        }
    }
}

impl ToStrictAst for ast::LoopStatement {
    type Ast = LoopStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let continuing_statement = self
            .block()
            .unwrap()
            .syntax()
            .children()
            .find_map(ast::ContinuingStatement::cast);

        LoopStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            body: self.block().unwrap().gen_ast(diagnostics),
            continuing: continuing_statement.map(|stmt| stmt.gen_ast(diagnostics)),
        }
    }
}

impl ToStrictAst for ast::ContinuingStatement {
    type Ast = ContinuingStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let break_if_statement = self
            .block()
            .unwrap()
            .syntax()
            .children()
            .find_map(ast::BreakIfStatement::cast);

        ContinuingStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            body: self.block().unwrap().gen_ast(diagnostics),
            break_if: break_if_statement.map(|stmt| stmt.gen_ast(diagnostics)),
        }
    }
}

impl ToStrictAst for ast::BreakIfStatement {
    type Ast = BreakIfStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        BreakIfStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            expression: self.condition().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::ForStatement {
    type Ast = ForStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        ForStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            initializer: self.initializer().map(|init| init.gen_ast(diagnostics)),
            condition: self.condition().map(|cond| cond.gen_ast(diagnostics)),
            update: self.continuing_part().map(|upd| upd.gen_ast(diagnostics)),
            body: self.block().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::WhileStatement {
    type Ast = WhileStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        WhileStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            condition: self.condition().unwrap().gen_ast(diagnostics),
            body: self.block().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::CompoundStatement {
    type Ast = CompoundStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let statements = self
            .statements()
            .filter(|statement| match statement {
                ast::Statement::ContinuingStatement(_) => false,
                ast::Statement::BreakIfStatement(_) => false,
                _ => true,
            })
            .map(|statement| statement.gen_ast(diagnostics))
            .collect::<Vec<_>>();
        CompoundStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            statements,
        }
    }
}

impl ToStrictAst for ast::FunctionCallStatement {
    type Ast = FunctionCallStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        FunctionCallStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            call: self.expression().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::AssignmentStatement {
    type Ast = AssignmentStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        AssignmentStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            operator: wgsl_types::syntax::AssignmentOperator::Equal,
            lhs: self.left_side().unwrap().gen_ast(diagnostics),
            rhs: self.right_side().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::CompoundAssignmentStatement {
    type Ast = AssignmentStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let operator = match self.operator().unwrap() {
            ast::operators::AssignmentOperator::PlusEqual => AssignmentOperator::PlusEqual,
            ast::operators::AssignmentOperator::TimesEqual => AssignmentOperator::TimesEqual,
            ast::operators::AssignmentOperator::MinusEqual => AssignmentOperator::MinusEqual,
            ast::operators::AssignmentOperator::DivisionEqual => AssignmentOperator::DivisionEqual,
            ast::operators::AssignmentOperator::ShiftLeftAssign => {
                AssignmentOperator::ShiftLeftAssign
            }
            ast::operators::AssignmentOperator::ShiftRightAssign => {
                AssignmentOperator::ShiftRightAssign
            }
            ast::operators::AssignmentOperator::ModuloEqual => AssignmentOperator::ModuloEqual,
            ast::operators::AssignmentOperator::AndEqual => AssignmentOperator::AndEqual,
            ast::operators::AssignmentOperator::OrEqual => AssignmentOperator::OrEqual,
            ast::operators::AssignmentOperator::XorEqual => AssignmentOperator::XorEqual,
        };
        AssignmentStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            operator,
            lhs: self.left_side().unwrap().gen_ast(diagnostics),
            rhs: self.right_side().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::PhonyAssignmentStatement {
    type Ast = AssignmentStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        // TODO: This is a bit of a hack, it produces an invalid ident!
        let lhs = Expression::TypeOrIdentifier(Ident::new("_".to_string()).into());
        let equal = self.equal_token().unwrap();
        let lhs = Spanned::new(
            lhs,
            Span::new(
                usize::from(equal.text_range().start())..usize::from(equal.text_range().start()),
            ),
        );

        AssignmentStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            operator: wgsl_types::syntax::AssignmentOperator::Equal,
            lhs,
            rhs: self.right_side().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::BreakStatement {
    type Ast = BreakStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        BreakStatement {
            attributes: self.attributes().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::ContinueStatement {
    type Ast = ContinueStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        ContinueStatement {
            attributes: self.attributes().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::DiscardStatement {
    type Ast = DiscardStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        DiscardStatement {
            attributes: self.attributes().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::ReturnStatement {
    type Ast = ReturnStatement;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        ReturnStatement {
            attributes: self.attributes().gen_ast(diagnostics),
            expression: self
                .expression()
                .map(|expression| expression.gen_ast(diagnostics)),
        }
    }
}

impl ToStrictAst for ast::TypeSpecifier {
    type Ast = TypeExpression;
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let (path, ident) = self.path().unwrap().gen_ast(diagnostics);
        TypeExpression {
            path,
            ident,
            template_args: self.template_parameters().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::Path {
    type Ast = (Option<ModulePath>, Ident);
    fn gen_ast(self, _diagnostics: &mut Diagnostics) -> Self::Ast {
        let mut components = self
            .segments()
            .map(|segment| segment.text().to_owned())
            .collect::<Vec<_>>();
        let ident = Ident::new(components.pop().unwrap());

        let origin = match self.relative() {
            Some(ast::ImportRelative::ImportPackageRelative(_)) => PathOrigin::Absolute,
            Some(ast::ImportRelative::ImportSuperRelative(import_super)) => {
                PathOrigin::Relative(usize::from(import_super.super_count()))
            }
            None => match components.pop() {
                Some(name) => PathOrigin::Package(name.to_string()),
                // Default case when there are no components.
                None => return (None, ident),
            },
        };

        (Some(ModulePath { origin, components }), ident)
    }
}

impl ToStrictAst for Option<ast::TemplateList> {
    type Ast = Option<Vec<TemplateArg>>;
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        match self {
            Some(template_list) => Some(template_list.gen_ast(diagnostics)),
            None => None,
        }
    }
}

impl ToStrictAst for ast::TemplateList {
    type Ast = Vec<TemplateArg>;
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        self.parameters()
            .gen_ast(diagnostics)
            .into_iter()
            .map(|expression| TemplateArg { expression })
            .collect::<Vec<_>>()
    }
}

impl ToStrictAst for ast::Expression {
    type Ast = Spanned<Expression>;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let span = self.span();
        let expression = match self {
            ast::Expression::IndexExpression(index_expression) => {
                Expression::Indexing(index_expression.gen_ast(diagnostics))
            }
            ast::Expression::FieldExpression(field_expression) => {
                Expression::NamedComponent(field_expression.gen_ast(diagnostics))
            }
            ast::Expression::PrefixExpression(prefix_expression) => {
                Expression::Unary(prefix_expression.gen_ast(diagnostics))
            }
            ast::Expression::InfixExpression(infix_expression) => {
                Expression::Binary(infix_expression.gen_ast(diagnostics))
            }
            ast::Expression::IdentExpression(ident_expression) => {
                Expression::TypeOrIdentifier(ident_expression.gen_ast(diagnostics))
            }
            ast::Expression::FunctionCall(function_call) => {
                Expression::FunctionCall(function_call.gen_ast(diagnostics))
            }
            ast::Expression::ParenthesisExpression(parenthesis_expression) => {
                Expression::Parenthesized(parenthesis_expression.gen_ast(diagnostics))
            }
            ast::Expression::Literal(literal) => Expression::Literal(literal.gen_ast(diagnostics)),
        };
        Spanned::new(expression, span)
    }
}

impl ToStrictAst for ast::IndexExpression {
    type Ast = IndexingExpression;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        IndexingExpression {
            base: self.expression().unwrap().gen_ast(diagnostics),
            index: self.index().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::FieldExpression {
    type Ast = NamedComponentExpression;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let component = Ident::new(self.field().unwrap().text().to_owned());
        NamedComponentExpression {
            base: self.expression().unwrap().gen_ast(diagnostics),
            component,
        }
    }
}

impl ToStrictAst for ast::PrefixExpression {
    type Ast = UnaryExpression;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let operator = match self.operator_kind().unwrap() {
            ast::operators::UnaryOperator::Negation => UnaryOperator::Negation,
            ast::operators::UnaryOperator::LogicalNegation => UnaryOperator::LogicalNegation,
            ast::operators::UnaryOperator::AddressOf => UnaryOperator::AddressOf,
            ast::operators::UnaryOperator::Indirection => UnaryOperator::Indirection,
            ast::operators::UnaryOperator::BitwiseComplement => UnaryOperator::BitwiseComplement,
        };
        UnaryExpression {
            operator,
            operand: self.expression().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::InfixExpression {
    type Ast = BinaryExpression;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let operator = match self.operator_kind().unwrap() {
            ast::operators::BinaryOperation::Logical(
                ast::operators::LogicOperation::ShortCircuitAnd,
            ) => wgsl_types::syntax::BinaryOperator::ShortCircuitAnd,
            ast::operators::BinaryOperation::Logical(
                ast::operators::LogicOperation::ShortCircuitOr,
            ) => wgsl_types::syntax::BinaryOperator::ShortCircuitOr,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::Addition,
            ) => wgsl_types::syntax::BinaryOperator::Addition,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::Multiplication,
            ) => wgsl_types::syntax::BinaryOperator::Multiplication,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::Subtraction,
            ) => wgsl_types::syntax::BinaryOperator::Subtraction,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::Division,
            ) => wgsl_types::syntax::BinaryOperator::Division,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::ShiftLeft,
            ) => wgsl_types::syntax::BinaryOperator::ShiftLeft,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::ShiftRight,
            ) => wgsl_types::syntax::BinaryOperator::ShiftRight,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::BitwiseXor,
            ) => wgsl_types::syntax::BinaryOperator::BitwiseXor,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::BitwiseOr,
            ) => wgsl_types::syntax::BinaryOperator::BitwiseOr,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::BitwiseAnd,
            ) => wgsl_types::syntax::BinaryOperator::BitwiseAnd,
            ast::operators::BinaryOperation::Arithmetic(
                ast::operators::ArithmeticOperation::Remainder,
            ) => wgsl_types::syntax::BinaryOperator::Remainder,
            ast::operators::BinaryOperation::Comparison(
                ast::operators::ComparisonOperation::Equality,
            ) => wgsl_types::syntax::BinaryOperator::Equality,
            ast::operators::BinaryOperation::Comparison(
                ast::operators::ComparisonOperation::Inequality,
            ) => wgsl_types::syntax::BinaryOperator::Inequality,
            ast::operators::BinaryOperation::Comparison(
                ast::operators::ComparisonOperation::LessThan,
            ) => wgsl_types::syntax::BinaryOperator::LessThan,
            ast::operators::BinaryOperation::Comparison(
                ast::operators::ComparisonOperation::LessThanEqual,
            ) => wgsl_types::syntax::BinaryOperator::LessThanEqual,
            ast::operators::BinaryOperation::Comparison(
                ast::operators::ComparisonOperation::GreaterThan,
            ) => wgsl_types::syntax::BinaryOperator::GreaterThan,
            ast::operators::BinaryOperation::Comparison(
                ast::operators::ComparisonOperation::GreaterThanEqual,
            ) => wgsl_types::syntax::BinaryOperator::GreaterThanEqual,
        };
        BinaryExpression {
            operator,
            left: self.left_side().unwrap().gen_ast(diagnostics),
            right: self.right_side().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::IdentExpression {
    type Ast = TypeExpression;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        let (path, ident) = self.path().unwrap().gen_ast(diagnostics);
        TypeExpression {
            path,
            ident,
            template_args: self.template_parameters().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::ParenthesisExpression {
    type Ast = ParenthesizedExpression;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        ParenthesizedExpression {
            expression: self.inner().unwrap().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::Literal {
    type Ast = LiteralExpression;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        match self.kind() {
            ast::LiteralToken::IntLiteral(syntax_token) => {
                let (text, int_variant) = split_int_suffix(syntax_token.text());
                let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                    Some(hex) => u64::from_str_radix(hex, 16),
                    None => text.parse::<u64>(),
                }
                .unwrap_or_else(|_| {
                    diagnostics.errors.push(Error {
                        error: ErrorKind::InvalidToken,
                        span: self.span(),
                    });
                    0
                });

                match int_variant {
                    BuiltinInt::I32 => {
                        LiteralExpression::I32(i32::try_from(value).unwrap_or_else(|_| {
                            diagnostics.errors.push(Error {
                                error: ErrorKind::InvalidToken,
                                span: self.span(),
                            });
                            0
                        }))
                    }
                    BuiltinInt::U32 => {
                        LiteralExpression::U32(u32::try_from(value).unwrap_or_else(|_| {
                            diagnostics.errors.push(Error {
                                error: ErrorKind::InvalidToken,
                                span: self.span(),
                            });
                            0
                        }))
                    }
                    BuiltinInt::I64 => {
                        cfg_select! {
                            feature = "naga-ext" => {
                                LiteralExpression::I64(i64::try_from(value).unwrap_or_else(|_| {
                                    diagnostics.errors.push(Error {
                                        error: ErrorKind::InvalidToken,
                                        span: self.span(),
                                    });
                                    0
                                }))
                            }
                            _ => panic!("naga-ext feature not enabled for I64"),
                        }
                    }

                    BuiltinInt::U64 => {
                        cfg_select! {
                            feature = "naga-ext" => {
                                LiteralExpression::U64(u64::try_from(value).unwrap_or_else(|_| {
                                    diagnostics.errors.push(Error {
                                        error: ErrorKind::InvalidToken,
                                        span: self.span(),
                                    });
                                    0
                                }))
                            }
                            _ => panic!("naga-ext feature not enabled for U64"),
                        }
                    }
                    BuiltinInt::Abstract => {
                        LiteralExpression::AbstractInt(i64::try_from(value).unwrap_or_else(|_| {
                            diagnostics.errors.push(Error {
                                error: ErrorKind::InvalidToken,
                                span: self.span(),
                            });
                            0
                        }))
                    }
                }
            }
            ast::LiteralToken::FloatLiteral(syntax_token) => {
                use std::str::FromStr as _;
                // Float suffixes are not accepted by `f32::from_str`. Ignore them
                let (text, float_variant) = split_float_suffix(syntax_token.text());

                // https://www.w3.org/TR/WGSL/#numeric-literals says that an error happens if
                // - A hexadecimal floating point literal with a f or h suffix overflows or cannot be exactly represented by the target type.
                // - A decimal floating point literal with a f or h suffix overflows the target type.
                match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                    Some(hex) => {
                        // Note: Rust itself is going to gain the ability to parse hex floats https://github.com/rust-lang/rust/issues/160626

                        let (parts, trailing) = hex_float::wgsl::parse_with_options(
                            hex,
                            hex_float::PartFlags::WHOLE
                                | hex_float::PartFlags::POINT
                                | hex_float::PartFlags::FRACTION
                                | hex_float::PartFlags::EXPONENT,
                        )
                        .expect("parsed number should be a valid float");
                        assert!(
                            trailing.is_empty(),
                            "all characters should have been parsed"
                        );

                        match float_variant {
                            // TODO: Where is my f16 binary format?
                            BuiltinFloat::F16 => {
                                LiteralExpression::F16(parts.to_float::<f32>().get())
                            }
                            BuiltinFloat::F32 => {
                                LiteralExpression::F32(match parts.to_float::<f32>() {
                                    hex_float::Assembled::Exact(v) => v,
                                    hex_float::Assembled::Rounded(v) => {
                                        diagnostics.errors.push(Error {
                                            error: ErrorKind::InvalidToken,
                                            span: self.span(),
                                        });
                                        v
                                    }
                                    hex_float::Assembled::Infinity(v) => {
                                        diagnostics.errors.push(Error {
                                            error: ErrorKind::InvalidToken,
                                            span: self.span(),
                                        });
                                        v
                                    }
                                })
                            }
                            BuiltinFloat::Abstract => {
                                LiteralExpression::AbstractFloat(parts.to_float::<f64>().get())
                            }
                        }
                    }
                    None => match float_variant {
                        BuiltinFloat::F16 => LiteralExpression::F16(
                            f32::from_str(text).expect("parsed number should be a valid float"),
                        ),
                        BuiltinFloat::F32 => LiteralExpression::F32(
                            f32::from_str(text).expect("parsed number should be a valid float"),
                        ),
                        BuiltinFloat::Abstract => LiteralExpression::AbstractFloat(
                            f64::from_str(text).expect("parsed number should be a valid float"),
                        ),
                    },
                }
            }
            ast::LiteralToken::True(_) => LiteralExpression::Bool(true),
            ast::LiteralToken::False(_) => LiteralExpression::Bool(false),
        }
    }
}

enum BuiltinInt {
    I32,
    U32,
    // SHADER_INT64
    I64,
    U64,
    Abstract,
}

fn split_int_suffix(number: &str) -> (&str, BuiltinInt) {
    if number.ends_with("lu") {
        (&number[0..(number.len() - "lu".len())], BuiltinInt::U64)
    } else if number.ends_with("li") {
        (&number[0..(number.len() - "li".len())], BuiltinInt::I64)
    } else if number.ends_with('u') {
        (&number[0..(number.len() - 'u'.len_utf8())], BuiltinInt::U32)
    } else if number.ends_with('i') {
        (&number[0..(number.len() - 'i'.len_utf8())], BuiltinInt::I32)
    } else {
        (number, BuiltinInt::Abstract)
    }
}

enum BuiltinFloat {
    F16,
    F32,
    Abstract,
}

fn split_float_suffix(number: &str) -> (&str, BuiltinFloat) {
    // future reference: naga has li and lu suffix for 64bits literals.
    if number.ends_with('f') {
        (&number[0..(number.len() - 1)], BuiltinFloat::F32)
    } else if number.ends_with('h') {
        (&number[0..(number.len() - 1)], BuiltinFloat::F16)
    } else {
        (number, BuiltinFloat::Abstract)
    }
}

impl ToStrictAst for ast::FunctionCall {
    type Ast = FunctionCall;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        FunctionCall {
            ty: self.ident_expression().unwrap().gen_ast(diagnostics),
            arguments: self.parameters().unwrap().arguments().gen_ast(diagnostics),
        }
    }
}

impl ToStrictAst for ast::Name {
    type Ast = Ident;

    fn gen_ast(self, _diagnostics: &mut Diagnostics) -> Self::Ast {
        Ident::new(self.text().to_owned())
    }
}

trait WithSpan {
    fn span(&self) -> Span;
}
impl<T: ?Sized + AstNode> WithSpan for T {
    fn span(&self) -> Span {
        Span::from(self.syntax().text_range())
    }
}

/// Conversion trait, maps a w-a to a wesl-rs type.
///
/// This lets us avoid having to write the type name everywhere,
/// since the conversion is very type-directed.
trait ToStrictAst {
    type Ast;
    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast;
}

impl<T: ToStrictAst + syntax::AstNode> ToStrictAst for syntax::AstChildren<T> {
    type Ast = Vec<T::Ast>;

    fn gen_ast(self, diagnostics: &mut Diagnostics) -> Self::Ast {
        self.map(|child| child.gen_ast(diagnostics)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::{Expect, expect};
    use std::borrow::Cow;
    use std::fmt::Write as _;

    #[expect(clippy::needless_pass_by_value, reason = "matches expect! macro")]
    fn check(fixture: &str, expect: Expect) {
        let (module, diagnostics) = parse_str(fixture);
        let errors_string = diagnostics
            .errors
            .into_iter()
            .map(|error| error.with_source(Cow::Borrowed(fixture)).to_string())
            .collect::<Vec<String>>()
            .join("\n");

        let mut actual = module.to_string();
        if !errors_string.is_empty() {
            writeln!(actual).unwrap();
            writeln!(actual, "{}", errors_string).unwrap();
        }
        expect.assert_eq(&actual);
    }

    #[test]
    fn parse_const() {
        check(
            "const a = true;",
            expect![[r#"
                const a = true;
            "#]],
        );
    }

    #[test]
    fn parse_main_function() {
        check(
            "@vertex fn vs_main() { let a = 3 * 5; }",
            expect![[r#"
                @vertex
                fn vs_main() {
                    let a = 3 * 5;
                }
            "#]],
        );
    }

    #[test]
    fn parse_import_statements() {
        check(
            "import package::{foo::bar, baz};",
            expect![[r#"
                import package::{ foo::bar, baz };


            "#]],
        );
    }
}
