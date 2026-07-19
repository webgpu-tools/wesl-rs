#![doc = include_str!("../README.md")]
#![allow(clippy::missing_safety_doc)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ops::Deref;
use std::os::raw::{c_char, c_void};
use std::ptr;

use wesl::syntax::{ModulePath, TranslationUnit};
use wesl::{
    Compiler,
    error::ResolveError,
    resolver::{Resolver, VirtualResolver},
};

#[cfg(feature = "eval")]
use wesl::{
    eval::{Eval, EvalAttrs, Inputs, Instance, RefInstance},
    syntax::{AccessMode, AddressSpace},
};

// TODO: this seems unfinished. only wesl_create/destroy_compiler is implemented.
#[allow(unused)]
pub struct WeslCompiler {
    compiler: Compiler,
}

pub struct WeslTranslationUnit {
    unit: TranslationUnit,
}

/// cbindgen:rename-all=ScreamingSnakeCase
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum WeslManglerKind {
    WeslManglerEscape = 0,
    WeslManglerHash = 1,
    WeslManglerNone = 2,
}

/// cbindgen:rename-all=ScreamingSnakeCase
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum WeslBindingType {
    WeslBindingUniform = 0,
    WeslBindingStorage = 1,
    WeslBindingReadOnlyStorage = 2,
    WeslBindingFiltering = 3,
    WeslBindingNonFiltering = 4,
    WeslBindingComparison = 5,
    WeslBindingFloat = 6,
    WeslBindingUnfilterableFloat = 7,
    WeslBindingSint = 8,
    WeslBindingUint = 9,
    WeslBindingDepth = 10,
    WeslBindingWriteOnly = 11,
    WeslBindingReadWrite = 12,
    WeslBindingReadOnly = 13,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WeslBinding {
    pub group: u32,
    pub binding: u32,
    pub kind: WeslBindingType,
    pub data_len: usize,
    pub data: *const u8,
}

#[repr(C)]
pub struct WeslResolveSourceResult {
    pub success: bool,
    pub source: *const c_char,
}

#[repr(C)]
pub struct WeslResolveModuleResult {
    pub success: bool,
    pub module: *mut WeslTranslationUnit,
}

pub type WeslResolveSourceFunction = unsafe extern "C" fn(
    path: *const c_char,
    userdata: *mut c_void,
) -> *mut WeslResolveSourceResult;

pub type WeslResolveSourceFreeFunction =
    unsafe extern "C" fn(result: *const WeslResolveSourceResult, userdata: *mut c_void);

pub type WeslResolveModuleFunction = unsafe extern "C" fn(
    path: *const c_char,
    userdata: *mut c_void,
) -> *mut WeslResolveModuleResult;
pub type WeslResolveModuleFreeFunction =
    unsafe extern "C" fn(result: *const WeslResolveModuleResult, userdata: *mut c_void);

// Workaround for https://github.com/mozilla/cbindgen/issues/326

pub type WeslResolveModuleFunctionOption = Option<
    unsafe extern "C" fn(
        path: *const c_char,
        userdata: *mut c_void,
    ) -> *mut WeslResolveModuleResult,
>;
pub type WeslResolveModuleFreeFunctionOption =
    Option<unsafe extern "C" fn(result: *const WeslResolveModuleResult, userdata: *mut c_void)>;

pub type WeslResolveStringFunction =
    unsafe extern "C" fn(path: *const c_char, userdata: *mut c_void) -> *const c_char;
pub type WeslResolveFreeStringFunction =
    unsafe extern "C" fn(result: *const c_char, userdata: *mut c_void);

pub type WeslResolveStringFunctionOption =
    Option<unsafe extern "C" fn(path: *const c_char, userdata: *mut c_void) -> *const c_char>;
pub type WeslResolveFreeStringFunctionOption =
    Option<unsafe extern "C" fn(result: *const c_char, userdata: *mut c_void)>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WeslResolverOptions {
    pub userdata: *mut std::ffi::c_void,

    pub resolve_source: WeslResolveSourceFunction,
    pub resolve_source_free: WeslResolveSourceFreeFunction,

    pub display_name: WeslResolveStringFunctionOption,
    pub free_display_name: WeslResolveFreeStringFunctionOption,

    pub fs_path: WeslResolveStringFunctionOption,
    pub free_fs_path: WeslResolveFreeStringFunctionOption,
}

#[repr(C)]
pub struct WeslCompileOptions {
    pub mangler: WeslManglerKind,
    pub sourcemap: bool,
    pub imports: bool,
    pub condcomp: bool,
    pub generics: bool,
    pub strip: bool,
    pub lower: bool,
    pub validate: bool,
    pub naga: bool,
    pub lazy: bool,
    pub keep_root: bool,
    pub mangle_root: bool,
    pub resolver: *const WeslResolverOptions,
}
#[repr(C)]
pub struct WeslStringMap {
    pub keys: *const *const c_char,
    pub values: *const *const c_char,
    pub len: usize,
}

#[repr(C)]
pub struct WeslBoolMap {
    pub keys: *const *const c_char,
    pub values: *const bool,
    pub len: usize,
}
#[repr(C)]
pub struct WeslStringArray {
    pub items: *const *const c_char,
    pub len: usize,
}

#[repr(C)]
pub struct WeslBindingArray {
    pub items: *const WeslBinding,
    pub len: usize,
}

#[repr(C)]
pub struct WeslDiagnostic {
    pub file: *const c_char,
    pub span_start: usize,
    pub span_end: usize,
    pub title: *const c_char,
}

#[repr(C)]
pub struct WeslError {
    pub source: *const c_char,
    pub message: *const c_char,
    pub diagnostics: *const WeslDiagnostic,
    pub diagnostics_len: usize,
}

#[repr(C)]
pub struct WeslResult {
    pub success: bool,
    pub data: *const c_char,
    pub error: WeslError,
}

#[repr(C)]
pub struct WeslParseResult {
    pub success: bool,
    pub data: *const WeslTranslationUnit,
    pub error: WeslError,
}

#[repr(C)]
pub struct WeslExecResult {
    pub success: bool,
    pub resources: *const WeslBindingArray,
    pub error: WeslError,
}

fn map_mangler_kind(value: WeslManglerKind) -> Option<wesl::ManglerKind> {
    match value {
        WeslManglerKind::WeslManglerNone => Some(wesl::ManglerKind::None),
        WeslManglerKind::WeslManglerHash => Some(wesl::ManglerKind::Hash),
        WeslManglerKind::WeslManglerEscape => Some(wesl::ManglerKind::Escape),
    }
}

// -- helpers

unsafe fn string_map_to_hashmap(map: &WeslStringMap) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for i in 0..map.len {
        unsafe {
            let key_ptr = *map.keys.add(i);
            let value_ptr = *map.values.add(i);

            if !key_ptr.is_null() && !value_ptr.is_null() {
                let key = CStr::from_ptr(key_ptr).to_string_lossy().into_owned();
                let value = CStr::from_ptr(value_ptr).to_string_lossy().into_owned();
                result.insert(key, value);
            }
        }
    }

    result
}

