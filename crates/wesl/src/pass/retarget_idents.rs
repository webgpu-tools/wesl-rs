use std::{
    collections::{HashMap, hash_map::Entry},
    rc::Rc,
};

use crate::{idents::builtin_ident, visit::Visit};
use wesl_macros::query_mut;

use wgsl_parse::syntax::*;

/// was that not in the std at some point???
type BoxedIterator<'a, T> = Box<dyn Iterator<Item = T> + 'a>;

#[allow(dead_code)]
pub trait IteratorExt: Iterator {
    fn boxed<'a>(self) -> BoxedIterator<'a, Self::Item>
    where
        Self: Sized + 'a;
}

impl<T: Iterator> IteratorExt for T {
    fn boxed<'a>(self) -> BoxedIterator<'a, Self::Item>
    where
        Self: Sized + 'a,
    {
        Box::new(self)
    }
}

struct ScopeInner {
    local: HashMap<String, Ident>,
    parent: Option<Rc<ScopeInner>>,
}

struct Scope(Rc<ScopeInner>);

impl ScopeInner {
    fn iter(&self) -> impl Iterator<Item = (&String, &Ident)> {
        self.local
            .iter()
            .chain(self.parent.iter().flat_map(|parent| parent.iter().boxed()))
    }
}

impl Scope {
    fn new() -> Scope {
        Scope(Rc::new(ScopeInner {
            local: HashMap::new(),
            parent: None,
        }))
    }

    fn push(&self) -> Scope {
        Scope(Rc::new(ScopeInner {
            local: HashMap::new(),
            parent: Some(self.0.clone()),
        }))
    }

    fn iter(&self) -> impl Iterator<Item = (&String, &Ident)> {
        self.0.iter()
    }

    // insert in scope; or if already present, retarget the ident.
    fn insert(&mut self, ident: &mut Ident) {
        let inner = Rc::get_mut(&mut self.0).expect("cannot insert: scope use-count > 1");
        match inner.local.entry(ident.to_string()) {
            Entry::Occupied(entry) => {
                *ident = entry.get().clone();
            }
            Entry::Vacant(entry) => {
                entry.insert(ident.clone());
            }
        }
    }
}

