use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tsify::Tsify;
use wasm_bindgen::prelude::*;
use wesl::{
    CompileResult, Compiler,
    eval::{Eval, EvalAttrs, Inputs, Instance, RefInstance, Ty, ty_eval_ty},
    resolver::VirtualResolver,
    syntax::{self, AccessMode, AddressSpace, TranslationUnit},
};

#[derive(Tsify, Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(rename_all = "lowercase")]
pub enum ManglerKind {
    #[default]
    Escape,
    Hash,
    None,
}

impl From<ManglerKind> for wesl::ManglerKind {
    fn from(value: ManglerKind) -> Self {
        match value {
            ManglerKind::Escape => wesl::ManglerKind::Escape,
            ManglerKind::Hash => wesl::ManglerKind::Hash,
            ManglerKind::None => wesl::ManglerKind::None,
        }
    }
}

#[derive(Tsify, Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub enum Command {
    Compile(CompileOptions),
    Eval(EvalOptions),
    Exec(ExecOptions),
    Dump(DumpOptions),
}

#[derive(Tsify, Clone, Debug, Default, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(rename_all = "lowercase")]
pub enum Feature {
    Enable,
    Disable,
    // for simplicity in the CLI the default is Disable, but I guess in the playground it's nicer to keep.
    #[default]
    Keep,
    Error,
}

impl From<Feature> for wesl::Feature {
    fn from(feature: Feature) -> Self {
        match feature {
            Feature::Enable => wesl::Feature::Enable,
            Feature::Disable => wesl::Feature::Disable,
            Feature::Keep => wesl::Feature::Keep,
            Feature::Error => wesl::Feature::Error,
        }
    }
}

#[derive(Tsify, Clone, Debug, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct CompileOptions {
    #[tsify(type = "{ [name: string]: string }")]
    pub files: HashMap<String, String>,
    pub root: String,
    #[serde(default)]
    pub mangler: ManglerKind,
    pub sourcemap: bool,
    pub imports: bool,
    pub condcomp: bool,
    pub generics: bool,
    pub strip: bool,
    pub lower: bool,
    pub validate: bool,
    pub naga: bool,
    pub lazy: bool,
    #[serde(default)]
    pub keep: Option<Vec<String>>,
    pub keep_root: bool,
    pub mangle_root: bool,
    #[tsify(type = "{ [name: string]: Feature }")]
    pub features: HashMap<String, Feature>,
    #[serde(default)]
    pub features_default: Feature,
}

#[derive(Tsify, Clone, Copy, Debug, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
enum BindingType {
    #[serde(rename = "uniform")]
    Uniform,
    #[serde(rename = "storage")]
    Storage,
    #[serde(rename = "read-only-storage")]
    ReadOnlyStorage,
    #[serde(rename = "filtering")]
    Filtering,
    #[serde(rename = "non-filtering")]
    NonFiltering,
    #[serde(rename = "comparison")]
    Comparison,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "unfilterable-float")]
    UnfilterableFloat,
    #[serde(rename = "sint")]
    Sint,
    #[serde(rename = "uint")]
    Uint,
    #[serde(rename = "depth")]
    Depth,
    #[serde(rename = "write-only")]
    WriteOnly,
    #[serde(rename = "read-write")]
    ReadWrite,
    #[serde(rename = "read-only")]
    ReadOnly,
}

#[derive(Tsify, Clone, Debug, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Binding {
    group: u32,
    binding: u32,
    kind: BindingType,
    #[tsify(type = "Uint8Array")]
    #[serde(with = "serde_bytes")]
    data: Box<[u8]>,
}

#[derive(Tsify, Clone, Debug, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct EvalOptions {
    #[serde(flatten)]
    pub compile: CompileOptions,
    pub expression: String,
}