unsafe fn bool_map_to_hashmap(map: &WeslBoolMap) -> HashMap<String, bool> {
    let mut result = HashMap::new();

    for i in 0..map.len {
        unsafe {
            let key_ptr = *map.keys.add(i);
            let value = *map.values.add(i);

            if !key_ptr.is_null() {
                let key = CStr::from_ptr(key_ptr).to_string_lossy().into_owned();
                result.insert(key, value);
            }
        }
    }

    result
}

unsafe fn string_array_to_vec(array: &WeslStringArray) -> Option<Vec<String>> {
    let mut result = Vec::new();

    for i in 0..array.len {
        unsafe {
            let item_ptr = *array.items.add(i);
            if !item_ptr.is_null() {
                let item = CStr::from_ptr(item_ptr).to_string_lossy().into_owned();
                result.push(item);
            }
        }
    }

    Some(result)
}

fn create_c_string(s: &str) -> *const c_char {
    match CString::new(s) {
        Ok(c_str) => {
            let ptr = c_str.as_ptr();
            std::mem::forget(c_str);
            ptr
        }
        Err(_) => ptr::null(),
    }
}

fn wesl_error_to_c(e: wesl::Error) -> WeslError {
    let d = wesl::error::Diagnostic::from(e);

    let diagnostics = if let (Some(span), Some(res)) = (&d.detail.span, &d.detail.module_path) {
        let diag = WeslDiagnostic {
            file: create_c_string(&res.components.join("/")),
            span_start: span.start,
            span_end: span.end,
            title: create_c_string(&d.error.to_string()),
        };

        let boxed = Box::new(diag);

        Box::into_raw(boxed)
    } else {
        ptr::null()
    };

    WeslError {
        source: d
            .detail
            .output
            .as_ref()
            .map_or(ptr::null(), |s| create_c_string(s)),
        message: create_c_string(&d.to_string()),
        diagnostics,
        diagnostics_len: if diagnostics.is_null() {
            0
        } else {
            1
        },
    }
}

