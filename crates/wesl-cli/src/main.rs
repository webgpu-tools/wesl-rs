//! The Command-line interface for `wesl-rs`.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{
    convert::Infallible,
    error::Error,
    fs::{self, File},
    io::{IsTerminal, Read, Write},
    path::PathBuf,
    str::FromStr,
};
use wesl::{
    CompileOptions, CompileResult, Compiler, Feature, Features, ManglerKind,
    error::Diagnostic,
    eval::{Eval, EvalAttrs, Inputs, Instance, LiteralInstance, RefInstance, Ty, ty_eval_ty},
    package::PackageBuilder,
    resolver::{Router, StandardResolver, VirtualResolver},
    syntax::{self, AccessMode, AddressSpace, ModulePath, PathOrigin, TranslationUnit},
};

// adapted from clap cookbook: https://docs.rs/clap/latest/clap/_derive/_cookbook/typed_derive/index.html
fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: FromStr + Default,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s.find('=');
    if let Some(pos) = pos {
        Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
    } else {
        Ok((s.parse()?, U::default()))
    }
}

#[derive(Parser)]
#[command(version, author, about)]
#[command(propagate_version = true)]
struct Cli {
    /// Main command
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Clone, Debug)]
enum Command {
    /// Check correctness of the source file
    Check(CheckArgs),
    /// Parse the source and convert it back to code from the syntax tree
    // Parse(CommonArgs),
    // /// Output the syntax tree to stdout
    // Dump(CommonArgs),
    // /// Compile a source file and outputs the compiled file to stdout
    Compile(CompileArgs),
    /// Evaluate a const-expression
    Eval(EvalArgs),
    /// Execute a WGSL shader function on the CPU
    Exec(ExecArgs),
    /// Generate a publishable Cargo package from WESL source code
    Package(PackageArgs),
}

#[derive(Default, Clone, Copy, Debug, ValueEnum)]
pub enum ClapManglerKind {
    /// Escaped path mangler. `foo/bar/{item} -> foo_bar_item`
    #[default]
    Escape,
    /// Hash mangler. `foo/bar/{item} -> item_1985638328947`
    Hash,
    /// Make valid identifiers with unicode "confusables" characters.
    /// `foo/{bar<baz, moo>} -> foo::barᐸbazˏmooᐳ`
    Unicode,
    /// Disable mangling (warning: will break if case of name conflicts!)
    None,
}

impl From<ClapManglerKind> for ManglerKind {
    fn from(value: ClapManglerKind) -> Self {
        match value {
            ClapManglerKind::Escape => Self::Escape,
            ClapManglerKind::Hash => Self::Hash,
            ClapManglerKind::Unicode => Self::Unicode,
            ClapManglerKind::None => Self::None,
        }
    }
}

#[derive(Default, Clone, Copy, Debug, ValueEnum)]
pub enum ClapFeature {
    #[default]
    #[value(alias("true"))]
    Enable,
    #[value(alias("false"))]
    Disable,
    Keep,
    Error,
}

impl FromStr for ClapFeature {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enable" | "true" => Ok(Self::Enable),
            "disable" | "false" => Ok(Self::Disable),
            "keep" => Ok(Self::Keep),
            "error" => Ok(Self::Error),
            _ => {
                panic!("not a valid feature value, expected `enable`, `disable`, `keep` or `error`")
            }
        }
    }
}

impl From<ClapFeature> for Feature {
    fn from(value: ClapFeature) -> Self {
        match value {
            ClapFeature::Enable => Self::Enable,
            ClapFeature::Disable => Self::Disable,
            ClapFeature::Keep => Self::Keep,
            ClapFeature::Error => Self::Error,
        }
    }
}

