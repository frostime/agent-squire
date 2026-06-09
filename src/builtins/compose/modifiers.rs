use super::model::{
    ComposeError, ComposeResult, FailureCase, NormalizedExpression, RangePoint, RangeSpec,
    StreamSelector, Transform,
};
use super::sources::ResolvedSource;
use super::text;

#[derive(Debug, Clone)]
pub struct TransformResult {
    pub text: String,
    pub truncated: bool,
}

pub fn select_and_transform(
    source: ResolvedSource,
    expression: &NormalizedExpression,
    fail_on_truncated: bool,
) -> ComposeResult<TransformResult> {
    let mut text = match source {
        ResolvedSource::Text(text) => {
            if expression.stream.is_some() {
                return Err(ComposeError::new(
                    "invalid_modifier",
                    Some(FailureCase::Modifier),
                    "stdout/stderr selectors only apply to exec: sources",
                ));
            }
            text
        }
        ResolvedSource::Exec { stdout, stderr } => {
            match expression.stream.unwrap_or(StreamSelector::Stdout) {
                StreamSelector::Stdout => stdout,
                StreamSelector::Stderr => stderr,
            }
        }
    };

    let mut truncated = false;
    for transform in &expression.transforms {
        let result = apply_transform(&text, transform, fail_on_truncated)?;
        text = result.text;
        truncated |= result.truncated;
    }
    Ok(TransformResult { text, truncated })
}

pub fn apply_global_limits(
    mut result: TransformResult,
    max_lines: Option<usize>,
    max_bytes: Option<usize>,
    fail_on_truncated: bool,
) -> ComposeResult<TransformResult> {
    if let Some(max) = max_lines {
        let (text, truncated) = text::limit_lines(&result.text, max, fail_on_truncated)?;
        result.text = text;
        result.truncated |= truncated;
    }
    if let Some(max) = max_bytes {
        let (text, truncated) = text::limit_bytes(&result.text, max, fail_on_truncated)?;
        result.text = text;
        result.truncated |= truncated;
    }
    Ok(result)
}

fn apply_transform(
    input: &str,
    transform: &Transform,
    fail_on_truncated: bool,
) -> ComposeResult<TransformResult> {
    let (text, truncated) = match transform {
        Transform::Lines(range) => (
            text::select_lines(input, start_line(range, input), end_line(range, input))?,
            false,
        ),
        Transform::Slice(range) => (
            text::select_chars(input, start_char(range, input), end_char(range, input))?,
            false,
        ),
        Transform::Head(count) => (text::head_lines(input, *count), false),
        Transform::HeadChar(count) => (text::head_chars(input, *count), false),
        Transform::Tail(count) => (text::tail_lines(input, *count), false),
        Transform::TailChar(count) => (text::tail_chars(input, *count), false),
        Transform::Trim => (input.trim().to_string(), false),
        Transform::Oneline => (text::oneline(input), false),
        Transform::Indent(spaces) => (text::indent(input, *spaces), false),
        Transform::MaxLines(max) => text::limit_lines(input, *max, fail_on_truncated)?,
        Transform::MaxBytes(max) => text::limit_bytes(input, *max, fail_on_truncated)?,
    };
    Ok(TransformResult { text, truncated })
}

fn start_line(range: &RangeSpec, text: &str) -> usize {
    resolve_line_point(&range.start, text)
}

fn end_line(range: &RangeSpec, text: &str) -> usize {
    resolve_line_point(&range.end, text)
}

fn start_char(range: &RangeSpec, text: &str) -> usize {
    resolve_char_point(&range.start, text)
}

fn end_char(range: &RangeSpec, text: &str) -> usize {
    resolve_char_point(&range.end, text)
}

fn resolve_line_point(point: &RangePoint, text: &str) -> usize {
    match point {
        RangePoint::Beg => 1,
        RangePoint::End => text::split_line_segments(text).len().max(1),
        RangePoint::Number(value) => *value,
    }
}

fn resolve_char_point(point: &RangePoint, text: &str) -> usize {
    match point {
        RangePoint::Beg => 1,
        RangePoint::End => text.chars().count().max(1),
        RangePoint::Number(value) => *value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::compose::compile::compile_template;
    use crate::builtins::compose::parser::parse_template;

    #[test]
    fn transforms_text_left_to_right() {
        let ast = parse_template("${{file: x |> tail: 2 |> trim |> indent: 2}}").unwrap();
        let program = compile_template(&ast).unwrap();
        let expression = match &program.segments[0] {
            super::super::model::CompiledSegment::Interpolation(interpolation) => {
                &interpolation.expression
            }
            super::super::model::CompiledSegment::Literal(_) => panic!("expected interpolation"),
        };
        let result =
            select_and_transform(ResolvedSource::Text("a\nb\nc\n".into()), expression, false)
                .unwrap();
        assert_eq!(result.text, "  b\n  c");
    }
}
