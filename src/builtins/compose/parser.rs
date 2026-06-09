use super::model::{
    CommandBody, CommandNode, ComposeError, ComposeResult, Interpolation, Location, Segment,
    Template,
};

pub fn parse_template(input: &str) -> ComposeResult<Template> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut index = 0;
    let mut line = 1;
    let mut column = 1;

    while index < input.len() {
        if input[index..].starts_with("\\${{") {
            literal.push_str("${{");
            advance(&input[index..index + 4], &mut line, &mut column);
            index += 4;
            continue;
        }
        if input[index..].starts_with("\\}}") {
            literal.push_str("}}");
            advance(&input[index..index + 3], &mut line, &mut column);
            index += 3;
            continue;
        }
        if input[index..].starts_with("${{") {
            if !literal.is_empty() {
                segments.push(Segment::Literal(std::mem::take(&mut literal)));
            }
            let start_line = line;
            let start_column = column;
            let close = find_interpolation_close(input, index + 3).ok_or_else(|| {
                ComposeError::parse("unclosed_interpolation", "Unclosed interpolation block")
            })?;
            let raw = input[index..close + 2].to_string();
            let body = &input[index + 3..close];
            let commands = parse_expression(body)?;
            segments.push(Segment::Interpolation(Interpolation {
                raw,
                location: Location {
                    line: start_line,
                    column: start_column,
                },
                commands,
            }));
            advance(&input[index..close + 2], &mut line, &mut column);
            index = close + 2;
            continue;
        }

        let ch = input[index..]
            .chars()
            .next()
            .expect("index on char boundary");
        literal.push(ch);
        advance_char(ch, &mut line, &mut column);
        index += ch.len_utf8();
    }

    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }

    Ok(Template { segments })
}

pub fn parse_expression(input: &str) -> ComposeResult<Vec<CommandNode>> {
    let parts = split_pipeline(input)?;
    if parts.is_empty() {
        return Err(ComposeError::parse(
            "empty_expression",
            "Empty interpolation expression",
        ));
    }
    let commands = parts
        .into_iter()
        .map(parse_command)
        .collect::<ComposeResult<Vec<_>>>()?;
    validate_command_roles(&commands)?;
    Ok(commands)
}

fn parse_command(input: &str) -> ComposeResult<CommandNode> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(ComposeError::parse(
            "empty_command",
            "Empty command in interpolation",
        ));
    }

    let Some(colon) = raw.find(':') else {
        validate_name(raw)?;
        return Ok(CommandNode {
            name: raw.to_string(),
            body: None,
        });
    };

    let name = raw[..colon].trim();
    validate_name(name)?;
    let body_raw = raw[colon + 1..].trim();
    let body = if body_raw.is_empty() {
        Some(CommandBody {
            value: String::new(),
            quoted: false,
        })
    } else if body_raw.starts_with('"') {
        let value = serde_json::from_str::<String>(body_raw).map_err(|_| {
            ComposeError::parse(
                "invalid_quoted_body",
                format!("Invalid JSON string body for command {name}"),
            )
        })?;
        Some(CommandBody {
            value,
            quoted: true,
        })
    } else {
        if body_raw.contains("${{") {
            return Err(ComposeError::parse(
                "nested_interpolation_not_supported",
                "Nested interpolation is not supported",
            ));
        }
        if body_raw.contains('\n') || body_raw.contains('\r') {
            return Err(ComposeError::parse(
                "multiline_unquoted_body",
                format!("Command {name} has a multiline unquoted body"),
            ));
        }
        Some(CommandBody {
            value: body_raw.to_string(),
            quoted: false,
        })
    };

    Ok(CommandNode {
        name: name.to_string(),
        body,
    })
}

