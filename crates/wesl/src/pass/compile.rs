use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use wgsl_parse::{
    SyntaxNode,
    syntax::{Ident, ModulePath, TranslationUnit},
};

use crate::{
    SyntaxUtil,
    error::{Diagnostic, Error},
    pass::{self, CompileResult, CompilerDriver, ImportedItem, Module, UsedItems},
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

fn get_or_load_module<'a>(
    path: &ModulePath,
    modules: &'a mut HashMap<ModulePath, Module>,
    driver: &mut impl CompilerDriver,
) -> Result<&'a mut Module, Error> {
    let canonical_path = driver.canonical_path(path);
    ensure_module_loaded(&canonical_path, modules, driver)?;

    Ok(modules
        .get_mut(&canonical_path)
        .unwrap(/* SAFETY: module was just loaded */))
}

fn ensure_module_loaded(
    canonical_path: &ModulePath,
    modules: &mut HashMap<ModulePath, Module>,
    driver: &mut impl CompilerDriver,
) -> Result<(), Error> {
    if modules.contains_key(canonical_path) {
        return Ok(());
    }

    let syntax = driver.load_module(canonical_path)?;
    let mut module = Module::new(canonical_path.clone(), syntax);
    // take the wildcards out and insert imports below in the for loop
    let wildcards = std::mem::take(&mut module.imports.wildcards);
    modules.insert(canonical_path.clone(), module);

    for wildcard_path in wildcards {
        let wildcard_imports = get_or_load_module(&wildcard_path, modules, driver)?
            .syntax
            .global_declarations
            .iter()
            .filter_map(|decl| decl.ident())
            .map(|ident| {
                (
                    ident.clone(),
                    ImportedItem {
                        path: wildcard_path.clone(),
                        ident,
                        public: false, // TODO: public
                    },
                )
            })
            .collect_vec();

        modules
            .get_mut(canonical_path)
            .unwrap(/* SAFETY: module was inserted above */)
            .imports
            .idents
            .extend(wildcard_imports);
    }

    Ok(())
}

/// Default implementation of [`CompilerDriver::compile`]
pub fn compile(driver: &mut impl CompilerDriver) -> Result<CompileResult, Error> {
    let main_path = driver.main_path().clone();
    let main_module = driver.load_module(&main_path)?;
    let main_entrypoints = driver.main_entry_points(&main_module)?;

    let mut modules = HashMap::new();
    get_or_load_module(&main_path, &mut modules, driver)?;

    let mut used_items = UsedItems::new();
    let mut to_analyze = UsedItems::new();
    to_analyze.insert_module(main_path, main_entrypoints);

    loop {
        let mut next_to_analyze = UsedItems::new();

        for (path, items_to_analyze) in to_analyze.iter() {
            let module = get_or_load_module(path, &mut modules, driver)?;
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

    let mut modules = modules.into_values().collect_vec();
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
