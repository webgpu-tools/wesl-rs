use std::collections::HashSet;

use wgsl_parse::syntax::{Ident, ModulePath, TranslationUnit};

use crate::{
    error::{Error, ImportError},
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
/// 1. Load the main module.
/// 2. Get the list declarations in the main module serving as entry points;
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
    /// Get the path of the main module.
    fn main_path(&self) -> &ModulePath;

    /// List identifiers of declarations serving as starting point for static usage analysis.
    ///
    /// Typically they are the entry points functions (`@verted`, `@fragment` and `@compute`),
    /// but some users might want to keep different declarations.
    fn main_entry_points(&self, main_module: &TranslationUnit) -> Result<HashSet<Ident>, Error> {
        Ok(pass::main_entry_points(main_module))
    }

    /// Find declarations used in external modules which this module depends on, no matter what.
    ///
    /// Currently, only items referenced by module-scope `const_assert`s are always included.
    ///
    /// See [`Self::usage_analysis`].
    fn module_usage_analysis(
        &self,
        module: &Module,
        already_used: &mut UsedItems,
        to_analyze: &mut UsedItems,
    ) -> Result<(), Error> {
        pass::module_usage_analysis(module, already_used, to_analyze);
        Ok(())
    }

    /// Find declaration names which a local declaration depends on.
    ///
    /// Adds *external* referenced idents to the `to_analyze` parameter.
    /// Perform usage analysis recursively with *local* referenced idents, and adds them to `already_used`.
    /// So at the end of the call, `to_analyze` contains incomplete usage analysis which needs to continue
    /// in a separate module. `already_used` contains finished analysis.
    fn usage_analysis(
        &self,
        module: &Module,
        decl_name: &str,
        already_used: &mut UsedItems,
        to_analyze: &mut UsedItems,
    ) -> Result<(), Error> {
        let found = pass::usage_analysis(module, decl_name, already_used, to_analyze);

        if !found {
            return Err(
                ImportError::MissingDecl(module.path.clone(), decl_name.to_string()).into(),
            );
        }

        Ok(())
    }

    /// Get the [`TranslationUnit`] for a module at a given path.
    ///
    /// This function is called only once per module path.
    fn load_module(&mut self, path: &ModulePath) -> Result<TranslationUnit, Error>;

    /// Assemble the list of loaded module into a final output.
    ///
    /// `used_items` contains the result of static usage analysis, i.e the list identifiers
    /// which the main module entrypoints depend on, transitively.
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
