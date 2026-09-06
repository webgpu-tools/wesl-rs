use std::collections::{HashMap, HashSet};

use wgsl_parse::syntax::{Ident, ModulePath, TranslationUnit, Visibility};

use crate::{
    SyntaxUtil,
    error::{Diagnostic, Error},
    pass::{self, CompileResult, CompilerDriver, Module, UsedItems},
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
    let main_entrypoints = driver
        .main_entry_points(&main_module)?
        .into_iter()
        .map(|ident| (ident, Visibility::Private)) // No visibility requirements for entry points
        .collect::<HashMap<Ident, Visibility>>();

    let mut modules = Vec::new();
    modules.push(Module::new(main_path.clone(), main_module));

    let mut used_items = UsedItems::new();
    let mut to_analyze = UsedItems::new();
    to_analyze.insert_module(main_path, main_entrypoints);

    loop {
        let mut next_to_analyze = UsedItems::new();

        for (path, items_to_analyze) in to_analyze.iter() {
            let path = driver.canonical_path(path);
            let module = match modules.iter().find(|module| module.path == path) {
                Some(module) => module,
                None => {
                    let module = driver.load_module(&path)?;
                    modules.push_mut(Module::new(path, module))
                }
            };

            driver.module_usage_analysis(module, &mut used_items, &mut next_to_analyze)?;

            for (item, min_vis) in items_to_analyze {
                driver.usage_analysis(
                    module,
                    &item.name(),
                    *min_vis,
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

pub async fn compile_async(driver: &mut impl CompilerDriver) -> Result<CompileResult, Error> {
    let main_path = driver.main_path().clone();
    let main_module = driver.load_module(&main_path)?;
    let main_entrypoints = driver
        .main_entry_points(&main_module)?
        .into_iter()
        .map(|ident| (ident, Visibility::Private)) // No visibility requirements for entry points
        .collect::<HashMap<Ident, Visibility>>();

    let mut modules = Vec::new();
    modules.push(Module::new(main_path.clone(), main_module));

    let mut used_items = UsedItems::new();
    let mut to_analyze = UsedItems::new();
    to_analyze.insert_module(main_path, main_entrypoints);

    loop {
        let mut next_to_analyze = UsedItems::new();

        for (path, items_to_analyze) in to_analyze.iter() {
            let path = driver.canonical_path(path);
            let module = match modules.iter().find(|module| module.path == path) {
                Some(module) => module,
                None => {
                    let module = driver.load_module_async(&path).await?;
                    modules.push_mut(Module::new(path, module))
                }
            };

            driver.module_usage_analysis(module, &mut used_items, &mut next_to_analyze)?;

            for (item, min_vis) in items_to_analyze {
                driver.usage_analysis(
                    module,
                    &item.name(),
                    *min_vis,
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