#[cfg(feature = "eval")]
unsafe fn binding_array_to_vec(array: &WeslBindingArray) -> Vec<WeslBinding> {
    let mut result = Vec::new();

    for i in 0..array.len {
        let binding = unsafe { *array.items.add(i) };
        result.push(binding);
    }

    result
}

#[cfg(feature = "eval")]
fn parse_c_binding(
    b: &WeslBinding,
    wgsl: &wesl::syntax::TranslationUnit,
) -> Result<((u32, u32), RefInstance), wesl::Error> {
    let mut ctx = wesl::eval::Context::new(wgsl);

    let ty_expr = wgsl
        .global_declarations
        .iter()
        .find_map(|d| match d.node() {
            wesl::syntax::GlobalDeclaration::Declaration(d) => {
                let (group, binding) = d.attr_group_binding(&mut ctx).ok()?;
                if group == b.group && binding == b.binding {
                    d.ty.clone()
                } else {
                    None
                }
            }
            _ => None,
        })
        .ok_or_else(|| {
            wesl::Error::Custom(format!(
                "Resource @group({}) @binding({}) not found",
                b.group, b.binding
            ))
        })?;

    let ty = wesl::eval::ty_eval_ty(&ty_expr, &mut ctx)
        .map_err(|e| wesl::Error::Custom(format!("Failed to evaluate type: {e}")))?;

    let (storage, access) = match b.kind {
        WeslBindingType::WeslBindingUniform => (AddressSpace::Uniform, AccessMode::Read),
        WeslBindingType::WeslBindingStorage => (AddressSpace::Storage, AccessMode::ReadWrite),
        WeslBindingType::WeslBindingReadOnlyStorage => (AddressSpace::Storage, AccessMode::Read),
        _ => return Err(wesl::Error::Custom("Unsupported binding type".to_string())),
    };

    let data_slice = unsafe { std::slice::from_raw_parts(b.data, b.data_len) };
    let inst = Instance::from_buffer(data_slice, &ty).ok_or_else(|| {
        wesl::Error::Custom(format!(
            "Resource @group({}) @binding({}) ({} bytes) incompatible with type ({} bytes)",
            b.group,
            b.binding,
            b.data_len,
            ty.size_of().unwrap_or_default()
        ))
    })?;

    Ok((
        (b.group, b.binding),
        RefInstance::new(inst, storage, access),
    ))
}

