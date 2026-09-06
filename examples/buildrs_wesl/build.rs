use std::{borrow::Cow, collections::HashMap};

use wesl::{
    eval::{Context, Type, ty_eval_ty},
    sourcemap::{BasicSourceMap, SourceMap, SourceMapEntry},
    syntax::{GlobalDeclaration, ModulePath, PathOrigin},
};

/// Analogue of [`std::println!`] that allows for printing to the console during a build
/// script.
#[macro_export]
macro_rules! println {
    () => {
        ::std::println!("cargo:warning=\x1b[2K\r");
    };
    ($($arg:tt)*) => {
        ::std::println!("cargo:warning=\x1b[2K\r{}", ::std::format!($($arg)*));
    }
}

struct CodegenContext<'a> {
    compile_result: &'a wesl::CompileResult,
    modules: HashMap<ModulePath, Vec<CodegenDeclaration>>,
}

impl<'a> CodegenContext<'a> {
    fn new(compile_result: &'a wesl::CompileResult) -> Self {
        let modules = compile_result
            .modules
            .iter()
            .map(|module| (module.path.clone(), Vec::new()))
            .collect();
        Self {
            compile_result,
            modules,
        }
    }

    fn collect(&mut self) {
        let mut ctx = Context::new(&self.compile_result.syntax);

        let sourcemap = self
            .compile_result
            .sourcemap
            .as_ref()
            .expect("sourcemap missing");

        for declaration in &self.compile_result.syntax.global_declarations {
            if let Some((path, declaration)) =
                self.collect_declaration(&declaration, sourcemap, &mut ctx)
            {
                self.modules
                    .get_mut(&path)
                    .expect("module not found")
                    .push(declaration);
            }
        }
    }

    fn print(&self) {
        for (path, declarations) in &self.modules {
            println!("Module: {}", path);
            for declaration in declarations {
                match declaration {
                    CodegenDeclaration::Struct { name, members } => {
                        println!("struct {name} {{");
                        for member in members {
                            println!("  {}: {:?},", member.name, member.ty);
                        }
                        println!("}}");
                    }
                }
            }
        }
    }

    fn collect_declaration(
        &self,
        declaration: &GlobalDeclaration,
        sourcemap: &BasicSourceMap,
        ctx: &mut Context,
    ) -> Option<(ModulePath, CodegenDeclaration)> {
        match declaration {
            GlobalDeclaration::Void => None,
            GlobalDeclaration::Declaration(_) => None,
            GlobalDeclaration::TypeAlias(_) => None,
            GlobalDeclaration::Struct(r#struct) => {
                let name = sourcemap
                    .item(&r#struct.ident.name())
                    .map(Cow::Borrowed)
                    // Why are those not in the source map?
                    .unwrap_or_else(|| {
                        Cow::Owned(SourceMapEntry {
                            name: r#struct.ident.name().to_string(),
                            // Really? package::package?
                            path: ModulePath::new(
                                PathOrigin::Absolute,
                                vec!["package".to_string()],
                            ),
                            span: None,
                        })
                    });
                let members = r#struct
                    .members
                    .iter()
                    .map(|member| {
                        let name = member.ident.name();
                        let ty = ty_eval_ty(&member.ty, ctx).unwrap();
                        Some(CodegenStructMember {
                            name: name.to_string(),
                            ty,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some((
                    name.path.clone(),
                    CodegenDeclaration::Struct {
                        name: name.name.clone(),
                        members,
                    },
                ))
            }
            GlobalDeclaration::Function(_) => None,
            GlobalDeclaration::ConstAssert(_) => None,
            GlobalDeclaration::Compound(_) => None,
        }
    }
}

enum CodegenDeclaration {
    Struct {
        name: String,
        members: Vec<CodegenStructMember>,
    },
}

struct CodegenStructMember {
    name: String,
    ty: Type,
}

fn main() {
    let result = wesl::Compiler::new(wesl::CompileOptions {
        dependencies: vec![&random_wgsl::PACKAGE],
        // TODO: Document that this is important!
        strip: false,
        ..Default::default()
    })
    .compile("src/shaders/")
    .inspect_err(|e| eprintln!("{e}")) // pretty-print errors
    .expect("compilation error");

    result.write_artifact("main");

    let mut codegen_ctx = CodegenContext::new(&result);
    codegen_ctx.collect();
    codegen_ctx.print();
}