#[derive(Args, Clone, Debug)]
struct CompOptsArgs {
    /// Name mangling strategy
    #[arg(long, default_value = "escape")]
    mangler: ClapManglerKind,
    /// Show nicer error messages by computing a sourcemap
    #[arg(long)]
    no_sourcemap: bool,
    /// Disable imports
    #[arg(long)]
    no_imports: bool,
    /// Disable conditional compilation
    #[arg(long)]
    no_cond_comp: bool,
    /// Enable generics
    #[arg(long)]
    generics: bool,
    /// Disable stripping unused declarations
    #[arg(long)]
    no_strip: bool,
    /// Enable lowering output to compatibility-mode WGSL
    #[arg(long)]
    lower: bool,
    /// Disable performing validation checks
    #[arg(long)]
    no_validate: bool,
    /// Enable mangling of declarations in the main module.
    #[arg(long)]
    mangle_main: bool,
    /// Disable performing validation checks with naga
    #[cfg(feature = "naga")]
    #[arg(long)]
    no_naga: bool,
    /// Main module declaration names to keep. Keeps all main module entrypoints by
    /// default. Can be repeated to keep multiple declarations
    #[arg(long)]
    keep: Option<Vec<String>>,
    /// Keep all main module declarations instead of just the entrypoints
    #[arg(long)]
    keep_main: bool,
    /// Set a conditional compilation feature flag. Can be repeated
    #[arg(short='D', long, value_name="NAME | NAME=[enable, disable, keep, error]", value_parser = parse_key_val::<String, ClapFeature>)]
    feature: Vec<(String, ClapFeature)>,
    /// Default behavior for unspecified conditional compilation features
    #[arg(long, default_value = "disable")]
    feature_default: ClapFeature,
    /// Package root directory for `package::` imports. Defaults to the parent directory of the main module.
    #[arg(long)]
    root: Option<PathBuf>,
    /// Literal constants in the `constants` virtual module.
    #[arg(long = "constant", value_name="NAME=LITERAL", value_parser = parse_key_val::<String, String>)]
    constants: Vec<(String, String)>,
}

impl TryFrom<&CompOptsArgs> for CompileOptions {
    type Error = CliError;

    fn try_from(opts: &CompOptsArgs) -> Result<Self, CliError> {
        let flags = opts
            .feature
            .iter()
            .map(|(k, v)| (k.clone(), (*v).into()))
            .collect();

        let constants = opts
            .constants
            .iter()
            .map(
                |(name, expr)| -> Result<(String, LiteralInstance), CliError> {
                    let expr = expr
                        .parse::<syntax::LiteralExpression>()
                        .map_err(|e| Diagnostic::from(e).with_source(expr.to_string()))?;
                    Ok((name.to_string(), LiteralInstance::from(expr)))
                },
            )
            .collect::<Result<_, _>>()?;

        Ok(Self {
            imports: !opts.no_imports,
            condcomp: !opts.no_cond_comp,
            generics: opts.generics,
            strip: !opts.no_strip,
            lower: opts.lower,
            validate: !opts.no_validate,
            sourcemap: !opts.no_sourcemap,
            mangle_main: opts.mangle_main,
            keep: if opts.no_strip {
                None
            } else {
                opts.keep.clone()
            },
            keep_main: opts.keep_main,
            features: Features {
                default: opts.feature_default.into(),
                flags,
            },
            mangler: opts.mangler.into(),
            constants,
            dependencies: Default::default(),
        })
    }
}

#[derive(Args, Clone, Debug)]
struct CompileArgs {
    #[command(flatten)]
    options: CompOptsArgs,
    /// WESL file entry point
    file: Option<PathBuf>,
}

#[derive(Args, Clone, Debug)]
struct CheckArgs {
    /// Input file type (wgsl or wesl)
    #[arg(long, default_value = "wesl")]
    kind: CheckKind,
    /// Validate output using Naga
    #[cfg(feature = "naga")]
    #[arg(long)]
    naga: bool,
    /// WGSL file entry point
    file: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Debug, Default)]
enum CheckKind {
    /// Check that an input file is valid WGSL
    Wgsl,
    /// Check that an input file is valid WESL
    #[default]
    Wesl,
}

/// reference: <https://gpuweb.github.io/gpuweb/#binding-type>
#[derive(Clone, Copy, Debug)]
enum BindingType {
    Uniform,
    Storage,
    ReadOnlyStorage,
    Filtering,
    NonFiltering,
    Comparison,
    Float,
    UnfilterableFloat,
    Sint,
    Uint,
    Depth,
    WriteOnly,
    ReadWrite,
    ReadOnly,
}

