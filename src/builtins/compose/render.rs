use std::time::{Duration, Instant};

use super::model::{
    CompiledInterpolation, CompiledSegment, CompiledTemplate, ComposeArtifact, ComposeError,
    ComposeResult, FailureCase, NormalizedExpression, RenderOptions,
};
use super::modifiers::{TransformResult, apply_global_limits, select_and_transform};
use super::sources::{SourceCache, resolve_source};

pub struct Rendered {
    pub text: String,
    pub truncated: bool,
    pub artifacts: Vec<ComposeArtifact>,
}

#[derive(Debug, Clone, Copy)]
pub struct RenderDeadline {
    deadline: Instant,
}

impl RenderDeadline {
    fn from_now(seconds: u64) -> Self {
        Self {
            deadline: Instant::now() + Duration::from_secs(seconds),
        }
    }

    pub fn remaining(self) -> Option<Duration> {
        self.deadline.checked_duration_since(Instant::now())
    }

    fn ensure_not_expired(self) -> ComposeResult<()> {
        if self.remaining().is_some() {
            Ok(())
        } else {
            Err(ComposeError::new(
                "total_timeout",
                Some(FailureCase::Timeout),
                "Render total timeout expired",
            ))
        }
    }
}

// Render is the first phase allowed to evaluate sources. It walks the compiled
// program, evals interpolations, and joins their text with literal segments.
pub fn render_program(
    program: &CompiledTemplate,
    options: &RenderOptions,
) -> ComposeResult<Rendered> {
    let deadline = options.total_timeout_seconds.map(RenderDeadline::from_now);
    let mut cache = SourceCache::new(options.max_spill_bytes);
    let mut out = String::new();
    let mut any_truncated = false;
    let mut artifacts = Vec::new();

    for segment in &program.segments {
        match segment {
            CompiledSegment::Literal(text) => out.push_str(text),
            CompiledSegment::Interpolation(interpolation) => {
                if let Some(deadline) = deadline {
                    deadline
                        .ensure_not_expired()
                        .map_err(|err| err.with_compiled_interpolation(interpolation))?;
                }

                let rendered = eval_interpolation(interpolation, options, &mut cache, deadline)?;
                if let Some(deadline) = deadline {
                    deadline.ensure_not_expired().map_err(|err| {
                        err.with_artifacts(rendered.artifacts.clone())
                            .with_compiled_interpolation(interpolation)
                    })?;
                }
                any_truncated |= rendered.truncated;
                artifacts.extend(rendered.artifacts);
                out.push_str(&rendered.text);
            }
        }
    }

    Ok(Rendered {
        text: out,
        truncated: any_truncated,
        artifacts,
    })
}

// Fallbacks recover source/transform failures for this interpolation only. The
// recovered literal is not fed back through remaining transforms.
fn eval_interpolation(
    interpolation: &CompiledInterpolation,
    options: &RenderOptions,
    cache: &mut SourceCache,
    deadline: Option<RenderDeadline>,
) -> ComposeResult<TransformResult> {
    eval_expression(interpolation, options, cache, deadline)
        .or_else(|err| {
            let artifacts = err
                .artifacts
                .clone()
                .map(|artifacts| artifacts.into_vec())
                .unwrap_or_default();
            interpolation
                .expression
                .policies
                .recover(&err)
                .map(|text| TransformResult {
                    text,
                    truncated: false,
                    artifacts,
                    selected_artifact: None,
                })
                .ok_or(err)
        })
        .map_err(|err| err.with_compiled_interpolation(interpolation))
}

fn eval_expression(
    interpolation: &CompiledInterpolation,
    options: &RenderOptions,
    cache: &mut SourceCache,
    deadline: Option<RenderDeadline>,
) -> ComposeResult<TransformResult> {
    let expression: &NormalizedExpression = &interpolation.expression;
    let source = resolve_source(
        &expression.source,
        options,
        cache,
        expression.timeout,
        deadline,
        interpolation.source_index,
    )?;
    if let Some(deadline) = deadline {
        deadline.ensure_not_expired()?;
    }
    let transformed = select_and_transform(source, expression, options.fail_on_truncated)?;
    let mut result = apply_global_limits(
        transformed,
        options.max_lines,
        options.max_bytes,
        options.fail_on_truncated,
    )?;
    if let Some(deadline) = deadline {
        deadline.ensure_not_expired()?;
    }
    append_selected_artifact_marker(&mut result);
    Ok(result)
}

fn append_selected_artifact_marker(result: &mut TransformResult) {
    if let Some(artifact) = &result.selected_artifact {
        result.text.push('\n');
        result
            .text
            .push_str(&format!("[asq compose: {}]\n", artifact.message));
    }
}