#[derive(Tsify, Clone, Debug, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ExecOptions {
    #[serde(flatten)]
    pub compile: CompileOptions,
    pub entrypoint: String,
    #[serde(default)]
    pub resources: Vec<Binding>,
    #[serde(default)]
    #[tsify(type = "{ [name: string]: string }")]
    pub overrides: HashMap<String, String>,
}

#[derive(Tsify, Clone, Debug, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DumpOptions {
    source: String,
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("resource `@group({0}) @binding({1})` not found")]
    ResourceNotFound(u32, u32),
    #[error(
        "resource `@group({0}) @binding({1})` ({2} bytes) incompatible with type `{3}` ({4} bytes)"
    )]
    ResourceIncompatible(u32, u32, u32, wesl::eval::Type, u32),
    #[error("Could not convert instance to buffer (type `{0}` is not storable)")]
    NotStorable(wesl::eval::Type),
    #[error("{0}")]
    Wesl(#[from] wesl::Error),
    #[error("{0}")]
    Diagnostic(#[from] wesl::error::Diagnostic),
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Diagnostic {
    file: String,
    span: std::ops::Range<u32>,
    title: String,
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Error {
    source: Option<String>,
    message: String,
    diagnostics: Vec<Diagnostic>,
}

fn run_compile(args: CompileOptions) -> Result<CompileResult, wesl::Error> {
    let mut resolver = VirtualResolver::new();
    let root = args.root.parse().map_err(|e| {
        wesl::Error::Custom(format!("`{}` is not a valid module path: {e}", args.root))
    })?;

    for (path, source) in args.files {
        let path = path.parse().map_err(|e| {
            wesl::Error::Custom(format!("`{path}` is not a valid module path: {e}"))
        })?;
        resolver.add_module(path, source.into());
    }

    let comp = Compiler::new(wesl::CompileOptions {
        imports: args.imports,
        condcomp: args.condcomp,
        generics: args.generics,
        strip: args.strip,
        lower: args.lower,
        validate: args.validate,
        sourcemap: args.sourcemap,
        mangler: args.mangler.into(),
        mangle_root: args.mangle_root,
        keep: args.keep,
        keep_root: args.keep_root,
        features: wesl::Features {
            default: args.features_default.into(),
            flags: args
                .features
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        },
        constants: Default::default(),    // TODO
        dependencies: Default::default(), // TODO
    })
    .set_resolver(resolver)
    .compile_module(&root)?;
    Ok(comp)
}

fn parse_binding(
    b: &Binding,
    wgsl: &TranslationUnit,
) -> Result<((u32, u32), RefInstance), CliError> {
    let mut ctx = wesl::eval::Context::new(wgsl);

    let ty_expr = wgsl
        .global_declarations
        .iter()
        .find_map(|d| match d.node() {
            syntax::GlobalDeclaration::Declaration(d) => {
                let (group, binding) = d.attr_group_binding(&mut ctx).ok()?;
                if group == b.group && binding == b.binding {
                    d.ty.clone()
                } else {
                    None
                }
            }
            _ => None,
        })
        .ok_or(CliError::ResourceNotFound(b.group, b.binding))?;

    let ty = ty_eval_ty(&ty_expr, &mut ctx).map_err(|e| {
        wesl::error::Diagnostic::from(e)
            .with_ctx(&ctx)
            .with_source(ty_expr.to_string())
    })?;
    let (storage, access) = match b.kind {
        BindingType::Uniform => (AddressSpace::Uniform, AccessMode::Read),
        BindingType::Storage => (AddressSpace::Storage, AccessMode::ReadWrite),
        BindingType::ReadOnlyStorage => (AddressSpace::Storage, AccessMode::Read),
        BindingType::Filtering => todo!(),
        BindingType::NonFiltering => todo!(),
        BindingType::Comparison => todo!(),
        BindingType::Float => todo!(),
        BindingType::UnfilterableFloat => todo!(),
        BindingType::Sint => todo!(),
        BindingType::Uint => todo!(),
        BindingType::Depth => todo!(),
        BindingType::WriteOnly => todo!(),
        BindingType::ReadWrite => todo!(),
        BindingType::ReadOnly => todo!(),
    };
    let inst = Instance::from_buffer(&b.data, &ty).ok_or_else(|| {
        CliError::ResourceIncompatible(
            b.group,
            b.binding,
            b.data.len() as u32,
            ty.clone(),
            ty.size_of().unwrap_or_default(),
        )
    })?;
    Ok((
        (b.group, b.binding),
        RefInstance::new(inst, storage, access),
    ))
}

fn parse_override(src: &str, wgsl: &TranslationUnit) -> Result<Instance, CliError> {
    let mut ctx = wesl::eval::Context::new(wgsl);
    let expr = src
        .parse::<syntax::Expression>()
        .map_err(|e| wesl::error::Diagnostic::from(e).with_source(src.to_string()))?;
    let inst = expr.eval_value(&mut ctx).map_err(|e| {
        wesl::error::Diagnostic::from(e)
            .with_ctx(&ctx)
            .with_source(src.to_string())
    })?;
    Ok(inst)
}

#[wasm_bindgen]
#[cfg_attr(
    not(feature = "debug"),
    allow(unused_variables, reason = "must exist, but not needed")
)]
pub fn init_log(level: &str) {
    #[cfg(feature = "debug")]
    {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            let level = match level {
                "Error" => log::Level::Error,
                "Warn" => log::Level::Warn,
                "Info" => log::Level::Info,
                "Debug" => log::Level::Debug,
                "Trace" => log::Level::Trace,
                _ => log::Level::Info,
            };
            console_log::init_with_level(level).expect("error initializing log");
        })
    }
}

