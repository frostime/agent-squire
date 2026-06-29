#[derive(Debug, Clone)]
pub struct SpecAst {
    pub shares: Vec<ShareAst>,
    pub arranges: Vec<ArrangeAst>,
}

#[derive(Debug, Clone)]
pub struct ShareAst {
    pub slug: String,
    pub path: String,
    pub items: Vec<ShareItemAst>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ShareItemAst {
    pub name: String,
    pub range: RangeExpr,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ArrangeAst {
    pub slug: Option<String>,
    pub path: String,
    pub before: FileState<BeforeItem>,
    pub after: FileState<AfterItem>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum FileState<T> {
    Missing,
    Empty,
    Sequence(Vec<T>),
}

#[derive(Debug, Clone)]
pub enum BeforeItem {
    Anonymous {
        range: RangeExpr,
        line: usize,
    },
    Named {
        name: String,
        range: RangeExpr,
        line: usize,
    },
    Gap {
        name: String,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub enum AfterItem {
    Anonymous {
        range: RangeExpr,
        line: usize,
    },
    Local {
        name: String,
        line: usize,
    },
    Gap {
        name: String,
        line: usize,
    },
    External {
        slug: String,
        name: String,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub struct RangeExpr {
    pub raw: String,
    pub start: usize,
    pub end: RangeEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeEnd {
    Line(usize),
    End,
}

impl RangeExpr {
    pub fn resolved_end(&self, line_count: usize) -> usize {
        match self.end {
            RangeEnd::Line(n) => n,
            RangeEnd::End => line_count,
        }
    }
}
