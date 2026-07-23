use std::collections::HashSet;

use wgsl_parse::syntax::{Ident, ModulePath, TranslationUnit};

use crate::{
    SyntaxUtil,
    error::{Diagnostic, Error},
    pass::{self, AsyncCompilerDriver, CompileResult, CompilerDriver, Module, UsedItems},
    resolver::{AsyncResolver, Resolver},
};

pub fn main_entry_points(main_module: &TranslationUnit) -> HashSet<Ident> {
    main_module.entry_points().collect()
}

/// Note: it does not call [`pass::retarget_idents`], because that must be done right after [`pass::condcomp`].
pub fn load_module(path: &ModulePath, resolver: &impl Resolver) -> Result<TranslationUnit, Error> {
    let source = resolver.resolve_source(path)?;

    let module: TranslationUnit = source.parse().map_err(|e| {
        Diagnostic::from(e)
            .with_module_path(path.clone(), resolver.display_name(path))
            .with_source(source.to_string())
    })?;

    Ok(module)
}

pub async fn load_module_async(
    path: &ModulePath,
    resolver: &impl AsyncResolver,
) -> Result<TranslationUnit, Error> {
    let source = resolver.resolve_source_async(path).await?;

    let mut module: TranslationUnit = source.parse().map_err(|e| {
        Diagnostic::from(e)
            .with_module_path(path.clone(), resolver.display_name(path))
            .with_source(source.to_string())
    })?;

    pass::retarget_idents(&mut module);

    Ok(module)
}

/// Default implementation of [`CompilerDriver::compile`]
pub fn compile(driver: &mut impl CompilerDriver) -> Result<CompileResult, Error> {
    let main_path = driver.main_path().clone();
    let main_module = driver.load_module(&main_path)?;
    let main_entrypoints = driver.main_entry_points(&main_module)?;

    let mut modules = Vec::new();
    modules.push(Module::new(main_path.clone(), main_module));

    let mut used_items = UsedItems::new();
    let mut to_analyze = UsedItems::new();
    to_analyze.insert_module(main_path, main_entrypoints);

    loop {
        let mut next_to_analyze = UsedItems::new();

        for (path, items_to_analyze) in to_analyze.iter() {
            let module = match modules.iter().find(|module| module.path == *path) {
                Some(module) => module,
                None => {
                    let module = driver.load_module(path)?;
                    modules.push_mut(Module::new(path.clone(), module))
                }
            };

            driver.module_usage_analysis(module, &mut used_items, &mut next_to_analyze)?;

            for item in items_to_analyze {
                driver.usage_analysis(
                    module,
                    &item.name(),
                    &mut used_items,
                    &mut next_to_analyze,
                )?;
            }
        }

        if next_to_analyze.is_empty() {
            break;
        }

        to_analyze = next_to_analyze;
    }

    let final_module = driver.link(&mut modules, &used_items)?;

    Ok(CompileResult {
        syntax: final_module,
        modules,
        used_items,
    })
}

pub async fn compile_async(driver: &mut impl AsyncCompilerDriver) -> Result<CompileResult, Error> {
    let main_path = driver.main_path().clone();
    let main_module = driver.load_module(&main_path)?;
    let main_entrypoints = driver.main_entry_points(&main_module)?;

    let mut modules = Vec::new();
    modules.push(Module::new(main_path.clone(), main_module));

    let mut newly_used = UsedItems::new();
    let mut already_used = UsedItems::new();

    newly_used.insert_module(main_path, main_entrypoints);

    while !newly_used.is_empty() {
        let mut next_newly_used = UsedItems::new();

        for (path, used_items) in newly_used.iter() {
            let module = match modules.iter().find(|module| module.path == *path) {
                Some(module) => module,
                None => {
                    let module = driver.load_module_async(path).await?;
                    modules.push_mut(Module::new(path.clone(), module))
                }
            };

            for item in used_items {
                driver.usage_analysis(
                    module,
                    &item.name(),
                    &mut already_used,
                    &mut next_newly_used,
                )?;
            }
        }

        newly_used = next_newly_used;
    }

    let final_module = driver.link(&mut modules, &already_used)?;

    Ok(CompileResult {
        syntax: final_module,
        modules,
        used_items: already_used,
    })
}