#[cfg(feature = "eval")]
fn create_c_binding_array(bindings: Vec<WeslBinding>) -> *const WeslBindingArray {
    if bindings.is_empty() {
        return ptr::null();
    }

    let items = bindings.into_boxed_slice();
    let len = items.len();
    let items_ptr = Box::into_raw(items) as *const WeslBinding;

    let array = Box::new(WeslBindingArray {
        items: items_ptr,
        len,
    });
    Box::into_raw(array)
}

// -- main API

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_create_compiler() -> *mut WeslCompiler {
    let compiler = Compiler::default();
    Box::into_raw(Box::new(WeslCompiler { compiler }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_destroy_compiler(compiler: *mut WeslCompiler) {
    if !compiler.is_null() {
        let _ = unsafe { Box::from_raw(compiler) };
    }
}

fn error_from_str(s: &str) -> WeslError {
    WeslError {
        source: ptr::null(),
        message: create_c_string(s),
        diagnostics: ptr::null(),
        diagnostics_len: 0,
    }
}

fn result_from_str(s: &str) -> WeslResult {
    WeslResult {
        success: false,
        data: ptr::null(),
        error: error_from_str(s),
    }
}

fn result_invalid_parameters() -> WeslResult {
    result_from_str("Invalid parameters")
}

const NO_ERROR: WeslError = WeslError {
    source: ptr::null(),
    message: ptr::null(),
    diagnostics: ptr::null(),
    diagnostics_len: 0,
};

struct CustomResolver {
    pub options: WeslResolverOptions,
}

struct FreeGuard<T> {
    pub data: *const T,
    pub free_function: unsafe extern "C" fn(*const T, *mut c_void),
    pub free_userdata: *mut c_void,
}

impl<T> Drop for FreeGuard<T> {
    fn drop(&mut self) {
        unsafe {
            (self.free_function)(self.data, self.free_userdata);
        }
    }
}

impl<T> Deref for FreeGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data }
    }
}

impl FreeGuard<c_char> {
    unsafe fn c_str(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.data) }
    }
}

fn mod_path_to_cstring(path: &ModulePath) -> CString {
    let path = path.to_string();

    CString::new(path).expect("Module path contained nul bytes!")
}

unsafe fn resolver_path_to_string<T, F: FnOnce(Cow<'_, str>) -> T>(
    path: &ModulePath,
    transform: F,
    get_func: Option<WeslResolveStringFunction>,
    free_func: Option<WeslResolveFreeStringFunction>,
    userdata: *mut c_void,
) -> Option<T> {
    let get_func = get_func?;
    let free_func = free_func?;

    let cstring = mod_path_to_cstring(path);

    let result = unsafe { get_func(cstring.as_ptr(), userdata) };

    if result.is_null() {
        return None;
    }

    let result = FreeGuard {
        data: result,
        free_function: free_func,
        free_userdata: userdata,
    };

    let result_cstr = unsafe { result.c_str() };
    let result_str = result_cstr.to_string_lossy();

    let ret_result = transform(result_str);

    Some(ret_result)
}

/// Helper type to call the base `resolve_module` implementation.
struct ProxyResolver<'a, T>(&'a T);

impl<T: Resolver> wesl::Resolver for ProxyResolver<'_, T> {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        self.0.resolve_source(path)
    }
}