fn wesl_err_to_diagnostic(e: wesl::Error, source: Option<String>) -> Error {
    log::debug!("[WESL] error: {e:?}");
    let d = wesl::error::Diagnostic::from(e);
    Error {
        source: source.or_else(|| d.detail.output.clone()),
        #[cfg(feature = "ansi-to-html")]
        message: ansi_to_html::convert(&d.to_string()).unwrap(),
        #[cfg(not(feature = "ansi-to-html"))]
        message: d.to_string(),
        diagnostics: {
            if let (Some(span), Some(res)) = (&d.detail.span, &d.detail.module_path) {
                vec![Diagnostic {
                    file: res.components.join("/"),
                    span: span.start as u32..span.end as u32,
                    title: d.error.to_string(),
                }]
            } else {
                vec![]
            }
        },
    }
}

#[cfg(feature = "naga")]
fn run_naga(src: &str) -> Result<(), Error> {
    use naga::back::wgsl::WriterFlags;
    use naga::valid::{Capabilities, ValidationFlags};
    let module = naga::front::wgsl::parse_str(src).map_err(|e| Error {
        source: Some(src.to_string()),
        message: e.message().to_string(),
        diagnostics: vec![],
    })?;
    let mut validator = naga::valid::Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = validator.validate(&module).map_err(|e| Error {
        source: Some(src.to_string()),
        message: e.emit_to_string(src),
        diagnostics: e
            .spans()
            .map(|(span, msg)| {
                let range = span.to_range().unwrap_or_default();
                Diagnostic {
                    file: "output".to_string(),
                    span: range.start as u32..range.end as u32,
                    title: msg.to_string(),
                }
            })
            .collect(),
    })?;
    let flags = WriterFlags::EXPLICIT_TYPES;
    naga::back::wgsl::write_string(&module, &info, flags).map_err(|e| Error {
        source: Some(src.to_string()),
        message: e.to_string(),
        diagnostics: vec![],
    })?;
    Ok(())
}

enum RunResult {
    Compile(TranslationUnit),
    Dump(TranslationUnit),
    Eval(Instance),
    Exec(Vec<Binding>),
}