/// Make all identifiers that point to the same declaration refer to the same string.
///
/// Retarget local references to the local declaration ident and global
/// references to the global declaration ident. It does this by keeping track of the
/// local declarations scope.
///
/// Same-scope declarations with the same name will have the same identifier.
/// Note: this can be valid code only with `@if` conditional declarations.
pub fn retarget_idents(syntax: &mut TranslationUnit) {
    fn flatten_imports(imports: &mut [ImportStatement]) -> impl Iterator<Item = &mut Ident> + '_ {
        fn rec(content: &mut ImportContent) -> impl Iterator<Item = &mut Ident> + '_ {
            match content {
                ImportContent::Item(item) => {
                    std::iter::once(item.rename.as_mut().unwrap_or(&mut item.ident)).boxed()
                }
                ImportContent::Collection(coll) => coll
                    .iter_mut()
                    .flat_map(|import| rec(&mut import.content))
                    .boxed(),
            }
        }
        imports
            .iter_mut()
            .flat_map(|import| rec(&mut import.content))
    }

    fn retarget_ty(ty: &mut TypeExpression, scope: &Scope) {
        if let Some((_, id)) = scope
            .iter()
            .find(|(name, _)| name.as_str() == *ty.ident.name())
        {
            ty.ident = id.clone();
        } else {
            let builtin = builtin_ident(&ty.ident.name()).cloned();
            if let Some(id) = builtin {
                ty.ident = id;
            }
        }
        query_mut!(ty.template_args.[].[].expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)))
            .for_each(|ty| retarget_ty(ty, scope));
    }

    // retarget local references to the local declaration ident and global
    // references to the global declaration ident. It does this by keeping track of the
    // local declarations scope.
    fn retarget_stats<'a>(
        stats: impl IntoIterator<Item = &'a mut StatementNode>,
        mut scope: Scope,
    ) -> Scope {
        stats.into_iter().for_each(|stmt| match stmt.node_mut() {
            Statement::Void => (),
            Statement::Compound(s) => {
                query_mut!(s.attributes.[].(x => x.visit_mut()))
                    .for_each(|ty| retarget_ty(ty, &scope));
                retarget_stats(&mut s.statements, scope.push());
            }
            Statement::Assignment(s) => {
                query_mut!(s.{
                    attributes.[].(x => x.visit_mut()),
                    lhs.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                    rhs.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                })
                .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::Increment(s) => {
                query_mut!(s.{
                    attributes.[].(x => x.visit_mut()),
                    expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                })
                .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::Decrement(s) => {
                query_mut!(s.{
                    attributes.[].(x => x.visit_mut()),
                    expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                })
                .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::If(s) => {
                let s2 = &mut *s; // COMBAK: not sure why this is needed?
                query_mut!(s2.{
                    attributes.[].(x => x.visit_mut()),
                    if_clause.{
                        expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                        body.{
                            attributes.[].(x => x.visit_mut()),
                        }
                    },
                    else_if_clauses.[].{
                        attributes.[].(x => x.visit_mut()),
                        expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                        body.{
                            attributes.[].(x => x.visit_mut()),
                        }
                    },
                    else_clause.[].{
                        attributes.[].(x => x.visit_mut()),
                        body.{
                            attributes.[].(x => x.visit_mut()),
                        },
                    },
                })
                .for_each(|ty| retarget_ty(ty, &scope));
                retarget_stats(&mut s.if_clause.body.statements, scope.push());
                for clause in &mut s.else_if_clauses {
                    retarget_stats(&mut clause.body.statements, scope.push());
                }
                if let Some(clause) = &mut s.else_clause {
                    retarget_stats(&mut clause.body.statements, scope.push());
                }
            }
            Statement::Switch(s) => {
                let s2 = &mut *s; // COMBAK: not sure why this is needed?
                query_mut!(s2.{
                    attributes.[].(x => x.visit_mut()),
                    expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                    body_attributes.[].(x => x.visit_mut()),
                    clauses.[].{
                        attributes.[].(x => x.visit_mut()),
                        case_selectors.[].CaseSelector::Expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                        body.{
                            attributes.[].(x => x.visit_mut()),
                        }
                    },

                })
                .for_each(|ty| retarget_ty(ty, &scope));
                for clause in &mut s.clauses {
                    retarget_stats(&mut clause.body.statements, scope.push());
                }
            }
            Statement::Loop(s) => {
                let s2 = &mut *s; // COMBAK: not sure why this is needed?
                query_mut!(s2.{
                    attributes.[].(x => x.visit_mut()),
                    body.attributes.[].(x => x.visit_mut()),
                })
                .for_each(|ty| retarget_ty(ty, &scope));
                let scope = retarget_stats(&mut s.body.statements, scope.push());
                // continuing, if present, must be the last statement of the loop body
                // and therefore has access to the scope at the end of the body.
                if let Some(s) = &mut s.continuing {
                    let s2 = &mut *s; // COMBAK: not sure why this is needed?
                    query_mut!(s2.{
                        attributes.[].(x => x.visit_mut()),
                        body.attributes.[].(x => x.visit_mut()),
                    })
                    .for_each(|ty| retarget_ty(ty, &scope));
                    let scope = retarget_stats(&mut s.body.statements, scope.push());
                    // break-if, if present, must be the last statement of the continuing body
                    // and therefore has access to the scope at the end of the body.
                    if let Some(s) = &mut s.break_if {
                        let s2 = &mut *s; // COMBAK: not sure why this is needed?
                        query_mut!(s2.{
                            attributes.[].(x => x.visit_mut()),
                            expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                        })
                        .for_each(|ty| retarget_ty(ty, &scope));
                    }
                }
            }
            Statement::For(s) => {
                query_mut!(s.attributes.[].(x => x.visit_mut()))
                    .for_each(|ty| retarget_ty(ty, &scope));
                let scope = if let Some(init) = &mut s.initializer {
                    retarget_stats([init], scope.push())
                } else {
                    scope.push()
                };
                query_mut!(s.condition.[].(x => Visit::<TypeExpression>::visit_mut(&mut **x)))
                    .for_each(|ty| retarget_ty(ty, &scope));
                query_mut!(s.body.attributes.[].(x => x.visit_mut()))
                    .for_each(|ty| retarget_ty(ty, &scope));
                if let Some(update) = &mut s.update {
                    retarget_stats([update], scope.push());
                }
                retarget_stats(&mut s.body.statements, scope);
            }
            Statement::While(s) => {
                let s2 = &mut *s; // COMBAK: not sure why this is needed?
                query_mut!(s2.{
                    attributes.[].(x => x.visit_mut()),
                    condition.(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                    body.attributes.[].(x => x.visit_mut()),
                })
                .for_each(|ty| retarget_ty(ty, &scope));
                retarget_stats(&mut s.body.statements, scope.push());
            }
            Statement::Break(s) => {
                query_mut!(s.attributes.[].(x => x.visit_mut()))
                    .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::Continue(s) => {
                query_mut!(s.attributes.[].(x => x.visit_mut()))
                    .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::Return(s) => {
                query_mut!(s.expression.[].(x => Visit::<TypeExpression>::visit_mut(&mut **x)))
                    .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::Discard(s) => {
                query_mut!(s.attributes.[].(x => x.visit_mut()))
                    .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::FunctionCall(s) => {
                query_mut!(s.{
                    attributes.[].(x => x.visit_mut()),
                    call.{
                        ty,
                        arguments.[].(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                    }
                })
                .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::ConstAssert(s) => {
                query_mut!(s.{
                    expression.(x => Visit::<TypeExpression>::visit_mut(&mut **x))
                })
                .for_each(|ty| retarget_ty(ty, &scope));
            }
            Statement::Declaration(s) => {
                let s2 = &mut *s; // COMBAK: not sure why this is needed?
                query_mut!(s2.{
                    attributes.[].(x => x.visit_mut()),
                    ty.[],
                    initializer.[].(x => Visit::<TypeExpression>::visit_mut(&mut **x)),
                })
                .for_each(|ty| retarget_ty(ty, &scope));
                scope.insert(&mut s.ident);
            }
        });
        scope
    }

    let mut scope = Scope::new();

    for ident in flatten_imports(&mut syntax.imports) {
        scope.insert(ident);
    }

    for decl in &mut syntax.global_declarations {
        let ident = match decl.node_mut() {
            GlobalDeclaration::Void => None,
            GlobalDeclaration::Declaration(decl) => Some(&mut decl.ident),
            GlobalDeclaration::TypeAlias(decl) => Some(&mut decl.ident),
            GlobalDeclaration::Struct(decl) => Some(&mut decl.ident),
            GlobalDeclaration::Function(decl) => Some(&mut decl.ident),
            GlobalDeclaration::ConstAssert(_) => None,
        };

        if let Some(ident) = ident {
            scope.insert(ident);
        }
    }

    for decl in &mut syntax.global_declarations {
        match decl.node_mut() {
            GlobalDeclaration::Void => (),
            GlobalDeclaration::Declaration(d) => {
                Visit::<TypeExpression>::visit_mut(d).for_each(|ty| retarget_ty(ty, &scope))
            }
            GlobalDeclaration::TypeAlias(d) => {
                Visit::<TypeExpression>::visit_mut(d).for_each(|ty| retarget_ty(ty, &scope))
            }
            GlobalDeclaration::Struct(d) => {
                Visit::<TypeExpression>::visit_mut(d).for_each(|ty| retarget_ty(ty, &scope))
            }
            GlobalDeclaration::Function(d) => {
                #[cfg(feature = "generics")]
                let scope = {
                    let mut scope = scope.push();
                    d.attributes
                        .iter_mut()
                        .filter_map(|attr| match attr.node_mut() {
                            Attribute::Type(attr) => Some(&mut attr.ident),
                            _ => None,
                        })
                        .for_each(|ident| scope.insert(ident));
                    scope
                };
                let d2 = &mut *d; // COMBAK: not sure why this is needed?
                query_mut!(d2.{
                    attributes.[].(x => x.visit_mut()),
                    parameters.[].{
                        attributes.[].(x => x.visit_mut()),
                        ty,
                    },
                    return_attributes.[].(x => x.visit_mut()),
                    return_type.[],
                    body.{
                        attributes.[].(x => x.visit_mut()),
                    }
                })
                .for_each(|ty| retarget_ty(ty, &scope));
                let mut scope = scope.push();
                d.parameters
                    .iter_mut()
                    .for_each(|param| scope.insert(&mut param.ident));
                retarget_stats(&mut d.body.statements, scope);
            }
            GlobalDeclaration::ConstAssert(d) => {
                Visit::<TypeExpression>::visit_mut(d).for_each(|ty| retarget_ty(ty, &scope))
            }
        }
    }
}

/// Check that retarget_ident handles shadowing correctly.
///
/// Procedure: we strip the ident digits, call retarget_idents and then rename idents
/// with a unique digit. We should end up back with the test code, assuming the iterator
/// order is predictable (is should be a depth-first search).
#[test]
fn test_retarget_idents() {
    use std::collections::HashSet;

    let source = r#"
        import        i0;
        import        i::{i0, i0, j};
        const_assert  i0+c1+v2+a3+s4; // due to hoisting, all global decls idents should be visible
        const         c1: c1 = i0+c1+v2+a3+s4;
        var<private>  v2: v2 = i0+c1+v2+a3+s4;
        alias         a3 = a3<i0, c1, v2, a3>;
        struct        s4 { m: m5 }

        fn f18(p: p6) {
            let x7 = i0+c1+v2+a3+s4;
            let x7 = x7;

            // shadowing with local declarations
            let i8 = i0+c1+v2+a3+s4;
            let c9 = i8+c1+v2+a3+s4;
            let v10 = i8+c9+v2+a3+s4;
            let a11 = i8+c9+v10+a3+s4;
            let s12 = i8+c9+v10+a11+s4;
            let x7 = i8+c9+v10+a11+s12; // x is in the same scope as previous x: should have same ident

            {
                let x13 = i8+c9+v10+a11+s12 +x7; // different x
                let x13 = x13;

                for (let x14 = x13; x14; x14++) {
                    let x14: x14 = x14; // in for loops, the initializer cannot be shadowed
                }

                loop {
                    var x15 = x13;
                    const y17 = x15;
                    continuing {
                        let x16 = x15;
                        break if x16 + y17;
                    }
                }
            }

            switch x7 {
                case x7, f18, g19 { x7 = x7; }
            }
        }

        fn f18() {} // f is in the same scope as previous f: should have same ident
        fn g19() {}
    "#;

    let wgsl: TranslationUnit = source.parse().expect("parse failure");
    let source_stripped = source.replace(|c: char| c.is_ascii_digit(), "");
    let mut wgsl_stripped: TranslationUnit = source_stripped.parse().expect("parse failure");
    retarget_idents(&mut wgsl_stripped);

    let mut idents = HashSet::new();
    let mut ordered_idents = Vec::new();
    // the test assumes that Visit is in depth-first order so the ident order is predictable.
    for ty in Visit::<TypeExpression>::visit(&wgsl_stripped) {
        let inserted = idents.insert(ty.ident.clone());
        if inserted {
            ordered_idents.push(ty.ident.clone());
        }
    }

    for (i, ident) in ordered_idents.iter().enumerate() {
        println!("ident #{i}: {ident}, count: {}", ident.use_count() - 2);
        ident.clone().rename(format!("{ident}{i}"));
    }

    println!("=== expectation ===\n{wgsl}");
    println!("=== test output ===\n{wgsl_stripped}");

    assert_eq!(wgsl.to_string(), wgsl_stripped.to_string())
}