impl FromStr for BindingType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "uniform" => Ok(Self::Uniform),
            "storage" => Ok(Self::Storage),
            "read-only-storage" => Ok(Self::ReadOnlyStorage),
            "filtering" => Ok(Self::Filtering),
            "non-filtering" => Ok(Self::NonFiltering),
            "comparison" => Ok(Self::Comparison),
            "float" => Ok(Self::Float),
            "unfilterable-float" => Ok(Self::UnfilterableFloat),
            "sint" => Ok(Self::Sint),
            "uint" => Ok(Self::Uint),
            "depth" => Ok(Self::Depth),
            "write-only" => Ok(Self::WriteOnly),
            "read-write" => Ok(Self::ReadWrite),
            "read-only" => Ok(Self::ReadOnly),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug)]
struct Binding {
    group: u32,
    binding: u32,
    kind: BindingType,
    data: Box<[u8]>,
}

impl FromStr for Binding {
    type Err = Box<dyn Error + Send + Sync + 'static>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(':');
        let binding = (|| {
            Ok(Binding {
                group: it
                    .next()
                    .ok_or("missing @group number")?
                    .parse()
                    .map_err(|e| format!("failed to parse group: {e}"))?,
                binding: it
                    .next()
                    .ok_or("missing @binding number")?
                    .parse()
                    .map_err(|e| format!("failed to parse binding: {e}"))?,
                kind: it
                    .next()
                    .ok_or("missing resource binding type")?
                    .parse()
                    .map_err(|()| "invalid resource binding type".to_string())?,
                data: {
                    let path = PathBuf::from(it.next().ok_or("missing data")?);
                    let mut file = File::open(&path).expect("failed to open binding file");
                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf)
                        .expect("failed to read binding file");
                    buf.into_boxed_slice()
                },
            })
        })();
        binding.map_err(|e: String| format!("failed to parse binding: {e}").into())
    }
}

#[derive(Args, Clone, Debug)]
struct EvalArgs {
    /// Context to evaluate the expression into
    #[command(flatten)]
    options: CompOptsArgs,
    /// Output as binary (WGSL memory representation) for storable types
    #[arg(short, long)]
    binary: bool,
    /// Optional WESL entrypoint module to evaluate the expression into
    #[arg(long)]
    file: Option<PathBuf>,
    /// Const-expression to evaluate
    expr: String,
}

#[derive(Args, Clone, Debug)]
struct ExecArgs {
    /// Context to evaluate the expression into
    #[command(flatten)]
    options: CompOptsArgs,
    /// Bound resources. Only buffer bindings are supported at the moment.
    /// Syntax: colon-separated group,binding,binding_type,path
    /// Example: 0:0:storage:./my_buffer.bin
    ///  - group and binding are @group and @binding numbers
    ///  - binding_type is the `GPUBufferBindingType` (uniform, storage, read-only-storage)
    ///  - path is a path to a binary file of the buffer contents
    #[arg(long = "resource", value_parser = Binding::from_str, verbatim_doc_comment)]
    resources: Vec<Binding>,
    /// Pipeline-overridable constants.
    #[arg(long = "override", value_name="NAME=EXPRESSION", value_parser = parse_key_val::<String, String>)]
    overrides: Vec<(String, String)>,
    /// Output as binary (WGSL memory representation) for storable types
    #[arg(short, long = "out-binary")]
    binary: bool,
    /// Function name to execute
    #[arg(long, default_value = "main")]
    entrypoint: String,
    /// WESL entrypoint module to evaluate the expression into
    file: Option<PathBuf>,
}

