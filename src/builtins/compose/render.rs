use std::time::Instant;

use super::model::{
    CompiledInterpolation, CompiledSegment, CompiledTemplate, ComposeError, ComposeResult,
    FailureCase, NormalizedExpression, RenderOptions,
};
use super::modifiers::{TransformResult, apply_global_limits, select_and_transform};
use super::sources::{SourceCache, resolve_source};

pub struct Rendered {
    pub text: String,
    pub truncated: bool,
}

pub fn render_program(
    program: &CompiledTemplate,
    options: &RenderOptions,
) -> ComposeResult<Rendered> {
    let started = Instant::now();
    let mut cache = SourceCache::default();
    let mut out = String::new();
    let mut any_truncated = false;

    for segment in &program.segments {
        match segment {
            CompiledSegment::Literal(text) => out.push_str(text),
            CompiledSegment::Interpolation(interpolation) => {
                if let Some(total_timeout) = options.total_timeout_seconds
                    && started.elapsed().as_secs() > total_timeout
                {
                    return Err(ComposeError::new(
                        "total_timeout",
                        Some(FailureCase::Timeout),
                        format!("Render timed out after {total_timeout} seconds"),
                    )
                    .with_compiled_interpolation(interpolation));
                }

                let rendered = eval_interpolation(interpolation, options, &mut cache)?;
                any_truncated |= rendered.truncated;
                out.push_str(&rendered.text);
            }
        }
    }

    Ok(Rendered {
        text: out,
        truncated: any_truncated,
    })
}

fn eval_interpolation(
    interpolation: &CompiledInterpolation,
    options: &RenderOptions,
    cache: &mut SourceCache,
) -> ComposeResult<TransformResult> {
    eval_expression(&interpolation.expression, options, cache)
        .or_else(|err| {
            interpolation
                .expression
                .policies
                .recover(&err)
                .map(|text| TransformResult {
                    text,
                    truncated: false,
                })
                .ok_or(err)
        })
        .map_err(|err| err.with_compiled_interpolation(interpolation))
}

fn eval_expression(
    expression: &NormalizedExpression,
    options: &RenderOptions,
    cache: &mut SourceCache,
) -> ComposeResult<TransformResult> {
    let source = resolve_source(&expression.source, options, cache, expression.timeout)?;
    let transformed = select_and_transform(source, expression, options.fail_on_truncated)?;
    apply_global_limits(
        transformed,
        options.max_lines,
        options.max_bytes,
        options.fail_on_truncated,
    )
}