impl wesl::Resolver for CustomResolver {
    fn resolve_source<'a>(
        &'a self,
        path: &ModulePath,
    ) -> Result<std::borrow::Cow<'a, str>, ResolveError> {
        let cstring = mod_path_to_cstring(path);

        let result =
            unsafe { (self.options.resolve_source)(cstring.as_ptr(), self.options.userdata) };

        if result.is_null() {
            return Err(ResolveError::Error(
                wesl::Error::Custom("No value returned from resolver".into()).into(),
            ));
        }

        let result = FreeGuard {
            data: result,
            free_function: self.options.resolve_source_free,
            free_userdata: self.options.userdata,
        };

        if !result.success {
            // TODO: Better error reporting.
            return Err(ResolveError::Error(
                wesl::Error::Custom("Custom resolver failed".into()).into(),
            ));
        }

        let result_cstr = unsafe { CStr::from_ptr(result.source) };
        let result_str = result_cstr.to_str().map_err(|_| {
            ResolveError::Error(
                wesl::Error::Custom("Resolved source is not valid UTF-8".into()).into(),
            )
        })?;

        Ok(result_str.to_owned().into())
    }

    fn display_name(&self, path: &ModulePath) -> Option<String> {
        unsafe {
            resolver_path_to_string(
                path,
                |str| str.into_owned(),
                self.options.display_name,
                self.options.free_display_name,
                self.options.userdata,
            )
        }
    }

    fn fs_path(&self, path: &ModulePath) -> Option<std::path::PathBuf> {
        unsafe {
            resolver_path_to_string(
                path,
                |str| str.deref().into(),
                self.options.display_name,
                self.options.free_display_name,
                self.options.userdata,
            )
        }
    }
}

