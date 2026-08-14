use std::collections::HashMap;

use wgsl_parse::syntax::{self, TranslationUnit};
use wgsl_types::{Instance, ShaderStage, inst::RefInstance};

use crate::{
    CompileResult,
    error::{Diagnostic, Error},
    eval::{Context, Eval, EvalError, Exec, Inputs, SyntaxUtil, exec_entrypoint},
};

/// The result of [`CompileResult::exec`].
///
/// This type contains both the return value of the function called (if any) and the
/// evaluation context (including bindings).
///
/// This type implements `Display`, call `to_string()` to get the function return value.
pub struct ExecResult<'a> {
    /// The executed function return value
    pub inst: Option<Instance>,
    /// Context after execution
    pub ctx: Context<'a>,
}

impl ExecResult<'_> {
    /// Get the function return value.
    pub fn return_value(&self) -> Option<&Instance> {
        self.inst.as_ref()
    }

    /// Get a [shader resource](https://www.w3.org/TR/WGSL/#resource).
    ///
    /// Shader resources (aka. bindings) with `write`
    /// [access mode](https://www.w3.org/TR/WGSL/#memory-access-mode) can be modified
    /// after executing an entry point.
    pub fn resource(&self, group: u32, binding: u32) -> Option<&RefInstance> {
        self.ctx.resource(group, binding)
    }
}

/// The result of [`CompileResult::eval`].
///
/// This type contains both the resulting WGSL instance and the evaluation context
/// (including bindings).
///
/// This type implements `Display`, call `to_string()` to get the evaluation result.
pub struct EvalResult<'a> {
    /// The expression evaluation result
    pub inst: Instance,
    /// Context after evaluation
    pub ctx: Context<'a>,
}

impl EvalResult<'_> {
    // TODO: make context non-mut
    /// Convert the result instance to its in-memory representation.
    pub fn to_buffer(&mut self) -> Option<Vec<u8>> {
        self.inst.to_buffer()
    }
}

impl std::fmt::Display for EvalResult<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inst.fmt(f)
    }
}

impl CompileResult {
    /// Evaluate a const-expression in the context of this compilation result (experimental).
    ///
    /// Not all builtin WGSL functions are supported yet.
    /// Contrary to [`eval_str`], the provided expression can reference declarations
    /// in the compiled WGSL: global const-declarations and user-defined functions with
    /// the `@const` attribute.
    ///
    /// # WESL Reference
    ///
    /// The user-defined `@const` attribute is non-standard.
    /// See issue [#46](https://github.com/wgsl-tooling-wg/wesl-spec/issues/46#issuecomment-2389531479).
    pub fn eval<'a>(&'a self, source: &str) -> Result<EvalResult<'a>, Error> {
        let expr = source
            .parse::<syntax::Expression>()
            .map_err(|e| Error::Error(Diagnostic::from(e).with_source(source.to_string())))?;
        let (inst, ctx) = eval(&expr, &self.syntax);
        let inst = inst.map_err(|e| {
            Diagnostic::from(e)
                .with_source(source.to_string())
                .with_ctx(&ctx)
        });

        let inst = if let Some(sourcemap) = &self.sourcemap {
            inst.map_err(|e| Error::Error(e.with_sourcemap(sourcemap)))
        } else {
            inst.map_err(Error::Error)
        }?;

        let res = EvalResult { inst, ctx };
        Ok(res)
    }

    /// Execute an entrypoint in the same way that it would be executed on the GPU (experimental).
    ///
    /// Experimental.
    ///
    /// # WESL Reference
    ///
    /// The `@const` attribute is non-standard.
    pub fn exec<'a>(
        &'a self,
        entrypoint: &str,
        inputs: Inputs,
        bindings: HashMap<(u32, u32), RefInstance>,
        overrides: HashMap<String, Instance>,
    ) -> Result<ExecResult<'a>, Error> {
        let mut ctx = Context::new(&self.syntax);
        ctx.add_bindings(bindings);
        ctx.add_overrides(overrides);
        ctx.set_stage(ShaderStage::Exec);

        let entry_fn = SyntaxUtil::decl_function(ctx.source, entrypoint)
            .ok_or_else(|| EvalError::UnknownFunction(entrypoint.to_string()))?;

        let _ = self.syntax.exec(&mut ctx)?;

        let inst = exec_entrypoint(entry_fn, inputs, &mut ctx).map_err(|e| {
            if let Some(sourcemap) = &self.sourcemap {
                Diagnostic::from(e).with_ctx(&ctx).with_sourcemap(sourcemap)
            } else {
                Diagnostic::from(e).with_ctx(&ctx)
            }
        })?;

        Ok(ExecResult { inst, ctx })
    }
}

/// Evaluate a const-expression from a string (experimental).
///
/// Only builtin function declarations marked `@const` can be called from
/// const-expressions.
///
/// Not all builtin `@const` WGSL functions are supported yet.
pub fn eval_str(expr: &str) -> Result<Instance, Error> {
    let expr = expr
        .parse::<syntax::Expression>()
        .map_err(|e| Error::Error(Diagnostic::from(e).with_source(expr.to_string())))?;
    let module = TranslationUnit::default();
    let (inst, ctx) = eval(&expr, &module);
    inst.map_err(|e| {
        Error::Error(
            Diagnostic::from(e)
                .with_source(expr.to_string())
                .with_ctx(&ctx),
        )
    })
}

/// Evaluate a const-expression (experimental).
///
/// Only builtin function declarations marked `@const` can be called from
/// const-expressions.
///
/// Not all builtin `@const` WGSL functions are supported yet.
pub fn eval<'s>(
    expr: &syntax::Expression,
    wgsl: &'s TranslationUnit,
) -> (Result<Instance, EvalError>, Context<'s>) {
    let mut ctx = Context::new(wgsl);
    let res = wgsl.exec(&mut ctx).and_then(|_| expr.eval(&mut ctx));
    (res, ctx)
}

/// Execute a shader on the CPU (experimental).
pub fn exec<'s>(
    expr: &impl Eval,
    wgsl: &'s TranslationUnit,
    bindings: HashMap<(u32, u32), RefInstance>,
    overrides: HashMap<String, Instance>,
) -> (Result<Option<Instance>, EvalError>, Context<'s>) {
    let mut ctx = Context::new(wgsl);
    ctx.add_bindings(bindings);
    ctx.add_overrides(overrides);
    ctx.set_stage(ShaderStage::Exec);

    let res = wgsl.exec(&mut ctx).and_then(|_| match expr.eval(&mut ctx) {
        Ok(ret) => Ok(Some(ret)),
        Err(EvalError::Void(_)) => Ok(None),
        Err(e) => Err(e),
    });
    (res, ctx)
}
