use std::collections::{BTreeMap, HashMap, HashSet};

use itertools::Itertools;
use wgsl_parse::{
    SyntaxNode,
    syntax::{Ident, ModulePath, PathOrigin, TranslationUnit, Visibility},
};

use crate::{
    SyntaxUtil,
    error::{Diagnostic, Error, ImportError},
    pass::{self, CompileResult, CompilerDriver, ImportedItem, Module, UsedItems},
    resolver::{AsyncResolver, Resolver},
};

pub fn main_entry_points(main_module: &TranslationUnit) -> HashSet<Ident> {
    main_module.entry_points().collect()
}

/// Note: it does not call [`pass::retarget_idents()`], because that must be done right after [`pass::condcomp()`].
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

/// Insert explicit imports in place of wildcards for the module with path `path`
///
/// # Panics
///
/// If a module path (either `path` or `wildcards`) is not loaded in `modules`.
fn resolve_wildcards(
    path: &ModulePath,
    modules: &mut BTreeMap<ModulePath, Module>,
    wildcards: &[ModulePath],
) -> Result<(), Error> {
    if wildcards.is_empty() {
        return Ok(());
    }

    // There can be multiple wildcard-imports, and they can have conflicts (same name).
    // Conflicts are allowed, so long as the identifier is not used.
    let mut candidates = HashMap::<String, Vec<ImportedItem>>::new();

    for wildcard_path in wildcards {
        let imported_module = modules.get(wildcard_path).unwrap();

        // wildcard-importing from a different package requires public declarations.
        // TODO: also check that the source package has `@!wildcardable`.
        let min_vis = match (&path.origin, &wildcard_path.origin) {
            (PathOrigin::Package(a), PathOrigin::Package(b)) if a != b => Visibility::Public,
            _ => Visibility::Package,
        };

        for ident in wildcard_exports(imported_module, min_vis) {
            candidates
                .entry(ident.to_string())
                .or_default()
                .push(ImportedItem {
                    path: wildcard_path.clone(),
                    ident,
                    visibility: Visibility::Private,
                });
        }
    }

    let module = modules.get_mut(path).unwrap();
    for unknown_ident in pass::retarget_idents(&mut module.syntax) {
        if &*unknown_ident.name() == "_" {
            continue;
        }

        match candidates.get(&*unknown_ident.name()).map(Vec::as_slice) {
            Some([item]) => {
                module
                    .imports
                    .idents
                    .insert(unknown_ident.clone(), item.clone());
            }
            Some(_) => {
                return Err(Diagnostic::from(ImportError::AmbiguousWildcard(
                    unknown_ident.to_string(),
                ))
                .with_module_path(module.path.clone(), None)
                .into());
            }
            None => {
                // this is probably undefined. Validation (if enabled) will catch it.
            }
        };
    }

    Ok(())
}

/// Get the idents exported by the module and importable as wildcard-import with the given visibility
/// (public or package)
fn wildcard_exports(module: &Module, min_vis: Visibility) -> impl Iterator<Item = Ident> + '_ {
    module
        .syntax
        .global_declarations
        .iter()
        .filter_map(|decl| decl.ident().map(|ident| (ident, decl.visibility())))
        .chain(
            module
                .imports
                .idents
                .iter()
                .map(|(ident, item)| (ident.clone(), item.visibility)),
        )
        .filter(move |(_, vis)| *vis >= min_vis)
        .map(|(ident, _)| ident)
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

    let mut modules = BTreeMap::new();
    modules.insert(
        main_path.clone(),
        Module::new(main_path.clone(), main_module),
    );

    let mut used_items = UsedItems::new();
    let mut to_analyze = UsedItems::new();
    to_analyze.insert_module(main_path, main_entrypoints);

    loop {
        let mut next_to_analyze = UsedItems::new();

        for (path, items_to_analyze) in to_analyze.iter() {
            let path = driver.canonical_path(path);
            if !modules.contains_key(&path) {
                let syntax = driver.load_module(&path)?;
                modules.insert(path.clone(), Module::new(path.clone(), syntax));
            }

            let wildcards = std::mem::take(&mut modules.get_mut(&path).unwrap().imports.wildcards);
            for path in &wildcards {
                if !modules.contains_key(path) {
                    let syntax = driver.load_module(path)?;
                    modules.insert(path.clone(), Module::new(path.clone(), syntax));
                }
            }
            resolve_wildcards(&path, &mut modules, &wildcards)?;
            let module = modules.get(&path).unwrap();
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
    let main_entrypoints = driver
        .main_entry_points(&main_module)?
        .into_iter()
        .map(|ident| (ident, Visibility::Private)) // No visibility requirements for entry points
        .collect::<HashMap<Ident, Visibility>>();

    let mut modules = BTreeMap::new();
    modules.insert(
        main_path.clone(),
        Module::new(main_path.clone(), main_module),
    );

    let mut used_items = UsedItems::new();
    let mut to_analyze = UsedItems::new();
    to_analyze.insert_module(main_path, main_entrypoints);

    loop {
        let mut next_to_analyze = UsedItems::new();

        for (path, items_to_analyze) in to_analyze.iter() {
            let path = driver.canonical_path(path);
            if !modules.contains_key(&path) {
                let syntax = driver.load_module_async(&path).await?;
                modules.insert(path.clone(), Module::new(path.clone(), syntax));
            }

            let wildcards = std::mem::take(&mut modules.get_mut(&path).unwrap().imports.wildcards);
            for path in &wildcards {
                if !modules.contains_key(path) {
                    let syntax = driver.load_module_async(path).await?;
                    modules.insert(path.clone(), Module::new(path.clone(), syntax));
                }
            }
            resolve_wildcards(&path, &mut modules, &wildcards)?;
            let module = modules.get(&path).unwrap();
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

    let mut modules = modules.into_values().collect_vec();
    let final_module = driver.link(&mut modules, &used_items)?;

    Ok(CompileResult {
        syntax: final_module,
        modules,
        used_items,
    })
}
