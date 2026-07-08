use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use itertools::Itertools;
use wgsl_parse::{SyntaxNode, syntax::*};

use crate::{Diagnostic, Error, Mangler, ResolveError, Resolver, SyntaxUtil, visit::Visit};

type Imports = HashMap<Ident, ImportedItem>;
type Modules = HashMap<ModulePath, Rc<RefCell<Module>>>;

#[derive(Clone, Debug)]
struct ImportedItem {
    path: ModulePath,
    ident: Ident, // this is the ident's original name before `as` renaming.
    public: bool,
}

type E = ImportError;

#[derive(Debug)]
pub(crate) struct Module {
    pub(crate) source: TranslationUnit,
    pub(crate) path: ModulePath,
    idents: HashMap<Ident, usize>,        // lookup (ident, decl_index)
    used_idents: RefCell<HashSet<Ident>>, // used idents that have already been usage-analyzed
    imports: Imports,
}

impl Module {
    fn new(source: TranslationUnit, path: ModulePath) -> Self {
        let idents = source
            .global_declarations
            .iter()
            .enumerate()
            .filter_map(|(i, decl)| decl.ident().map(|id| (id, i)))
            .collect::<HashMap<_, _>>();

        Self {
            source,
            path,
            idents,
            used_idents: Default::default(),
            imports: Default::default(),
        }
    }

    fn find_decl(&self, ident: &Ident) -> Option<(&Ident, &usize)> {
        self.idents.get_key_value(ident).or_else(|| {
            self.idents
                .iter()
                .find(|(id, _)| *id.name() == *ident.name())
        })
    }
    fn find_import(&self, ident: &Ident) -> Option<(&Ident, &ImportedItem)> {
        self.imports.get_key_value(ident).or_else(|| {
            self.imports
                .iter()
                .find(|(id, _)| *id.name() == *ident.name())
        })
    }
}

#[derive(Debug)]
pub(crate) struct Resolutions {
    modules: Modules,
    order: Vec<ModulePath>,
}

impl Resolutions {
    pub(crate) fn new(source: TranslationUnit, path: ModulePath) -> Self {
        let mut resol = Self::new_uninit();
        resol.push_module(Module::new(source, path));
        resol
    }
    /// Warning: you *must* call `push_module` right after this.
    pub fn new_uninit() -> Self {
        Resolutions {
            modules: Default::default(),
            order: Default::default(),
        }
    }
    #[allow(unused)]
    pub(crate) fn root_module(&self) -> Rc<RefCell<Module>> {
        self.modules.get(self.root_path()).unwrap().clone() // safety: new() requires push_module
    }
    pub(crate) fn root_path(&self) -> &ModulePath {
        self.order.first().unwrap() // safety: new() requires push_module
    }
    pub(crate) fn modules(&self) -> impl Iterator<Item = Rc<RefCell<Module>>> + '_ {
        self.order.iter().map(|i| self.modules[i].clone())
    }
    pub(crate) fn push_module(&mut self, module: Module) -> Rc<RefCell<Module>> {
        let path = module.path.clone();
        let module = Rc::new(RefCell::new(module));
        self.modules.insert(path.clone(), module.clone());
        self.order.push(path);
        module
    }
    pub(crate) fn into_module_order(self) -> Vec<ModulePath> {
        self.order
    }
}

fn err_with_module(e: Error, module: &Module, resolver: &impl Resolver) -> Error {
    Error::from(
        Diagnostic::from(e)
            .with_module_path(module.path.clone(), resolver.display_name(&module.path)),
    )
}

/// get or load a module with the resolver.
fn load_module<R: Resolver>(
    path: &ModulePath,
    resolutions: &mut Resolutions,
    resolver: &R,
    onload: &impl Fn(&Module, &mut Resolutions, &R) -> Result<(), Error>,
) -> Result<Rc<RefCell<Module>>, Error> {
    if let Some(module) = resolutions.modules.get(path) {
        return Ok(module.clone());
    }

    let source = resolver.resolve_module(path)?;
    load_module_with_source(source, path, resolutions, resolver, onload)
}

fn load_module_with_source<R: Resolver>(
    source: TranslationUnit,
    path: &ModulePath,
    resolutions: &mut Resolutions,
    resolver: &R,
    onload: &impl Fn(&Module, &mut Resolutions, &R) -> Result<(), Error>,
) -> Result<Rc<RefCell<Module>>, Error> {
    let module = Module::new(source, path.clone());
    let module = resolutions.push_module(module);

    let imports = flatten_imports(&module.borrow().source.imports, path);
    {
        let mut module = module.borrow_mut();
        module.imports = imports;
        module.source.retarget_idents();
    }

    {
        let module = module.borrow();
        onload(&module, resolutions, resolver)
            .map_err(|e| err_with_module(e, &module, resolver))?;
    }

    Ok(module)
}

