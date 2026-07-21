use std::collections::HashSet;

use wgsl_parse::syntax::{Ident, ModulePath, TranslationUnit};

use crate::{
    error::Error,
    pass::{self, Module, UsedItems},
};

pub struct CompileResult {
    pub syntax: TranslationUnit,
    pub modules: Vec<Module>,
    pub used_items: UsedItems,
}

/// Re-implementing this trait gives full control over the steps of the compilation pipeline.
/// It is implemented by [`crate::Compiler`].
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
        Ok(pass::root_entry_points(root_module))
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
    fn link(
        &self,
        modules: &mut Vec<Module>,
        used_items: &UsedItems,
    ) -> Result<TranslationUnit, Error>;

    /// Run the compilation pipeline.
    ///
    /// See standalone default implementation in [`pass::compile`].
    fn compile(&mut self) -> Result<CompileResult, Error> {
        pass::compile(self)
    }
}

pub trait AsyncCompilerDriver: CompilerDriver {
    fn load_module_async(
        &mut self,
        path: &ModulePath,
    ) -> impl Future<Output = Result<TranslationUnit, Error>>;

    /// Run the compilation pipeline.
    ///
    /// See standalone default implementation in [`pass::compile_async`].
    fn compile_async(&mut self) -> impl Future<Output = Result<CompileResult, Error>> {
        pass::compile_async(self)
    }
}
