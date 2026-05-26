use std::env;
use std::fs;
use std::io::{self, Read};

use anyhow::{Context, Result, bail};

pub fn read_text_source(source: &str) -> Result<String> {
    if source == "@stdin" {
        let mut s = String::new();
        io::stdin()
            .read_to_string(&mut s)
            .context("failed to read stdin")?;
        return Ok(s);
    }

    if let Some(path) = source.strip_prefix("@file:") {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read input file {path}"));
    }

    if let Some(name) = source.strip_prefix("@env:") {
        return env::var(name)
            .with_context(|| format!("failed to read environment variable {name}"));
    }

    if let Some(escaped) = source.strip_prefix("@@") {
        return Ok(format!("@{escaped}"));
    }

    if source.starts_with('@') {
        bail!("unknown input source syntax: {source}");
    }

    Ok(source.to_string())
}

pub fn expand_arg_source(arg: &str) -> Result<String> {
    read_text_source(arg)
}