fn split_pipeline(input: &str) -> ComposeResult<Vec<&str>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut parts = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;

    while index < trimmed.len() {
        let ch = trimmed[index..]
            .chars()
            .next()
            .expect("index on char boundary");
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += ch.len_utf8();
            continue;
        }

        if ch == '"' {
            in_string = true;
            index += ch.len_utf8();
            continue;
        }

        if trimmed[index..].starts_with("|>") {
            let part = trimmed[start..index].trim();
            if part.is_empty() {
                return Err(ComposeError::parse(
                    "empty_command",
                    "Empty command in pipeline",
                ));
            }
            parts.push(part);
            index += 2;
            start = index;
            continue;
        }

        index += ch.len_utf8();
    }

    if in_string {
        return Err(ComposeError::parse(
            "invalid_quoted_body",
            "Unclosed JSON string body",
        ));
    }

    let part = trimmed[start..].trim();
    if part.is_empty() {
        return Err(ComposeError::parse(
            "empty_command",
            "Trailing pipeline separator",
        ));
    }
    parts.push(part);
    Ok(parts)
}

fn find_interpolation_close(input: &str, start: usize) -> Option<usize> {
    let mut index = start;
    let mut in_string = false;
    let mut escaped = false;

    while index < input.len() {
        let ch = input[index..].chars().next()?;
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += ch.len_utf8();
            continue;
        }

        if ch == '"' {
            in_string = true;
            index += ch.len_utf8();
            continue;
        }

        if input[index..].starts_with("${{") {
            return None;
        }
        if input[index..].starts_with("\\}}") {
            index += 3;
            continue;
        }
        if input[index..].starts_with("}}") {
            return Some(index);
        }
        index += ch.len_utf8();
    }
    None
}

fn validate_name(name: &str) -> ComposeResult<()> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(ComposeError::parse(
            "missing_command_name",
            "Missing command name",
        ));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(ComposeError::parse(
            "invalid_command_name",
            format!("Invalid command name: {name}"),
        ));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
        return Err(ComposeError::parse(
            "invalid_command_name",
            format!("Invalid command name: {name}"),
        ));
    }
    Ok(())
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

fn advance(text: &str, line: &mut usize, column: &mut usize) {
    for ch in text.chars() {
        advance_char(ch, line, column);
    }
}

fn advance_char(ch: char, line: &mut usize, column: &mut usize) {
    if ch == '\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::compose::model::Segment;

    #[test]
    fn parses_multiline_interpolation_and_no_arg_commands() {
        let template = parse_template("A\n${{\n stdin |> trim\n}}\nB").unwrap();
        assert_eq!(template.segments.len(), 3);
        let Segment::Interpolation(interpolation) = &template.segments[1] else {
            panic!("expected interpolation");
        };
        assert_eq!(interpolation.location.line, 2);
        assert_eq!(interpolation.commands[0].name, "stdin");
        assert_eq!(interpolation.commands[1].name, "trim");
    }

    #[test]
    fn quoted_body_may_contain_pipeline_and_close_marker() {
        let template = parse_template(r#"${{file: "a|>b}}c" |> fallback: "x\ny"}}"#).unwrap();
        let Segment::Interpolation(interpolation) = &template.segments[0] else {
            panic!("expected interpolation");
        };
        assert_eq!(
            interpolation.commands[0].body.as_ref().unwrap().value,
            "a|>b}}c"
        );
        assert_eq!(
            interpolation.commands[1].body.as_ref().unwrap().value,
            "x\ny"
        );
    }

    #[test]
    fn rejects_meaningful_multiline_unquoted_body() {
        let err = parse_template("${{fallback: a\nb}}").unwrap_err();
        assert_eq!(err.code, "multiline_unquoted_body");

        let err = parse_template("${{file: a\nb}}").unwrap_err();
        assert_eq!(err.code, "multiline_unquoted_body");
    }

    #[test]
    fn escaped_interpolation_is_literal() {
        let template = parse_template(r"Use \${{file: a\}} now").unwrap();
        let Segment::Literal(text) = &template.segments[0] else {
            panic!("expected literal");
        };
        assert_eq!(text, "Use ${{file: a}} now");
    }
}
