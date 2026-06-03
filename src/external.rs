use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::cli::CommandContext;
use crate::runtime::input;
use crate::runtime::output::PrintMode;

pub const MAP_HELP: &str = r#"# External command mappings

Config files:
  ~/.config/agent-squire/config.toml
  .agent-squire.toml

Minimal format:

[commands.fetch]
run = ["python3", "~/skills/fetch_web.py"]
summary = "Fetch readable webpage content."
print_aware = true
expand_args = false

Behavior:
  squire fetch URL --print text

- The mapped command name is resolved from config.
- Remaining args are appended to the raw command.
- If print_aware = true and global --print is not compact, Squire appends:
    --print <mode>
- If expand_args = true, @stdin / @file:path / @env:NAME are expanded before execution.
- Use @@file:path to pass a literal @file:path when expand_args = true.
"#;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub commands: BTreeMap<String, MappedCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedCommand {
    pub run: Vec<String>,

    #[serde(default)]
    pub summary: Option<String>,

    #[serde(default)]
    pub print_aware: bool,

    #[serde(default)]
    pub expand_args: bool,
}

pub fn load_config() -> Config {
    let mut merged = Config::default();

    for path in config_paths() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match toml::from_str::<Config>(&text) {
            Ok(cfg) => {
                for (name, cmd) in cfg.commands {
                    if !cmd.run.is_empty() {
                        merged.commands.insert(name, cmd);
                    }
                }
            }
            Err(err) => {
                eprintln!("warning: failed to parse {}: {err}", path.display());
            }
        }
    }

    merged
}

pub fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        paths.push(Path::new(&home).join(".config/agent-squire/config.toml"));
    }

    if let Ok(appdata) = std::env::var("APPDATA") {
        paths.push(Path::new(&appdata).join("agent-squire/config.toml"));
    }

    paths.push(PathBuf::from(".agent-squire.toml"));
    paths
}

pub fn run_mapped(name: &str, args: Vec<OsString>, ctx: &CommandContext) -> Result<u8> {
    let config = load_config();
    let Some(mapped) = config.commands.get(name) else {
        bail!("unknown command '{name}'. Run 'squire list' or configure it in .agent-squire.toml");
    };

    if mapped.run.is_empty() {
        bail!("mapped command '{name}' has empty run vector");
    }

    let program = expand_home(&mapped.run[0]);
    let mut command = Command::new(program);
    command.current_dir(&ctx.cwd);

    for fixed in mapped.run.iter().skip(1) {
        command.arg(expand_home(fixed));
    }

    for arg in args {
        if mapped.expand_args {
            let s = arg.to_str().ok_or_else(|| {
                anyhow::anyhow!("mapped command args must be valid UTF-8 when expand_args = true")
            })?;
            command.arg(input::expand_arg_source(s)?);
        } else {
            command.arg(arg);
        }
    }

    if mapped.print_aware && ctx.print != PrintMode::Compact {
        command.arg("--print");
        command.arg(ctx.print.to_string());
    }

    let status = command
        .status()
        .with_context(|| format!("failed to run mapped command '{name}'"))?;

    Ok(status.code().unwrap_or(1).clamp(0, 255) as u8)
}

fn expand_home(value: &str) -> String {
    if value == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| value.to_string());
    }

    if let Some(rest) = value.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }

    value.to_string()
}
