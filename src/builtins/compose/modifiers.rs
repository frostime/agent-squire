use std::collections::BTreeMap;

use super::model::{CommandNode, ComposeError, ComposeResult, FailureCase};
use super::sources::{ResolvedSource, SourceSpec};
use super::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSelector {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub enum Transform {
    Lines(RangeSpec),
    Slice(RangeSpec),
    Head(usize),
    HeadChar(usize),
    Tail(usize),
    TailChar(usize),
    Trim,
    Oneline,
    Indent(usize),
    MaxLines(usize),
    MaxBytes(usize),
}

#[derive(Debug, Clone)]
pub struct RangeSpec {
    start: RangePoint,
    end: RangePoint,
}

#[derive(Debug, Clone)]
enum RangePoint {
    Beg,
    End,
    Number(usize),
}

#[derive(Debug, Clone, Default)]
pub struct FailurePolicies {
    fallback: Option<String>,
    cases: BTreeMap<&'static str, String>,
}

impl FailurePolicies {
    pub fn recover(&self, error: &ComposeError) -> Option<String> {
        let case = error.case?;
        self.cases
            .get(case.as_str())
            .cloned()
            .or_else(|| self.fallback.clone())
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedExpression {
    pub source: SourceSpec,
    pub timeout: Option<u64>,
    pub stream: Option<StreamSelector>,
    pub transforms: Vec<Transform>,
    pub policies: FailurePolicies,
}

#[derive(Debug, Clone)]
pub struct TransformResult {
    pub text: String,
    pub truncated: bool,
}

pub fn normalize(commands: &[CommandNode]) -> ComposeResult<NormalizedExpression> {
    let source = parse_source(&commands[0])?;
    let mut timeout = None;
    let mut stream = None;
    let mut transforms = Vec::new();
    let mut policies = FailurePolicies::default();

    for command in &commands[1..] {
        match command.name.as_str() {
            "timeout" => {
                if timeout.is_some() {
                    return Err(ComposeError::parse(
                        "duplicate_runtime_modifier",
                        "Duplicate timeout: modifier",
                    ));
                }
                let seconds = parse_positive_u64(command, "timeout")?;
                timeout = Some(seconds);
            }
            "stdout" | "stderr" => {
                let next = if command.name == "stdout" {
                    StreamSelector::Stdout
                } else {
                    StreamSelector::Stderr
                };
                if stream.is_some_and(|existing| existing != next) {
                    return Err(ComposeError::parse(
                        "conflicting_stream_selectors",
                        "stdout and stderr selectors are mutually exclusive",
                    ));
                }
                stream = Some(next);
            }
            "fallback" => {
                if policies.fallback.is_some() {
                    return Err(ComposeError::parse(
                        "duplicate_failure_policy",
                        "Duplicate fallback policy",
                    ));
                }
                policies.fallback = Some(body_value(command)?.to_string());
            }
            name if FailureCase::from_policy_name(name).is_some() => {
                let case = FailureCase::from_policy_name(name).expect("checked");
                if policies.cases.contains_key(case.as_str()) {
                    return Err(ComposeError::parse(
                        "duplicate_failure_policy",
                        format!("Duplicate failure policy: {name}"),
                    ));
                }
                policies
                    .cases
                    .insert(case.as_str(), body_value(command)?.to_string());
            }
            _ => transforms.push(parse_transform(command)?),
        }
    }

    Ok(NormalizedExpression {
        source,
        timeout,
        stream,
        transforms,
        policies,
    })
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
            text::select_lines(input, range.start_line(input)?, range.end_line(input)?)?,
            false,
        ),
        Transform::Slice(range) => (
            text::select_chars(input, range.start_char(input)?, range.end_char(input)?)?,
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

fn parse_source(command: &CommandNode) -> ComposeResult<SourceSpec> {
    Ok(match command.name.as_str() {
        "stdin" => SourceSpec::Stdin,
        "file" => SourceSpec::File(body_value(command)?.to_string()),
        "env" => SourceSpec::Env(body_value(command)?.to_string()),
        "exec" => SourceSpec::Exec(body_value(command)?.to_string()),
        _ => {
            return Err(ComposeError::parse(
                "first_command_must_be_source",
                "First command in an interpolation must be a source",
            ));
        }
    })
}

fn parse_transform(command: &CommandNode) -> ComposeResult<Transform> {
    Ok(match command.name.as_str() {
        "lines" => Transform::Lines(parse_range(body_value(command)?)?),
        "slice" => Transform::Slice(parse_range(body_value(command)?)?),
        "head" => Transform::Head(parse_usize(command, "head")?),
        "head-char" => Transform::HeadChar(parse_usize(command, "head-char")?),
        "tail" => Transform::Tail(parse_usize(command, "tail")?),
        "tail-char" => Transform::TailChar(parse_usize(command, "tail-char")?),
        "trim" => Transform::Trim,
        "oneline" => Transform::Oneline,
        "indent" => Transform::Indent(parse_usize(command, "indent")?),
        "max-lines" => Transform::MaxLines(parse_usize(command, "max-lines")?),
        "max-bytes" => Transform::MaxBytes(parse_usize(command, "max-bytes")?),
        _ => {
            return Err(ComposeError::parse(
                "unknown_command",
                format!("Unknown stage command: {}", command.name),
            ));
        }
    })
}

fn parse_range(raw: &str) -> ComposeResult<RangeSpec> {
    let Some((start, end)) = raw.split_once('-') else {
        return Err(ComposeError::new(
            "invalid_range",
            Some(FailureCase::Range),
            format!("Invalid range: {raw}"),
        ));
    };
    Ok(RangeSpec {
        start: parse_point(start.trim())?,
        end: parse_point(end.trim())?,
    })
}

fn parse_point(raw: &str) -> ComposeResult<RangePoint> {
    match raw {
        "BEG" | "beg" => Ok(RangePoint::Beg),
        "END" | "end" => Ok(RangePoint::End),
        _ => {
            let value = raw.parse::<usize>().map_err(|_| {
                ComposeError::new(
                    "invalid_range",
                    Some(FailureCase::Range),
                    format!("Invalid range point: {raw}"),
                )
            })?;
            if value == 0 {
                return Err(ComposeError::new(
                    "invalid_range",
                    Some(FailureCase::Range),
                    "Ranges are 1-based",
                ));
            }
            Ok(RangePoint::Number(value))
        }
    }
}

impl RangeSpec {
    fn start_line(&self, text: &str) -> ComposeResult<usize> {
        resolve_line_point(&self.start, text)
    }

    fn end_line(&self, text: &str) -> ComposeResult<usize> {
        resolve_line_point(&self.end, text)
    }

    fn start_char(&self, text: &str) -> ComposeResult<usize> {
        resolve_char_point(&self.start, text)
    }

    fn end_char(&self, text: &str) -> ComposeResult<usize> {
        resolve_char_point(&self.end, text)
    }
}

fn resolve_line_point(point: &RangePoint, text: &str) -> ComposeResult<usize> {
    Ok(match point {
        RangePoint::Beg => 1,
        RangePoint::End => text::split_line_segments(text).len().max(1),
        RangePoint::Number(value) => *value,
    })
}

fn resolve_char_point(point: &RangePoint, text: &str) -> ComposeResult<usize> {
    Ok(match point {
        RangePoint::Beg => 1,
        RangePoint::End => text.chars().count().max(1),
        RangePoint::Number(value) => *value,
    })
}

fn body_value(command: &CommandNode) -> ComposeResult<&str> {
    command
        .body
        .as_ref()
        .map(|body| body.value.as_str())
        .ok_or_else(|| {
            ComposeError::parse(
                "missing_body",
                format!("Command {} requires a body", command.name),
            )
        })
}

fn parse_usize(command: &CommandNode, label: &str) -> ComposeResult<usize> {
    let value = body_value(command)?.parse::<usize>().map_err(|_| {
        ComposeError::new(
            "invalid_modifier",
            Some(FailureCase::Modifier),
            format!("{label}: requires a non-negative integer"),
        )
    })?;
    Ok(value)
}

fn parse_positive_u64(command: &CommandNode, label: &str) -> ComposeResult<u64> {
    let value = body_value(command)?.parse::<u64>().map_err(|_| {
        ComposeError::new(
            "invalid_modifier",
            Some(FailureCase::Modifier),
            format!("{label}: requires a positive integer"),
        )
    })?;
    if value == 0 {
        return Err(ComposeError::new(
            "invalid_modifier",
            Some(FailureCase::Modifier),
            format!("{label}: requires a positive integer"),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::compose::parser::parse_expression;

    #[test]
    fn normalizes_stream_before_transform() {
        let commands = parse_expression("exec: cmd |> head: 2 |> stderr |> timeout: 5").unwrap();
        let expression = normalize(&commands).unwrap();
        assert_eq!(expression.timeout, Some(5));
        assert_eq!(expression.stream, Some(StreamSelector::Stderr));
        assert_eq!(expression.transforms.len(), 1);
    }

    #[test]
    fn fallback_policy_recovers_specific_case_first() {
        let commands = parse_expression("file: missing |> fallback: all |> on-404: none").unwrap();
        let expression = normalize(&commands).unwrap();
        let error = ComposeError::new("source_404", Some(FailureCase::NotFound), "missing");
        assert_eq!(expression.policies.recover(&error).unwrap(), "none");
    }

    #[test]
    fn transforms_text_left_to_right() {
        let commands = parse_expression("file: x |> tail: 2 |> trim |> indent: 2").unwrap();
        let expression = normalize(&commands).unwrap();
        let result =
            select_and_transform(ResolvedSource::Text("a\nb\nc\n".into()), &expression, false)
                .unwrap();
        assert_eq!(result.text, "  b\n  c");
    }
}