/// load the modules that a declaration (named by its identifier) refers to, recursively.
/// the identifier must not be a builtin.
fn resolve_decl<R: Resolver>(
    module: &Module,
    ident: &Ident,
    resolutions: &mut Resolutions,
    resolver: &R,
    onload: &impl Fn(&Module, &mut Resolutions, &R) -> Result<(), Error>,
) -> Result<(), Error> {
    if let Some((_, n)) = module.find_decl(ident) {
        let decl = module.source.global_declarations.get(*n).unwrap().node();
        if let Some(ident) = decl.ident()
            && !module.used_idents.borrow_mut().insert(ident)
        {
            return Ok(());
        }

        for ty in Visit::<TypeExpression>::visit(decl) {
            resolve_ty(module, ty, resolutions, resolver, onload)?;
        }
        Ok(())
    } else if let Some((_, item)) = module.find_import(ident) {
        // the declaration can be a re-export (`@publish import`)
        if item.public {
            // load the external module for this imported item
            let ext_mod = load_module(&item.path, resolutions, resolver, onload)?;
            let ext_mod = ext_mod.borrow();
            resolve_decl(&ext_mod, &item.ident, resolutions, resolver, onload)
                .map_err(|e| err_with_module(e, &ext_mod, resolver))
        } else {
            Err(E::Private(ident.to_string(), module.path.clone()).into())
        }
    } else {
        Err(E::MissingDecl(module.path.clone(), ident.to_string()).into())
    }
}

/// load the modules that a TypeExpression refers to, recursively.
fn resolve_ty<R: Resolver>(
    module: &Module,
    ty: &TypeExpression,
    resolutions: &mut Resolutions,
    resolver: &R,
    onload: &impl Fn(&Module, &mut Resolutions, &R) -> Result<(), Error>,
) -> Result<(), Error> {
    // first, the recursive call
    for ty in Visit::<TypeExpression>::visit(ty) {
        resolve_ty(module, ty, resolutions, resolver, onload)?;
    }

    // get the path and identifier referred to by the TypeExpression, if it is imported
    let (ext_path, ext_id) = if let Some(path) = &ty.path {
        let path = resolve_inline_path(path, &module.path, &module.imports);
        (path, &ty.ident)
    } else if let Some(item) = module.imports.get(&ty.ident) {
        (item.path.clone(), &item.ident)
    } else {
        // This is a local declaration or a builtin, we mark the ident as used.
        if module.idents.contains_key(&ty.ident) {
            resolve_decl(module, &ty.ident, resolutions, resolver, onload)?;
        }
        return Ok(());
    };

    // if the import path points to a local declaration, we just check that it exists
    // and we're done.
    if ext_path == module.path {
        if module.idents.contains_key(&ty.ident) {
            return Ok(());
        } else {
            return Err(E::MissingDecl(ext_path, ty.ident.to_string()).into());
        }
    }

    // load the external module for this imported item
    let ext_mod = load_module(&ext_path, resolutions, resolver, &onload)?;
    let ext_mod = ext_mod.borrow();

    // and ensure the declaration's dependencies are resolved too
    resolve_decl(&ext_mod, ext_id, resolutions, resolver, onload)
        .map_err(|e| err_with_module(e, &ext_mod, resolver))
}

// XXX: it's quite messy.
/// Load all modules "used" transitively by the root module. Make external idents point at
/// the right declaration in the external module.
///
/// It is "lazy" because external modules are loaded only if used by the `keep` declarations
/// or module-scope `const_assert`s.
///
/// This approach is only valid when stripping is enabled. Otherwise, unused declarations
/// may refer to declarations in unused modules, and mangling will panic.
///
/// "used": used declarations in the root module are the `keep` parameter. Used declarations
/// in other modules are those reached by `keep` declarations, recursively.
/// Module-scope `const_assert`s are always included.
///
/// Returns a list of [`Module`]s with the list of their "used" idents.
///
/// See also: [`resolve_eager`]
pub fn resolve_lazy<'a>(
    keep: impl IntoIterator<Item = &'a Ident>,
    source: TranslationUnit,
    path: &ModulePath,
    resolver: &impl Resolver,
) -> Result<Resolutions, Error> {
    fn resolve_module(
        module: &Module,
        resolutions: &mut Resolutions,
        resolver: &impl Resolver,
    ) -> Result<(), Error> {
        // const_asserts of used modules must be included.
        // https://github.com/wgsl-tooling-wg/wesl-spec/issues/66
        let const_asserts = module
            .source
            .global_declarations
            .iter()
            .filter(|decl| decl.is_const_assert());

        for decl in const_asserts {
            for ty in Visit::<TypeExpression>::visit(decl.node()) {
                resolve_ty(module, ty, resolutions, resolver, &resolve_module)?;
            }
        }

        Ok(())
    }

    let mut resolutions = Resolutions::new_uninit();
    let module =
        load_module_with_source(source, path, &mut resolutions, resolver, &resolve_module)?;

    {
        let module = module.borrow();
        for id in keep {
            resolve_decl(&module, id, &mut resolutions, resolver, &resolve_module)
                .map_err(|e| err_with_module(e, &module, resolver))?;
        }
    }

    resolutions.retarget()?;
    Ok(resolutions)
}