#[derive(Args, Clone, Debug)]
struct PackageArgs {
    /// name of the generated crate
    name: String,
    /// directory containing the .wesl shader files
    dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("input file not found")]
    FileNotFound,
    #[error("resource `@group({0}) @binding({1})` not found")]
    ResourceNotFound(u32, u32),
    #[error(
        "resource `@group({0}) @binding({1})` ({2} bytes) is incompatible with type `{3}` ({4} bytes)"
    )]
    ResourceIncompatible(u32, u32, u32, wesl::eval::Type, u32),
    #[error("Could not convert instance to buffer (type `{0}` is not storable)")]
    NotStorable(wesl::eval::Type),
    #[error("{0}")]
    WeslError(#[from] wesl::error::Error),
    #[error("{0}")]
    WeslDiagnostic(#[from] wesl::error::Diagnostic<wesl::error::Error>),
    #[cfg(feature = "naga")]
    #[error("naga parse error: {}", .0.emit_to_string(.1))]
    NagaParse(naga::front::wgsl::ParseError, String),
    #[cfg(feature = "naga")]
    #[error("naga validation error: {}", .0.emit_to_string(.1))]
    NagaValid(Box<naga::WithSpan<naga::valid::ValidationError>>, String),
}

enum FileOrSource {
    File(PathBuf),
    Source(String),
}

fn run_compile(
    options: &CompOptsArgs,
    file_or_source: FileOrSource,
) -> Result<CompileResult, CliError> {
    let compile_options = CompileOptions::try_from(options)?;
    let compiler = Compiler::new(compile_options);

    match file_or_source {
        FileOrSource::File(path) => {
            let base = options
                .root
                .as_deref()
                .or(path.parent())
                .ok_or(CliError::FileNotFound)?;
            let name = path
                .file_stem()
                .ok_or(CliError::FileNotFound)?
                .to_string_lossy()
                .to_string(); // TODO: we should validate that the file name is valid for WESL modules.
            let path = ModulePath::new(PathOrigin::Absolute, vec![name]);
            let resolver = StandardResolver::new(base);

            let res = compiler.with_resolver(resolver).compile_module(&path)?;
            Ok(res)
        }
        FileOrSource::Source(source) => {
            let base = std::env::current_dir().unwrap();
            let name = "stdin";
            let mut router = Router::new();
            let mut resolver = VirtualResolver::new();
            let path = ModulePath::new(PathOrigin::Absolute, vec![name.to_string()]);
            resolver.add_module(ModulePath::new_root(), source.into());
            router.mount_resolver(path.clone(), resolver);
            router.mount_fallback_resolver(StandardResolver::new(base));

            let res = compiler.with_resolver(router).compile_module(&path)?;
            Ok(res)
        }
    }
}

fn parse_binding(
    b: &Binding,
    module: &TranslationUnit,
) -> Result<((u32, u32), RefInstance), CliError> {
    let mut ctx = wesl::eval::Context::new(module);

    let ty_expr = module
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
        Diagnostic::from(e)
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

fn eval_expr(src: &str, module: &TranslationUnit) -> Result<Instance, CliError> {
    let mut ctx = wesl::eval::Context::new(module);
    let expr = src
        .parse::<syntax::Expression>()
        .map_err(|e| Diagnostic::from(e).with_source(src.to_string()))?;
    let inst = expr.eval_value(&mut ctx).map_err(|e| {
        Diagnostic::from(e)
            .with_ctx(&ctx)
            .with_source(src.to_string())
    })?;
    Ok(inst)
}

fn main() {
    let cli = Cli::try_parse()
        .inspect_err(|e| {
            eprintln!("invalid arguments: {e}");
            std::process::exit(1)
        })
        .unwrap();
    run(cli).inspect_err(|e| eprintln!("{e}")).ok();
}

fn file_or_source(path: Option<PathBuf>) -> Option<FileOrSource> {
    path.map(FileOrSource::File).or_else(|| {
        // we don't read from stdin in case the input is a TTY.
        if std::io::stdin().is_terminal() {
            return None;
        }
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .ok()
            .map(|_| FileOrSource::Source(buf))
    })
}

#[cfg(feature = "naga")]
fn naga_validate(source: &str) -> Result<(), CliError> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| CliError::NagaParse(e, source.to_string()))?;
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    validator
        .validate(&module)
        .map_err(|e| CliError::NagaValid(Box::new(e), source.to_string()))?;
    Ok(())
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Check(args) => {
            let source = if let Some(file) = &args.file {
                fs::read_to_string(file).map_err(|_| CliError::FileNotFound)?
            } else {
                let mut source = String::new();
                std::io::stdin()
                    .read_to_string(&mut source)
                    .map_err(|_| CliError::FileNotFound)?;
                source
            };

            match &args.kind {
                CheckKind::Wgsl => {
                    // recognize is a spec-compliant parser, it does not recognize WESL
                    // extensions.
                    wgsl_parse::recognize_str(&source)
                        .map_err(|e| Diagnostic::from(e).with_source(source.clone()))?;
                    let mut module = wgsl_parse::parse_str(&source)
                        .map_err(|e| Diagnostic::from(e).with_source(source.clone()))?;
                    wesl::pass::retarget_idents(&mut module);
                    wesl::pass::validate_wgsl(&module)?;

                    #[cfg(feature = "naga")]
                    if args.naga {
                        naga_validate(&source)?;
                    }
                }
                CheckKind::Wesl => {
                    let mut module = TranslationUnit::from_str(&source)
                        .map_err(|e| Diagnostic::from(e).with_source(source))?;
                    wesl::pass::retarget_idents(&mut module);
                    wesl::pass::validate_wesl(&module)?;
                }
            }
            println!("OK");
        }
        Command::Compile(args) => {
            let comp = file_or_source(args.file)
                .map(|input| run_compile(&args.options, input))
                .unwrap_or_else(|| Ok(CompileResult::default()))?;
            #[cfg(feature = "naga")]
            if !args.options.no_naga {
                naga_validate(&comp.to_string())?;
            }
            println!("{comp}");
        }
        Command::Eval(args) => {
            let comp = file_or_source(args.file)
                .map(|input| run_compile(&args.options, input))
                .unwrap_or_else(|| Ok(CompileResult::default()))?;
            let eval = comp.eval(&args.expr)?;
            if args.binary {
                let buf = eval
                    .inst
                    .to_buffer()
                    .ok_or_else(|| CliError::NotStorable(eval.inst.ty()))?;
                std::io::stdout().write_all(buf.as_slice()).unwrap();
            } else {
                println!("{}", eval.inst)
            }
        }
        Command::Exec(args) => {
            let comp = file_or_source(args.file)
                .map(|input| run_compile(&args.options, input))
                .unwrap_or_else(|| Ok(CompileResult::default()))?;

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
                    Ok((name.to_string(), eval_expr(expr, &comp.syntax)?))
                })
                .collect::<Result<_, _>>()?;

            let exec = comp.exec(&args.entrypoint, inputs, resources, overrides)?;

            if let Some(inst) = &exec.inst {
                if args.binary {
                    let buf = inst
                        .to_buffer()
                        .ok_or_else(|| CliError::NotStorable(inst.ty()))?;
                    std::io::stdout().write_all(buf.as_slice()).unwrap();
                } else {
                    println!("return: {inst}")
                }
            } else if !args.binary {
                println!("return: void")
            }

            let resources = args
                .resources
                .iter()
                .filter_map(|r| {
                    let inst = exec.resource(r.group, r.binding)?.clone();
                    let inst = inst.read().ok()?.to_owned();
                    Some((r.group, r.binding, inst))
                })
                .collect::<Vec<_>>();

            for (group, binding, inst) in resources {
                if args.binary {
                    let buf = inst
                        .to_buffer()
                        .ok_or_else(|| CliError::NotStorable(inst.ty()))?;
                    std::io::stdout().write_all(buf.as_slice()).unwrap();
                } else {
                    println!("resource: group={group} binding={binding} value={inst}")
                }
            }
        }
        Command::Package(args) => {
            let code = PackageBuilder::new(&args.name)
                .scan_root(args.dir)
                .expect("failed to scan WESL files")
                .validate()
                .map_err(|e| {
                    eprintln!("{e}");
                    panic!()
                })
                .unwrap()
                .codegen();
            println!("{code}");
        }
    };
    Ok(())
}
