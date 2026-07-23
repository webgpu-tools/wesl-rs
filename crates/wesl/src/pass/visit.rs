use wesl_macros::{query, query_mut};

use wgsl_parse::syntax::*;

pub trait Visit<T> {
    /// Visit each child node of type `T` in the subtree of `Self`.
    ///
    /// Implementations of Visit do not recurse past `T`, meaning that if you really want
    /// to visit all children of type T you would have to call `<T as Visit<T>>::visit` on
    /// each visited `T`. Alternatively, use [`Self::visit_rec`] which solves this
    /// exact problem.
    fn visit<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        T: 'a;

    /// Mutable version of [`Self::visit`].
    fn visit_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut T>
    where
        T: 'a;

    /// Visit each child node of type `T` in the subtree of `Self`, recursively in depth-first order.
    ///
    /// Due to Rust's aliasing model, you can't iterate mutably on a node and its
    /// children. This function allows visiting recursively by passing a closure instead.
    #[allow(unused)]
    fn visit_rec<'a, F>(&'a self, f: &mut F)
    where
        T: Visit<T> + 'a,
        F: FnMut(&T),
    {
        Visit::<T>::visit(self).for_each(|x| {
            f(x);
            x.visit_rec(f);
        });
    }

    /// Mutable version of [`Self::visit_rec`].
    #[allow(unused)]
    fn visit_rec_mut<'a, F>(&'a mut self, f: &mut F)
    where
        T: Visit<T> + 'a,
        F: FnMut(&mut T),
    {
        Visit::<T>::visit_mut(self).for_each(|x| {
            f(x);
            x.visit_rec_mut(f);
        });
    }
}

macro_rules! impl_visit {
    ($type:ty => $visited:ty, $expr:tt) => {
        impl Visit<$visited> for $type {
            fn visit<'a>(&'a self) -> impl Iterator<Item = &'a $visited>
            where
                $visited: 'a,
            {
                #[allow(unused)]
                fn visit<'a, T: Visit<U>, U: 'a>(expr: &'a T) -> impl Iterator<Item = &'a U> {
                    Visit::<U>::visit(expr)
                }

                #[allow(unused)]
                fn recurse(expr: &$type) -> impl Iterator<Item = &$visited> {
                    Visit::<$visited>::visit(expr)
                }

                let root: &$type = self;
                query!(root.$expr)
            }
            fn visit_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut $visited>
            where
                $visited: 'a,
            {
                #[allow(unused)]
                fn visit<'a, T: Visit<U>, U: 'a>(
                    expr: &'a mut T,
                ) -> impl Iterator<Item = &'a mut U> {
                    Visit::<U>::visit_mut(expr)
                }

                #[allow(unused)]
                fn recurse(expr: &mut $type) -> impl Iterator<Item = &mut $visited> {
                    Visit::<$visited>::visit_mut(expr)
                }

                let root: &mut $type = self;
                query_mut!(root.$expr)
            }
        }
    };
}

impl_visit! { Expression => ExpressionNode,
    {
        Expression::Parenthesized.expression.(x => recurse(x)),
        Expression::NamedComponent.base.(x => recurse(x)),
        Expression::Indexing.{
            base.(x => recurse(x)),
            index.(x => recurse(x)),
        },
        Expression::Unary.operand.(x => recurse(x)),
        Expression::Binary.{
            left.(x => recurse(x)),
            right.(x => recurse(x)),
        },
        Expression::FunctionCall.arguments.[].(x => recurse(x)),
    }
}

impl_visit! { Expression => TypeExpression,
    {
        Expression::Parenthesized.expression.(x => recurse(x)),
        Expression::NamedComponent.base.(x => recurse(x)),
        Expression::Indexing.{ base.(x => recurse(x)), index.(x => recurse(x)) },
        Expression::Unary.operand.(x => recurse(x)),
        Expression::Binary.{ left.(x => recurse(x)), right.(x => recurse(x)) },
        Expression::FunctionCall.{
            ty,
            arguments.[].(x => recurse(x))
        },
        Expression::TypeOrIdentifier,
    }
}

impl_visit! { TypeExpression => TypeExpression,
    {
        template_args.[].[].expression.(x => visit::<Expression, TypeExpression>(x))
    }
}