/// Load all [`Module`]s referenced by the root module.
pub fn resolve_eager(
    source: TranslationUnit,
    path: &ModulePath,
    resolver: &impl Resolver,
) -> Result<Resolutions, Error> {
    fn resolve_module(
        module: &Module,
        resolutions: &mut Resolutions,
        resolver: &impl Resolver,
    ) -> Result<(), Error> {
        for decl in &module.source.global_declarations {
            if let Some(ident) = decl.ident() {
                resolve_decl(module, &ident, resolutions, resolver, &resolve_module)?;
            } else {
                for ty in Visit::<TypeExpression>::visit(decl.node()) {
                    resolve_ty(module, ty, resolutions, resolver, &resolve_module)?;
                }
            }
        }

        Ok(())
    }

    let mut resolutions = Resolutions::new_uninit();
    load_module_with_source(source, path, &mut resolutions, resolver, &resolve_module)?;

    resolutions.retarget()?;
    Ok(resolutions)
}

/// Flatten imports to a list.
fn flatten_imports(imports: &[ImportStatement], path: &ModulePath) -> Imports {
    fn rec(content: &ImportContent, path: ModulePath, public: bool, res: &mut Imports) {
        match content {
            ImportContent::Item(item) => {
                let ident = item.rename.as_ref().unwrap_or(&item.ident).clone();
                res.insert(
                    ident,
                    ImportedItem {
                        path,
                        ident: item.ident.clone(),
                        public,
                    },
                );
            }
            ImportContent::Collection(coll) => {
                for import in coll {
                    let path = path.clone().join(import.path.iter().cloned());
                    rec(&import.content, path, public, res);
                }
            }
        }
    }

    let mut res = Imports::default();

    for import in imports {
        let public = import.attributes.iter().any(|attr| attr.is_publish());
        match &import.path {
            Some(import_path) => {
                let path = path.join_path(import_path);
                rec(&import.content, path, public, &mut res);
            }
            None => {
                // this covers two cases: `import foo;` and `import {foo, ..};`.
                // COMBAK: these edge-cases smell
                match &import.content {
                    ImportContent::Item(_) => {
                        // `import foo`, this import statement does nothing currently.
                        // In the future, it may become a visibility/re-export mechanism.
                    }
                    ImportContent::Collection(coll) => {
                        for import in coll {
                            let mut components = import.path.iter().cloned();
                            if let Some(pkg_name) = components.next() {
                                // `import {foo::bar}`, foo becomes the package name.
                                let path = ModulePath::new(
                                    PathOrigin::Package(pkg_name),
                                    components.collect_vec(),
                                );
                                rec(&import.content, path, public, &mut res);
                            }
                        }
                    }
                }
            }
        }
    }

    res
}

/// Finds the normalized module path for an inline import.
///
/// Inline imports differ from import statements only in case of package imports:
/// the package component may refer to a local import shadowing the package name.
fn resolve_inline_path(
    path: &ModulePath,
    parent_path: &ModulePath,
    imports: &Imports,
) -> ModulePath {
    match &path.origin {
        PathOrigin::Package(pkg_name) => {
            // the path could be either a package, of referencing an imported module alias.
            let imported_item = imports.iter().find(|(ident, _)| *ident.name() == *pkg_name);

            if let Some((_, ext_item)) = imported_item {
                // this inline path references an imported item. Example:
                // import a::b::c as foo; foo::bar::baz() => a::b::c::bar::baz()
                let mut res = ext_item.path.clone(); // a::b
                res.push(&ext_item.ident.name()); // c
                res.join(path.components.iter().cloned())
            } else {
                parent_path.join_path(path)
            }
        }
        _ => parent_path.join_path(path),
    }
}

pub(crate) fn mangle_decls<'a>(
    wgsl: &'a mut TranslationUnit,
    path: &'a ModulePath,
    mangler: &impl Mangler,
) {
    wgsl.global_declarations
        .iter_mut()
        .filter_map(|decl| decl.ident())
        .for_each(|mut ident| {
            let new_name = mangler.mangle(path, &ident.name());
            ident.rename(new_name.clone());
        })
}