fn run_impl(args: Command) -> Result<RunResult, Error> {
    match args {
        Command::Compile(args) => {
            let comp = run_compile(args).map_err(|e| wesl_err_to_diagnostic(e, None))?;

            Ok(RunResult::Compile(comp.syntax))
        }
        Command::Eval(args) => {
            let comp =
                run_compile(args.compile.clone()).map_err(|e| wesl_err_to_diagnostic(e, None))?;

            let eval = comp
                .eval(&args.expression)
                .map_err(|e| wesl_err_to_diagnostic(e, Some(comp.to_string())))?;

            Ok(RunResult::Eval(eval.inst))
        }
        Command::Exec(args) => {
            let comp =
                run_compile(args.compile.clone()).map_err(|e| wesl_err_to_diagnostic(e, None))?;

            let resources = (|| -> Result<_, CliError> {
                let inputs = Inputs::new_zero_initialized();

                let resources = args
                    .resources
                    .iter()
                    .map(|b| parse_binding(b, &comp.syntax))
                    .collect::<Result<_, _>>()?;

                let overrides = args
                    .overrides
                    .iter()
                    .map(|(name, expr)| -> Result<(String, Instance), CliError> {
                        Ok((name.to_string(), parse_override(expr, &comp.syntax)?))
                    })
                    .collect::<Result<_, _>>()?;

                let exec = comp.exec(&args.entrypoint, inputs, resources, overrides)?;

                let resources = args
                    .resources
                    .iter()
                    .map(|r| {
                        let inst = exec
                            .resource(r.group, r.binding)
                            .ok_or(CliError::ResourceNotFound(r.group, r.binding))?
                            .clone();
                        let inst = inst
                            .read()
                            .map_err(|e| wesl::Error::EvalError(e.into()))?
                            .to_owned();
                        let mut res = r.clone();
                        res.data = inst
                            .to_buffer()
                            .ok_or_else(|| CliError::NotStorable(inst.ty()))?
                            .into_boxed_slice();
                        Ok(res)
                    })
                    .collect::<Result<Vec<_>, CliError>>()?;

                Ok(resources)
            })()
            .map_err(|e| match e {
                CliError::Wesl(e) => wesl_err_to_diagnostic(e, Some(comp.to_string())),
                e => Error {
                    source: Some(comp.to_string()),
                    message: e.to_string(),
                    diagnostics: Vec::new(),
                },
            })?;

            Ok(RunResult::Exec(resources))
        }
        Command::Dump(args) => {
            let wesl = args
                .source
                .parse::<syntax::TranslationUnit>()
                .map_err(|e| wesl_err_to_diagnostic(e.into(), None))?;
            Ok(RunResult::Dump(wesl))
        }
    }
}

#[wasm_bindgen]
pub fn run(
    #[wasm_bindgen(unchecked_param_type = "Command")] args: JsValue,
) -> Result<JsValue, JsValue> {
    init_log("debug");

    let args = serde_wasm_bindgen::from_value(args).expect("error parsing input");
    log::debug!("[WESL] run with args {args:?}");

    let serializer = serde_wasm_bindgen::Serializer::new()
        .serialize_bytes_as_arrays(false)
        .serialize_large_number_types_as_bigints(true);

    let naga = matches!(args, Command::Compile(CompileOptions { naga: true, .. }));

    match run_impl(args) {
        Ok(res) => match res {
            RunResult::Compile(wgsl) => {
                let source = wgsl.to_string();
                if naga {
                    #[cfg(feature = "naga")]
                    run_naga(&source).map_err(|e| e.serialize(&serializer).unwrap())?;
                }
                Ok(source.into())
            }
            RunResult::Dump(wgsl) => Ok(wgsl.serialize(&serializer).unwrap()),
            RunResult::Eval(inst) => Ok(inst.to_string().into()),
            RunResult::Exec(resources) => Ok(resources.serialize(&serializer).unwrap()),
        },
        Err(e) => Err(e.serialize(&serializer).unwrap()),
    }
}