impl_visit! { Statement => Attributes,
    {
        Statement::Compound.{ attributes, statements.[].(x => recurse(x)) },
        Statement::If.{
            attributes,
            else_if_clauses.[].body.statements.[].(x => recurse(x)),
            else_clause.[].body.statements.[].(x => recurse(x)),
        },
        Statement::Switch.{
            attributes,
            clauses.[].{
                attributes,
                body.statements.[].(x => recurse(x))
            },
        },
        Statement::Loop.{
            attributes,
            body.statements.[].(x => recurse(x)),
            continuing.[].{
                attributes,
                body.statements.[].(x => recurse(x)),
                break_if.[].attributes
            },
        },
        Statement::For.{
            attributes,
            body.statements.[].(x => recurse(x)),
        },
        Statement::While.{
            attributes,
            body.statements.[].(x => recurse(x)),
        },
        Statement::Break.attributes,
        Statement::Continue.attributes,
        Statement::Return.attributes,
        Statement::Discard.attributes,
        Statement::FunctionCall.attributes,
        Statement::ConstAssert.attributes,
        Statement::Declaration.attributes,
    }
}

impl_visit! { Statement => TypeExpression,
    {
        Statement::Compound.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            statements.[].(x => recurse(x)),
        },
        Statement::Assignment.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            lhs.(x => visit::<Expression, TypeExpression>(x)),
            rhs.(x => visit::<Expression, TypeExpression>(x)),
        },
        Statement::Increment.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            expression.(x => visit::<Expression, TypeExpression>(x)),
        },
        Statement::Decrement.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            expression.(x => visit::<Expression, TypeExpression>(x)),
        },
        Statement::If.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            if_clause.{
                expression.(x => visit::<Expression, TypeExpression>(x)),
                body.{
                    attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                    statements.[].(x => recurse(x)),
                }
            },
            else_if_clauses.[].{
                attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                expression.(x => visit::<Expression, TypeExpression>(x)),
                body.{
                    attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                    statements.[].(x => recurse(x)),
                }
            },
            else_clause.[].{
                attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                body.{
                    attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                    statements.[].(x => recurse(x)),
                }
            },
        },
        Statement::Switch.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            expression.(x => visit::<Expression, TypeExpression>(x)),
            body_attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            clauses.[].{
                attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                case_selectors.[].CaseSelector::Expression.(x => visit::<Expression, TypeExpression>(x)),
                body.{
                    attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                    statements.[].(x => recurse(x)),
                }
            }
        },
        Statement::Loop.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            body.{
                attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                statements.[].(x => recurse(x)),
            },
            continuing.[].{
                attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                body.{
                    attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                    statements.[].(x => recurse(x)),
                },
                break_if.[].{
                    attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                    expression.(x => visit::<Expression, TypeExpression>(x)),
                }
            }
        },
        Statement::For.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            initializer.[].(x => recurse(x)),
            condition.[].(x => visit::<Expression, TypeExpression>(x)),
            update.[].(x => recurse(x)),
            body.{
                attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                statements.[].(x => recurse(x)),
            },
        },
        Statement::While.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            condition.(x => visit::<Expression, TypeExpression>(x)),
            body.{
                attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
                statements.[].(x => recurse(x)),
            },
        },
        Statement::Break.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        },
        Statement::Continue.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        },
        Statement::Return.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            expression.[].(x => visit::<Expression, TypeExpression>(x)),
        },
        Statement::Discard.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        },
        Statement::FunctionCall.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            call.{
                ty,
                arguments.[].(x => visit::<Expression, TypeExpression>(x)),
            }
        },
        Statement::ConstAssert.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            expression.(x => visit::<Expression, TypeExpression>(x)),
        },
        Statement::Declaration.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            ty.[],
            initializer.[].(x => visit::<Expression, TypeExpression>(x)),
        },
    }
}

impl_visit! { Statement => ExpressionNode,
    {
        Statement::Compound.statements.[].(x => recurse(x)),
        Statement::Assignment.{ lhs, rhs },
        Statement::Increment.expression,
        Statement::Decrement.expression,
        Statement::If.{
            if_clause.{
                expression,
                body.statements.[].(x => recurse(x)),
            },
            else_if_clauses.[].{
                expression,
                body.statements.[].(x => recurse(x)),
            }
        },
        Statement::Switch.{
            expression,
            clauses.[].{
                case_selectors.[].CaseSelector::Expression,
                body.statements.[].(x => recurse(x)),
            }
        },
        Statement::Loop.{
            body.statements.[].(x => recurse(x)),
            continuing.[].{
                body.statements.[].(x => recurse(x)),
                break_if.[].expression,
            }
        },
        Statement::For.{
            initializer.[].(x => recurse(x)),
            condition.[],
            update.[].(x => recurse(x)),
            body.statements.[].(x => recurse(x)),
        },
        Statement::While.{
            condition,
            body.statements.[].(x => recurse(x)),
        },
        Statement::Return.expression.[],
        Statement::FunctionCall.call.arguments.[],
        Statement::ConstAssert.expression,
        Statement::Declaration.initializer.[],
    }
}

