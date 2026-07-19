use std::collections::HashSet;

use wgsl_parse::syntax::{Ident, ModulePath, TranslationUnit};

use crate::{
    SyntaxUtil,
    error::{Diagnostic, Error},
    pass::{self, Imports, UsedItems},
    resolver::{AsyncResolver, Resolver},
};

pub struct Module {
    pub syntax: TranslationUnit,
    pub path: ModulePath,
    pub imports: Imports,
}

impl Module {
    pub fn new(path: ModulePath, syntax: TranslationUnit) -> Self {
        let imports = pass::flatten_imports(&syntax.imports, &path);
        Self {
            syntax,
            path,
            imports,
        }
    }
}
/// Re-implementing this trait gives full control over the steps of the compilation pipeline.
/// It is implemented by [`Compiler`].
///
/// The steps, default-implemented in [`Self::compile`], are:
///
/// 1. Load the root module.
/// 2. Get the list declarations in the root module serving as entry points;
///    they are the basis of static usage analysis.
/// 3. Run static usage analysis: for each entry point, collect the list of declarations it depends on, transitively. Usage analysis returns the set of imported identifiers (defined in other modules).
/// 4. Load missing imported modules found via usage analysis.
/// 5. Run the previous two steps for with imported modules/identifiers, until all identifiers have been usage-analyzed.
/// 6. Assemble loaded modules into a final module.
///
/// WESL features are implemented at different steps of this pipeline:
///
/// * Conditional translation runs after module loading.
/// * Name mangling runs before assembly.
pub trait CompilerDriver: Sized {
    /// Get the path of the root module.
    fn root_path(&self) -> &ModulePath;

    /// List identifiers of declarations serving as starting point for static usage analysis.
    ///
    /// Typically they are the entry points functions (`@verted`, `@fragment` and `@compute`),
    /// but some users might want to keep different declarations.
    fn root_entry_points(&self, root_module: &TranslationUnit) -> Result<HashSet<Ident>, Error> {
        Ok(root_entry_points(root_module))
    }

    /// Perform static usage analysis of a module.
    ///
    /// Find identifiers of global declarations that a declaration named `decl_name` depends on.
    /// * If the dependency is implemented in the current module, it is added to `already_used`,
    ///   and is usage-analyzed recursively.
    /// * Otherwise, if it lives in another module, and is not in `already_used` it is added to `newly_used`.
    fn usage_analysis(
        &self,
        module: &Module,
        decl_name: &str,
        already_used: &mut UsedItems,
        newly_used: &mut UsedItems,
    ) -> Result<(), Error> {
        Ok(pass::usage_analysis(
            module,
            decl_name,
            already_used,
            newly_used,
        ))
    }

    /// Get the [`TranslationUnit`] for a module at a given path.
    ///
    /// This function is called only once per module path.
    fn load_module(&mut self, path: &ModulePath) -> Result<TranslationUnit, Error>;

    /// Assemble the list of loaded module into a final output.
    ///
    /// `used_items` contains the result of static usage analysis, i.e the list identifiers
    /// which the root module entrypoints depend on, transitively.
    fn link(&self, modules: Vec<Module>, used_items: &UsedItems) -> Result<TranslationUnit, Error>;

    /// Run the compilation pipeline.
    fn compile(&mut self) -> Result<TranslationUnit, Error> {
        compile(self)
    }
}

pub trait AsyncCompilerDriver: CompilerDriver {
    async fn load_module_async(&mut self, path: &ModulePath) -> Result<TranslationUnit, Error>;

    /// Run the compilation pipeline.
    async fn compile_async(&mut self) -> Result<TranslationUnit, Error> {
        compile_async(self).await
    }
}

pub fn root_entry_points(root_module: &TranslationUnit) -> HashSet<Ident> {
    root_module.entry_points().collect()
}

pub fn load_module(path: &ModulePath, resolver: &impl Resolver) -> Result<TranslationUnit, Error> {
    let source = resolver.resolve_source(path)?;

    let mut module: TranslationUnit = source.parse().map_err(|e| {
        Diagnostic::from(e)
            .with_module_path(path.clone(), resolver.display_name(path))
            .with_source(source.to_string())
    })?;

    pass::retarget_idents(&mut module);

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

pub fn compile(driver: &mut impl CompilerDriver) -> Result<TranslationUnit, Error> {
    let root_path = driver.root_path().clone();
    let root_module = driver.load_module(&root_path)?;
    let root_entrypoints = driver.root_entry_points(&root_module)?;

    let mut modules = Vec::new();
    modules.push(Module::new(root_path.clone(), root_module));

    let mut newly_used = UsedItems::new();
    let mut already_used = UsedItems::new();

    newly_used.insert_module(root_path, root_entrypoints);

    while !newly_used.is_empty() {
        let mut next_newly_used = UsedItems::new();

        for (path, used_items) in &newly_used.used_items {
            let module = match modules.iter().find(|module| module.path == *path) {
                Some(module) => module,
                None => {
                    let module = driver.load_module(path)?;
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

    let final_module = driver.link(modules, &already_used)?;

    Ok(final_module)
}

pub async fn compile_async(
    driver: &mut impl AsyncCompilerDriver,
) -> Result<TranslationUnit, Error> {
    let root_path = driver.root_path().clone();
    let root_module = driver.load_module(&root_path)?;
    let root_entrypoints = driver.root_entry_points(&root_module)?;

    let mut modules = Vec::new();
    modules.push(Module::new(root_path.clone(), root_module));

    let mut newly_used = UsedItems::new();
    let mut already_used = UsedItems::new();

    newly_used.insert_module(root_path, root_entrypoints);

    while !newly_used.is_empty() {
        let mut next_newly_used = UsedItems::new();

        for (path, used_items) in &newly_used.used_items {
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

    let final_module = driver.link(modules, &already_used)?;

    Ok(final_module)
}
