use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    File {
        path: PathBuf,
        range: Option<LineRange>,
    },
    Dir {
        path: PathBuf,
    },
    Tree {
        path: PathBuf,
    },
    Glob {
        pattern: String,
    },
    SelectedGlob {
        label: String,
        files: Vec<PathBuf>,
    },
    Command {
        command: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

impl LineRange {
    pub fn new(start: usize, end: usize) -> anyhow::Result<Self> {
        if start == 0 {
            anyhow::bail!("Invalid range: start must be >= 1");
        }
        if end < start {
            anyhow::bail!("Invalid range: end must be >= start");
        }
        Ok(Self { start, end })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    File,
    Dir,
    Tree,
    Glob,
    Cmd,
}

impl Prefix {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "file" => Some(Self::File),
            "dir" => Some(Self::Dir),
            "tree" => Some(Self::Tree),
            "glob" => Some(Self::Glob),
            "cmd" => Some(Self::Cmd),
            _ => None,
        }
    }
}
