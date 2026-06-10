use super::model::{
    CommandNode, CompiledInterpolation, CompiledSegment, CompiledTemplate, ComposeError,
    ComposeResult, FailureCase, FailurePolicies, Interpolation, RangePoint, RangeSpec, Segment,
    SourceInfo, SourceSpec, StreamSelector, Template, Transform,
};

// Compile performs static semantic analysis only: command role validation,
// normalization, and source listing. It must never read stdin/files/env or exec.
pub fn compile_template(template: &Template) -> ComposeResult<CompiledTemplate> {
    let mut segments = Vec::new();
    let mut sources = Vec::new();

    for segment in &template.segments {
        match segment {
            Segment::Literal(text) => segments.push(CompiledSegment::Literal(text.clone())),
            Segment::Interpolation(interpolation) => {
                let source_index = sources.len() + 1;
                let compiled = compile_interpolation(interpolation, source_index)?;
                sources.push(source_info(
                    source_index,
                    interpolation,
                    &compiled.expression,
                ));
                segments.push(CompiledSegment::Interpolation(compiled));
            }
        }
    }

    Ok(CompiledTemplate { segments, sources })
}

fn compile_interpolation(
    interpolation: &Interpolation,
    source_index: usize,
) -> ComposeResult<CompiledInterpolation> {
    validate_command_roles(&interpolation.commands)
        .map_err(|err| err.with_interpolation(interpolation))?;
    let expression =
        normalize(&interpolation.commands).map_err(|err| err.with_interpolation(interpolation))?;
    Ok(CompiledInterpolation {
        raw: interpolation.raw.clone(),
        location: interpolation.location.clone(),
        source_index,
        expression,
    })
}

fn source_info(
    index: usize,
    interpolation: &Interpolation,
    expression: &super::model::NormalizedExpression,
) -> SourceInfo {
    let (kind, argument) = match &expression.source {
        SourceSpec::Stdin => ("stdin", "".to_string()),
        SourceSpec::File(path) => ("file", path.clone()),
        SourceSpec::Env(name) => ("env", name.clone()),
        SourceSpec::Exec(command) => ("exec", command.clone()),
    };
    SourceInfo {
        index,
        kind: kind.to_string(),
        argument,
        location: interpolation.location.clone(),
    }
}

fn validate_command_roles(commands: &[CommandNode]) -> ComposeResult<()> {
    for (index, command) in commands.iter().enumerate() {
        let is_source = matches!(command.name.as_str(), "stdin" | "file" | "env" | "exec");
        if index == 0 && !is_source {
            return Err(ComposeError::parse(
                "first_command_must_be_source",
                "First command in an interpolation must be a source",
            ));
        }
        if index > 0 && is_source {
            return Err(ComposeError::parse(
                "source_after_first",
                format!("Source command {} may only appear first", command.name),
            ));
        }
        validate_known_command(command)?;
    }
    Ok(())
}

fn validate_known_command(command: &CommandNode) -> ComposeResult<()> {
    let name = command.name.as_str();
    let known = matches!(
        name,
        "stdin"
            | "file"
            | "env"
            | "exec"
            | "timeout"
            | "stdout"
            | "stderr"
            | "lines"
            | "slice"
            | "head"
            | "head-char"
            | "tail"
            | "tail-char"
            | "trim"
            | "oneline"
            | "indent"
            | "max-lines"
            | "max-bytes"
            | "fallback"
            | "on-404"
            | "on-error"
            | "on-timeout"
            | "on-range"
            | "on-binary"
            | "on-encoding"
            | "on-limit"
            | "on-modifier"
    );
    if !known {
        return Err(ComposeError::parse(
            "unknown_command",
            format!("Unknown command: {name}"),
        ));
    }

    let no_arg = matches!(name, "stdin" | "stdout" | "stderr" | "trim" | "oneline");
    if no_arg {
        if let Some(body) = &command.body
            && !body.value.is_empty()
        {
            return Err(ComposeError::parse(
                "unexpected_body",
                format!("Command {name} does not accept a body"),
            ));
        }
        return Ok(());
    }

    let Some(body) = &command.body else {
        return Err(ComposeError::parse(
            "missing_body",
            format!("Command {name} requires a body"),
        ));
    };
    if body.value.is_empty() && !body.quoted {
        return Err(ComposeError::parse(
            "missing_body",
            format!("Command {name} requires a body"),
        ));
    }
    Ok(())
}

// Normalize keeps authoring order only for text transforms. Controls, stream
// selectors, and failure policies are classified here so render has no ambiguity.
fn normalize(commands: &[CommandNode]) -> ComposeResult<super::model::NormalizedExpression> {
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
                timeout = Some(parse_positive_u64(command, "timeout")?);
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

    Ok(super::model::NormalizedExpression {
        source,
        timeout,
        stream,
        transforms,
        policies,
    })
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
    body_value(command)?.parse::<usize>().map_err(|_| {
        ComposeError::new(
            "invalid_modifier",
            Some(FailureCase::Modifier),
            format!("{label}: requires a non-negative integer"),
        )
    })
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
    use crate::builtins::compose::parser::parse_template;

    #[test]
    fn check_phase_catches_static_conflicts_without_eval() {
        let ast = parse_template("${{exec: definitely-not-real |> stdout |> stderr}}").unwrap();
        let err = compile_template(&ast).unwrap_err();
        assert_eq!(err.code, "conflicting_stream_selectors");
    }

    #[test]
    fn compiles_source_list_from_program() {
        let ast = parse_template("${{file: README.md}}\n${{stdin}}").unwrap();
        let program = compile_template(&ast).unwrap();
        assert_eq!(program.sources.len(), 2);
        assert_eq!(program.sources[0].kind, "file");
        assert_eq!(program.sources[1].kind, "stdin");
    }
}
