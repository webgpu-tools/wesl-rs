use wesl::syntax::*;
use wesl_quote::quote_module;

#[test]
fn marker_injection() {
    let inject_struct = Struct::new(Ident::new("mystruct".to_string()));
    let inject_func = Function::new(Ident::new("myfunc".to_string()));
    let inject_stmt = Statement::Void;
    let inject_expr = 1f32;
    let wgsl = quote_module! {
        #decl@inject_struct
        #decl@inject_func
        fn foo() {
            #stmt@inject_stmt
            let x: f32 = #expr@inject_expr;
        }
    };

    // two injected global decls + `fn foo`
    assert_eq!(wgsl.global_declarations.len(), 3);

    let names: Vec<_> = wgsl
        .global_declarations
        .iter()
        .filter_map(|d| match d.node() {
            GlobalDeclaration::Struct(s) => Some(s.ident.to_string()),
            GlobalDeclaration::Function(f) => Some(f.ident.to_string()),
            _ => None,
        })
        .collect();
    assert!(names.contains(&"mystruct".to_string()));
    assert!(names.contains(&"myfunc".to_string()));
    assert!(names.contains(&"foo".to_string()));

    // find `foo` and inspect its body
    let foo = wgsl
        .global_declarations
        .iter()
        .find_map(|d| match d.node() {
            GlobalDeclaration::Function(f) if f.ident.to_string() == "foo" => Some(f),
            _ => None,
        })
        .expect("foo not found");

    // first statement is the injected `Statement::Void`
    assert_eq!(foo.body.statements[0].node(), &Statement::Void);

    // second statement declares `x` with initializer the injected literal `1f32`
    match foo.body.statements[1].node() {
        Statement::Declaration(decl) => {
            assert_eq!(decl.ident.to_string(), "x");
            let init = decl.initializer.as_ref().unwrap();
            assert_eq!(
                init.node(),
                &Expression::Literal(LiteralExpression::F32(1.0))
            );
        }
        s => panic!("unexpected statement: {s:?}"),
    }
}

#[test]
fn bare_ident_injection() {
    let ty = Ident::new("u32".to_string());
    let wgsl = quote_module! {
        struct S { field: #ty }
    };
    assert_eq!(wgsl.global_declarations.len(), 1);
}

#[test]
fn mem_and_attr_markers() {
    let inject_mem = StructMember {
        attributes: Vec::new(),
        ident: Ident::new("injected_field".to_string()),
        ty: TypeExpression::from(Ident::new("u32".to_string())),
    };
    let inject_attr = Attribute::Custom(CustomAttribute {
        name: "myattr".to_string(),
        arguments: None,
    });

    let wgsl = quote_module! {
        struct S {
            #mem@inject_mem,
            other: f32,
        }
        #attr@inject_attr
        fn foo() {}
    };

    let s = wgsl
        .global_declarations
        .iter()
        .find_map(|d| match d.node() {
            GlobalDeclaration::Struct(s) => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(s.members[0].ident.to_string(), "injected_field");

    let foo = wgsl
        .global_declarations
        .iter()
        .find_map(|d| match d.node() {
            GlobalDeclaration::Function(f) => Some(f),
            _ => None,
        })
        .unwrap();
    match foo.attributes[0].node() {
        Attribute::Custom(a) => assert_eq!(a.name, "myattr"),
        a => panic!("unexpected attr: {a:?}"),
    }
}