fn validate_resolver_options(options: &WeslResolverOptions) -> Result<(), &'static str> {
    if options.fs_path.is_none() ^ options.free_fs_path.is_none() {
        return Err("fs_path and free_fs_path must both be provide if either is");
    }

    if options.display_name.is_none() ^ options.free_display_name.is_none() {
        return Err("display_name and free_display_name must both be provide if either is");
    }

    Ok(())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_compile(
    files: Option<&WeslStringMap>,
    root: *const c_char,
    options: &WeslCompileOptions,
    keep: &WeslStringArray,
    features: &WeslBoolMap,
) -> WeslResult {
    if root.is_null() {
        return result_invalid_parameters();
    }

    let root_str = unsafe { CStr::from_ptr(root).to_string_lossy() };
    let keep_vec = unsafe { string_array_to_vec(keep) };
    let features_map = unsafe { bool_map_to_hashmap(features) };

    let root_path = match root_str.parse() {
        Ok(path) => path,
        Err(e) => return result_from_str(&format!("Invalid root path: {e}")),
    };

    let resolver: Box<dyn wesl::Resolver> = match (files, options.resolver.is_null()) {
        (Some(files), true) => {
            let files_map = unsafe { string_map_to_hashmap(files) };
            let mut resolver = VirtualResolver::new();
            for (path, source) in files_map {
                if let Ok(module_path) = path.parse() {
                    resolver.add_module(module_path, source.into());
                }
            }

            Box::new(resolver)
        }
        (None, false) => {
            let resolver_options = unsafe { &*options.resolver };
            if let Err(msg) = validate_resolver_options(resolver_options) {
                return result_from_str(msg);
            }

            Box::new(CustomResolver {
                options: *resolver_options,
            })
        }
        (Some(_), false) => {
            return result_from_str("Files and custom resolver cannot be specified at once");
        }
        _ => return result_from_str("Files or custom resolver must be specified"),
    };

    let Some(mangler) = map_mangler_kind(options.mangler) else {
        return result_from_str("Invalid mangler kind specified");
    };

    let mut compiler = Compiler::new(wesl::CompileOptions {
        imports: options.imports,
        condcomp: options.condcomp,
        generics: options.generics,
        strip: options.strip,
        lower: options.lower,
        validate: options.validate,
        mangle_root: options.mangle_root,
        keep: keep_vec,
        features: wesl::Features {
            default: wesl::Feature::Disable,
            flags: features_map
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        },
        keep_root: options.keep_root,
        mangler,
        constants: todo!(),
        dependencies: todo!(),
    })
    .set_custom_resolver(resolver);
    let compiler = compiler
        .use_sourcemap(options.sourcemap)
        .set_mangler(mangler);

    match compiler.compile(&root_path) {
        Ok(result) => {
            let output = result.to_string();
            WeslResult {
                success: true,
                data: create_c_string(&output),
                error: NO_ERROR,
            }
        }
        Err(e) => WeslResult {
            success: false,
            data: ptr::null(),
            error: wesl_error_to_c(e),
        },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_parse(source: *const c_char) -> WeslParseResult {
    if source.is_null() {
        return WeslParseResult {
            success: false,
            data: ptr::null_mut(),
            error: error_from_str("Invalid parameters"),
        };
    };

    let cstr = unsafe { CStr::from_ptr(source) };
    let Ok(str) = cstr.to_str() else {
        return WeslParseResult {
            success: false,
            data: ptr::null_mut(),
            error: error_from_str("Source is not valid UTF-8"),
        };
    };

    match str.parse::<TranslationUnit>() {
        Ok(unit) => {
            let ptr = Box::into_raw(Box::new(WeslTranslationUnit { unit }));
            WeslParseResult {
                success: true,
                data: ptr,
                error: NO_ERROR,
            }
        }
        Err(e) => WeslParseResult {
            success: false,
            data: ptr::null_mut(),
            error: wesl_error_to_c(wesl::Error::ParseError(e)),
        },
    }
}

#[cfg(feature = "eval")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_eval(
    files: &WeslStringMap,
    root: *const c_char,
    expression: *const c_char,
    options: &WeslCompileOptions,
    features: Option<&WeslBoolMap>,
) -> WeslResult {
    let files_map = unsafe { string_map_to_hashmap(files) };
    let root_str = unsafe { CStr::from_ptr(root).to_string_lossy() };
    let expr_str = unsafe { CStr::from_ptr(expression).to_string_lossy() };
    let features_map = features
        .map(|features| unsafe { bool_map_to_hashmap(features) })
        .unwrap_or_default();

    let root_path = match root_str.parse() {
        Ok(path) => path,
        Err(e) => {
            return WeslResult {
                success: false,
                data: ptr::null(),
                error: WeslError {
                    source: ptr::null(),
                    message: create_c_string(&format!("Invalid root path: {e}")),
                    diagnostics: ptr::null(),
                    diagnostics_len: 0,
                },
            };
        }
    };

    let mut resolver = VirtualResolver::new();
    for (path, source) in files_map {
        if let Ok(module_path) = path.parse() {
            resolver.add_module(module_path, source.into());
        }
    }

    let mut compiler = Compiler::new(wesl::CompileOptions {
        imports: options.imports,
        condcomp: options.condcomp,
        generics: options.generics,
        strip: options.strip,
        lower: options.lower,
        validate: options.validate,
        // lazy: options.lazy,
        mangle_root: options.mangle_root,
        keep: None,
        features: wesl::Features {
            default: wesl::Feature::Disable,
            flags: features_map
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        },
        keep_root: options.keep_root,
        mangler: options.mangler.into(),
        constants: todo!(),
        dependencies: todo!(),
    })
    .set_custom_resolver(resolver);
    // let compiler = compiler
    //     .use_sourcemap(options.sourcemap)
    //     .set_mangler(map_mangler_kind(options.mangler).expect("invalid mangler kind"));

    match compiler.compile(&root_path) {
        Ok(result) => match result.eval(&expr_str) {
            Ok(eval_result) => WeslResult {
                success: true,
                data: create_c_string(&eval_result.inst.to_string()),
                error: WeslError {
                    source: ptr::null(),
                    message: ptr::null(),
                    diagnostics: ptr::null(),
                    diagnostics_len: 0,
                },
            },
            Err(e) => WeslResult {
                success: false,
                data: ptr::null(),
                error: wesl_error_to_c(e),
            },
        },
        Err(e) => WeslResult {
            success: false,
            data: ptr::null(),
            error: wesl_error_to_c(e),
        },
    }
}

#[cfg(not(feature = "eval"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_eval(
    _files: *const WeslStringMap,
    _root: *const c_char,
    _expression: *const c_char,
    _options: *const WeslCompileOptions,
    _features: *const WeslBoolMap,
) -> WeslResult {
    result_from_str("wesl_eval requires the 'eval' feature to be enabled")
}