impl Resolutions {
    /// Retarget used identifiers to point at the corresponding declaration.
    ///
    /// We call this after resolve, because it is mutating the modules, and we want to keep
    /// mutations and lookups separate if possible, to avoid multiple mut borrows.
    ///
    /// Panics
    /// * if an identifier has no corresponding declaration.
    /// * if a module is already borrowed.
    fn retarget(&self) -> Result<(), Error> {
        fn find_ext_ident(
            modules: &Modules,
            src_path: &ModulePath,
            src_id: &Ident,
        ) -> Option<Ident> {
            // load the external module for this external ident
            let module = modules.get(src_path)?;
            // SAFETY: since this is an external ident, it cannot be in the currently
            // borrowed module.
            let module = module.borrow();

            module
                .find_decl(src_id)
                .map(|(id, _)| id.clone())
                .or_else(|| {
                    // or it could be a re-exported import with `@publish`
                    module
                        .find_import(src_id)
                        .and_then(|(_, item)| find_ext_ident(modules, &item.path, &item.ident))
                })
        }

        fn retarget_ty(
            modules: &Modules,
            module_path: &ModulePath,
            module_imports: &Imports,
            module_idents: &HashMap<Ident, usize>,
            ty: &mut TypeExpression,
        ) -> Result<(), Error> {
            // first the recursive call
            for ty in Visit::<TypeExpression>::visit_mut(ty) {
                retarget_ty(modules, module_path, module_imports, module_idents, ty)?;
            }

            let (ext_path, ext_id) = if let Some(path) = &ty.path {
                let res = resolve_inline_path(path, module_path, module_imports);
                (res, &ty.ident)
            } else if let Some(item) = module_imports.get(&ty.ident) {
                (item.path.clone(), &item.ident)
            } else {
                // points to a local decl, we stop here.
                return Ok(());
            };

            // if the import path points to a local decl.
            // this must be a special case to avoid 2 mut borrows of the current module.
            if ext_path == *module_path {
                let local_id = module_idents
                    .iter()
                    .find(|(id, _)| *id.name() == *ext_id.name())
                    .map(|(id, _)| id.clone())
                    .ok_or_else(|| E::MissingDecl(ext_path, ext_id.to_string()))?;
                ty.path = None;
                ty.ident = local_id;
            }
            // get the ident of the external declaration pointed to by the type
            else if let Some(ext_id) = find_ext_ident(modules, &ext_path, ext_id) {
                ty.path = None;
                ty.ident = ext_id;
            }
            // the imported ident is used, but has no declaration!
            // this code path should not be reached, as this is already checked in resolve().
            else {
                return Err(E::MissingDecl(ext_path, ext_id.to_string()).into());
            }

            Ok(())
        }

        for module in self.modules.values() {
            let mut module = module.borrow_mut();
            let module = &mut *module;

            for decl in &mut module.source.global_declarations {
                // we only retarged used declarations. Other declarations are not checked.
                // unused declarations can even contain invalid code.
                if let Some(id) = decl.ident()
                    && !module.used_idents.borrow().contains(&id)
                {
                    continue;
                }

                for ty in Visit::<TypeExpression>::visit_mut(decl.node_mut()) {
                    retarget_ty(
                        &self.modules,
                        &module.path,
                        &module.imports,
                        &module.idents,
                        ty,
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Mangle all declarations in all modules. Should be called after [`Self::retarget`].
    ///
    /// Panics if a module is already borrowed.
    pub(crate) fn mangle(&mut self, mangler: &impl Mangler, mangle_root: bool) {
        let root_path = self.root_path().clone();
        for (path, module) in self.modules.iter_mut() {
            if mangle_root || path != &root_path {
                let mut module = module.borrow_mut();
                mangle_decls(&mut module.source, path, mangler);
            }
        }
    }

    /// Merge all declarations into a single module. If the `strip` flag is set, it will
    /// copy over only used declarations.
    pub(crate) fn assemble(&self, strip: bool) -> TranslationUnit {
        let mut wesl = TranslationUnit::default();
        for module in self.modules() {
            let module = module.borrow();
            if strip {
                wesl.global_declarations.extend(
                    module
                        .source
                        .global_declarations
                        .iter()
                        .filter(|decl| {
                            decl.is_const_assert()
                                || decl
                                    .ident()
                                    .is_some_and(|id| module.used_idents.borrow().contains(&id))
                        })
                        .cloned(),
                );
            } else {
                wesl.global_declarations
                    .extend(module.source.global_declarations.clone());
            }
            wesl.global_directives
                .extend(module.source.global_directives.clone());
        }
        // TODO: <https://github.com/wgsl-tooling-wg/wesl-spec/issues/71>
        // currently the behavior is:
        // * include all directives used (if strip)
        // * include all directives (if not strip)
        wesl.global_directives.dedup();
        wesl
    }
}