impl_visit! { Statement => StatementNode,
    {
        Statement::Compound.statements.[],
        Statement::If.{
            if_clause.body.statements.[],
            else_if_clauses.[].body.statements.[],
        },
        Statement::Switch.clauses.[].body.statements.[],
        Statement::Loop.{
            body.statements.[],
            continuing.[].body.statements.[],
        },
        Statement::For.{
            initializer.[],
            update.[],
            body.statements.[],
        },
        Statement::While.body.statements.[],
    }
}

impl_visit! { Attribute => TypeExpression,
    {
        Attribute::Align.(x => visit::<Expression, TypeExpression>(x)),
        Attribute::Binding.(x => visit::<Expression, TypeExpression>(x)),
        Attribute::BlendSrc.(x => visit::<Expression, TypeExpression>(x)),
        Attribute::Group.(x => visit::<Expression, TypeExpression>(x)),
        Attribute::Id.(x => visit::<Expression, TypeExpression>(x)),
        Attribute::Location.(x => visit::<Expression, TypeExpression>(x)),
        Attribute::Size.(x => visit::<Expression, TypeExpression>(x)),
        Attribute::WorkgroupSize.{
            x.(x => visit::<Expression, TypeExpression>(x)),
            y.[].(x => visit::<Expression, TypeExpression>(x)),
            z.[].(x => visit::<Expression, TypeExpression>(x)),
        },
        #[cfg(feature = "generics")]
        Attribute::Type.variants.[],
        Attribute::Custom.arguments.[].[].(x => visit::<Expression, TypeExpression>(x))
    }
}

impl_visit! { TranslationUnit => ExpressionNode,
    {
        global_declarations.[].(x => visit::<GlobalDeclaration, ExpressionNode>(x))
    }
}

impl_visit! { GlobalDeclaration => ExpressionNode,
    {
        GlobalDeclaration::Declaration.{
            initializer.[],
        },
        GlobalDeclaration::Function.{
            body.statements.[].(x => visit::<Statement, ExpressionNode>(x)),
        }
    }
}

impl_visit! { TranslationUnit => StatementNode,
    {
        global_declarations.[].(x => visit::<GlobalDeclaration, StatementNode>(x)),
    }
}

impl_visit! { GlobalDeclaration => StatementNode,
    {
        GlobalDeclaration::Function.body.statements.[]
    }
}

impl_visit! { TranslationUnit => Attributes,
    {
        imports.[].attributes,
        global_directives.[].{
            GlobalDirective::Diagnostic.attributes,
            GlobalDirective::Enable.attributes,
            GlobalDirective::Requires.attributes,
        },
        global_declarations.[].(x => visit::<GlobalDeclaration, Attributes>(x)),
    }
}

impl_visit! { GlobalDeclaration => Attributes,
    {
        GlobalDeclaration::Declaration.attributes,
        GlobalDeclaration::TypeAlias.attributes,
        GlobalDeclaration::Struct.{
            attributes,
            members.[].attributes,
        },
        GlobalDeclaration::Function.{
            attributes,
            parameters.[].attributes,
            body.{ attributes, statements.[].(x => visit::<Statement, Attributes>(x)) }
        },
        GlobalDeclaration::ConstAssert.attributes,
    }
}

impl_visit! { TranslationUnit => TypeExpression,
    {
        global_declarations.[].(x => visit::<GlobalDeclaration, TypeExpression>(x))
    }
}

impl_visit! { GlobalDeclaration => TypeExpression,
    {
        GlobalDeclaration::Declaration.(x => visit::<Declaration, TypeExpression>(x)),
        GlobalDeclaration::TypeAlias.(x => visit::<TypeAlias, TypeExpression>(x)),
        GlobalDeclaration::Struct.(x => visit::<Struct, TypeExpression>(x)),
        GlobalDeclaration::Function.(x => visit::<Function, TypeExpression>(x)),
        GlobalDeclaration::ConstAssert.(x => visit::<ConstAssert, TypeExpression>(x))
    }
}

impl_visit! { Declaration => TypeExpression,
    {
        attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        ty.[],
        initializer.[].(x => visit::<Expression, TypeExpression>(x)),
    }
}
impl_visit! { TypeAlias => TypeExpression,
    {
        attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        ty,
    }
}
impl_visit! { Struct => TypeExpression,
    {
        attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        members.[].(x => visit::<StructMember, TypeExpression>(x)),
    }
}
impl_visit! { StructMember => TypeExpression,
    {
        attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        ty,
    }
}
impl_visit! { Function => TypeExpression,
    {
        attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        parameters.[].{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            ty,
        },
        return_attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        return_type.[],
        body.{
            attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
            statements.[].(x => visit::<Statement, TypeExpression>(x)),
        }
    }
}
impl_visit! { ConstAssert => TypeExpression,
    {
        attributes.[].(x => visit::<Attribute, TypeExpression>(x)),
        expression.(x => visit::<Expression, TypeExpression>(x)),
    }
}