#[cfg(feature = "eval")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_exec(
    files: &WeslStringMap,
    root: &c_char,
    entrypoint: &c_char,
    options: &WeslCompileOptions,
    resources: Option<&WeslBindingArray>,
    overrides: Option<&WeslStringMap>,
    features: Option<&WeslBoolMap>,
) -> WeslExecResult {
    let files_map = unsafe { string_map_to_hashmap(files) };
    let root_str = unsafe { CStr::from_ptr(root).to_string_lossy() };
    let entrypoint_str = unsafe { CStr::from_ptr(entrypoint).to_string_lossy() };
    let resources_vec = resources
        .map(|resources| unsafe { binding_array_to_vec(resources) })
        .unwrap_or_default();
    let overrides_map = overrides
        .map(|overrides| unsafe { string_map_to_hashmap(overrides) })
        .unwrap_or_default();
    let features_map: HashMap<String, bool> = features
        .map(|features| unsafe { bool_map_to_hashmap(features) })
        .unwrap_or_default();

    let root_path = match root_str.parse() {
        Ok(path) => path,
        Err(e) => {
            return WeslExecResult {
                success: false,
                resources: ptr::null(),
                error: WeslError {
                    source: ptr::null(),
                    message: create_c_string(&format!("Invalid root path: {e}")),
                    diagnostics: ptr::null(),
                    diagnostics_len: 0,
                },
            };
        }
    };

    let mut resolver = VirtualResolver::new();
    for (path, source) in files_map {
        if let Ok(module_path) = path.parse() {
            resolver.add_module(module_path, source.into());
        }
    }

    let mut compiler = Wesl::new_barebones().set_custom_resolver(resolver);
    let compiler = compiler
        .set_options(wesl::CompileOptions {
            imports: options.imports,
            condcomp: options.condcomp,
            generics: options.generics,
            strip: options.strip,
            lower: options.lower,
            validate: options.validate,
            lazy: options.lazy,
            mangle_root: options.mangle_root,
            keep: None,
            features: wesl::Features {
                default: wesl::Feature::Disable,
                flags: features_map
                    .into_iter()
                    .map(|(k, v)| (k, v.into()))
                    .collect(),
            },
            keep_root: options.keep_root,
        })
        .use_sourcemap(options.sourcemap)
        .set_mangler(map_mangler_kind(options.mangler).expect("invalid mangler kind"));

    match compiler.compile(&root_path) {
        Ok(result) => {
            // parse resources
            let parsed_resources: Result<HashMap<(u32, u32), RefInstance>, wesl::Error> =
                resources_vec
                    .iter()
                    .map(|r| parse_c_binding(r, &result.syntax))
                    .collect();

            let parsed_resources = match parsed_resources {
                Ok(resources) => resources,
                Err(e) => {
                    return WeslExecResult {
                        success: false,
                        resources: ptr::null(),
                        error: wesl_error_to_c(e),
                    };
                }
            };

            // parse overrides
            let parsed_overrides: Result<HashMap<String, Instance>, wesl::Error> = overrides_map
                .iter()
                .map(|(name, expr)| {
                    let mut ctx = wesl::eval::Context::new(&result.syntax);
                    let expr = expr.parse::<wesl::syntax::Expression>().map_err(|e| {
                        wesl::Error::Custom(format!("Failed to parse override expression: {e}"))
                    })?;
                    let inst = expr.eval_value(&mut ctx).map_err(|e| {
                        wesl::Error::Custom(format!("Failed to evaluate override: {e}"))
                    })?;
                    Ok((name.clone(), inst))
                })
                .collect();

            let parsed_overrides = match parsed_overrides {
                Ok(overrides) => overrides,
                Err(e) => {
                    return WeslExecResult {
                        success: false,
                        resources: ptr::null(),
                        error: wesl_error_to_c(e),
                    };
                }
            };

            // execute
            let inputs = Inputs::new_zero_initialized();
            match result.exec(&entrypoint_str, inputs, parsed_resources, parsed_overrides) {
                Ok(exec_result) => {
                    // convert resources back to C format
                    let output_resources: Vec<WeslBinding> = resources_vec
                        .iter()
                        .filter_map(|r| {
                            let resource = exec_result.resource(r.group, r.binding)?;
                            let inst = resource.read().ok()?.to_owned();
                            let mut new_binding = *r;
                            if let Some(buffer) = inst.to_buffer() {
                                let boxed_data = buffer.into_boxed_slice();
                                new_binding.data_len = boxed_data.len();
                                new_binding.data = Box::into_raw(boxed_data) as *const u8;
                            }
                            Some(new_binding)
                        })
                        .collect();

                    WeslExecResult {
                        success: true,
                        resources: create_c_binding_array(output_resources),
                        error: WeslError {
                            source: ptr::null(),
                            message: ptr::null(),
                            diagnostics: ptr::null(),
                            diagnostics_len: 0,
                        },
                    }
                }
                Err(e) => WeslExecResult {
                    success: false,
                    resources: ptr::null(),
                    error: wesl_error_to_c(e),
                },
            }
        }
        Err(e) => WeslExecResult {
            success: false,
            resources: ptr::null(),
            error: wesl_error_to_c(e),
        },
    }
}

#[cfg(not(feature = "eval"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_exec(
    _files: *const WeslStringMap,
    _root: *const c_char,
    _entrypoint: *const c_char,
    _options: *const WeslCompileOptions,
    _resources: *const WeslBindingArray,
    _overrides: *const WeslStringMap,
    _features: *const WeslBoolMap,
) -> WeslExecResult {
    WeslExecResult {
        success: false,
        resources: ptr::null(),
        error: error_from_str("wesl_exec requires the 'eval' feature to be enabled"),
    }
}

// -- memory

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_free_string(ptr: *const c_char) {
    if !ptr.is_null() {
        let _ = unsafe { CString::from_raw(ptr as *mut c_char) };
    }
}

unsafe fn free_error(error: &WeslError) {
    unsafe {
        if !error.source.is_null() {
            wesl_free_string(error.source);
        }

        if !error.message.is_null() {
            wesl_free_string(error.message);
        }

        if !error.diagnostics.is_null() {
            let diag = &*error.diagnostics;
            if !diag.file.is_null() {
                wesl_free_string(diag.file);
            }
            if !diag.title.is_null() {
                wesl_free_string(diag.title);
            }
            let _ = Box::from_raw(error.diagnostics as *mut WeslDiagnostic);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_free_result(result: *mut WeslResult) {
    if !result.is_null() {
        unsafe {
            let result = &mut *result;

            if !result.data.is_null() {
                wesl_free_string(result.data);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_free_exec_result(result: *mut WeslExecResult) {
    if !result.is_null() {
        unsafe {
            let result = &mut *result;

            if !result.resources.is_null() {
                let resources = &*result.resources;

                // free each binding
                for i in 0..resources.len {
                    let binding = *resources.items.add(i);
                    if !binding.data.is_null() {
                        let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                            binding.data as *mut u8,
                            binding.data_len,
                        ));
                    }
                }

                let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                    resources.items as *mut WeslBinding,
                    resources.len,
                ));

                let _ = Box::from_raw(result.resources as *mut WeslBindingArray);
            }

            free_error(&result.error);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_free_parse_result(result: *mut WeslParseResult) {
    if !result.is_null() {
        unsafe {
            let result = &*result;

            free_error(&result.error);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_free_translation_unit(unit: *mut WeslTranslationUnit) {
    if !unit.is_null() {
        let _ = unsafe { Box::from_raw(unit) };
    }
}

// -- utility

// note: results from this function must not be freed
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wesl_version() -> *const c_char {
    const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}
