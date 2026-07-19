use crate::json::J;
use crate::span::{Span, line_col};
use std::fmt;

pub mod codes;
pub use codes::{Code, Phase};

#[cfg(test)]
mod tests;

/// A safe, machine-applicable edit [ROADMAP 3.8]: replace the source over
/// `span` with `replacement` verbatim and the file recompiles clean. Only
/// attached where the fix is genuinely derivable (a did-you-mean name, a
/// pointer-style correction) — never a prose-only hint.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub span: Span,
    pub replacement: String,
}

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub span: Span,
    /// A secondary span rendered inline as `(previously at L:C)` — used for
    /// duplicate-definition diagnostics [SPEC 20]. Only shown with source.
    pub related: Option<Span>,
    /// The stable diagnostic code [decision 7]. `Code::UNSPECIFIED` until a
    /// phase boundary ([`Error::in_phase`]) stamps the phase.
    pub code: Code,
    /// A machine-applicable replacement, where one honestly exists. Boxed: it
    /// is rarely set, and keeps `Result<_, Error>` small on the hot path.
    pub suggestion: Option<Box<Suggestion>>,
}

impl Error {
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span,
            related: None,
            code: Code::UNSPECIFIED,
            suggestion: None,
        }
    }

    /// Attach the span of a prior definition, shown as `(previously at L:C)`.
    pub fn with_related(mut self, span: Span) -> Self {
        self.related = Some(span);
        self
    }

    /// Attach the stable diagnostic code naming this error's family.
    pub fn code(mut self, code: Code) -> Self {
        self.code = code;
        self
    }

    /// Attach a machine-applicable replacement over `span`.
    pub fn suggest(mut self, span: Span, replacement: impl Into<String>) -> Self {
        self.suggestion = Some(Box::new(Suggestion {
            span,
            replacement: replacement.into(),
        }));
        self
    }

    /// Stamp the phase at a phase boundary — fills the generic `x000` code onto
    /// an untriaged error, leaving any named family code untouched.
    pub fn in_phase(mut self, phase: Phase) -> Self {
        if self.code.is_unspecified() {
            self.code = Code::generic(phase);
        }
        self
    }

    pub fn display_with_source<'a>(
        &'a self,
        source: &'a str,
        filename: &'a str,
    ) -> ErrorDisplay<'a> {
        ErrorDisplay {
            err: self,
            source,
            filename,
        }
    }

    /// The structured record as a JSON value [decision 9] — the `--json` form.
    pub fn to_json(&self, source: &str) -> J {
        diag_json(
            self.code,
            "error",
            &self.message,
            self.span,
            self.related,
            self.suggestion.as_deref(),
            source,
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.message)
    }
}

impl std::error::Error for Error {}

pub struct ErrorDisplay<'a> {
    err: &'a Error,
    source: &'a str,
    filename: &'a str,
}

impl<'a> fmt::Display for ErrorDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line, col) = line_col(self.source, self.err.span.start);
        write!(
            f,
            "{}:{}:{}: error: {}",
            self.filename, line, col, self.err.message
        )?;
        if let Some(related) = self.err.related {
            let (rl, rc) = line_col(self.source, related.start);
            write!(f, " (previously at {}:{})", rl, rc)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: Level,
    pub message: String,
    pub span: Span,
    pub related: Option<Span>,
    pub code: Code,
    pub suggestion: Option<Suggestion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// A hard diagnostic [SPEC 16/20] — the CLI fails on it like a compile error.
    Error,
    Warning,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
        }
    }
}

impl Diagnostic {
    pub fn warn(span: Span, message: impl Into<String>) -> Self {
        Self::new(Level::Warning, span, message)
    }

    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self::new(Level::Error, span, message)
    }

    fn new(level: Level, span: Span, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            span,
            related: None,
            code: Code::UNSPECIFIED,
            suggestion: None,
        }
    }

    /// Attach the stable diagnostic code naming this diagnostic's family.
    pub fn code(mut self, code: Code) -> Self {
        self.code = code;
        self
    }

    /// Attach a related span (a prior or conflicting definition).
    pub fn with_related(mut self, span: Span) -> Self {
        self.related = Some(span);
        self
    }

    /// Attach a machine-applicable replacement over `span`.
    pub fn suggest(mut self, span: Span, replacement: impl Into<String>) -> Self {
        self.suggestion = Some(Suggestion {
            span,
            replacement: replacement.into(),
        });
        self
    }

    /// Stamp the phase onto an untriaged diagnostic, leaving a named code alone.
    pub fn in_phase(mut self, phase: Phase) -> Self {
        if self.code.is_unspecified() {
            self.code = Code::generic(phase);
        }
        self
    }

    pub fn display_with_source<'a>(
        &'a self,
        source: &'a str,
        filename: &'a str,
    ) -> DiagnosticDisplay<'a> {
        DiagnosticDisplay {
            diag: self,
            source,
            filename,
        }
    }

    /// The structured record as a JSON value [decision 9] — the `--json` form.
    pub fn to_json(&self, source: &str) -> J {
        diag_json(
            self.code,
            self.level.as_str(),
            &self.message,
            self.span,
            self.related,
            self.suggestion.as_ref(),
            source,
        )
    }
}

/// Stamp a phase onto every untriaged diagnostic in a pass's output.
pub fn stamp_phase(mut diags: Vec<Diagnostic>, phase: Phase) -> Vec<Diagnostic> {
    for d in &mut diags {
        if d.code.is_unspecified() {
            d.code = Code::generic(phase);
        }
    }
    diags
}

pub struct DiagnosticDisplay<'a> {
    diag: &'a Diagnostic,
    source: &'a str,
    filename: &'a str,
}

impl<'a> fmt::Display for DiagnosticDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line, col) = line_col(self.source, self.diag.span.start);
        write!(
            f,
            "{}:{}:{}: {}: {}",
            self.filename,
            line,
            col,
            self.diag.level.as_str(),
            self.diag.message
        )
    }
}

// ── The structured JSON form [decision 9] — one document, serde-free ──

fn span_json(span: Span, source: &str) -> J {
    let (line, col) = line_col(source, span.start);
    let (end_line, end_col) = line_col(source, span.end);
    J::Obj(vec![
        ("start", J::Int(span.start as i64)),
        ("end", J::Int(span.end as i64)),
        ("line", J::Int(line as i64)),
        ("col", J::Int(col as i64)),
        ("endLine", J::Int(end_line as i64)),
        ("endCol", J::Int(end_col as i64)),
    ])
}

fn diag_json(
    code: Code,
    severity: &str,
    message: &str,
    span: Span,
    related: Option<Span>,
    suggestion: Option<&Suggestion>,
    source: &str,
) -> J {
    let mut obj = vec![
        ("code", J::s(code.as_str())),
        ("family", J::s(code.family)),
        ("severity", J::s(severity)),
        ("message", J::s(message)),
        ("span", span_json(span, source)),
    ];
    if let Some(r) = related {
        obj.push(("related", span_json(r, source)));
    }
    if let Some(s) = suggestion {
        obj.push((
            "suggestion",
            J::Obj(vec![
                ("span", span_json(s.span, source)),
                ("replacement", J::s(s.replacement.clone())),
                // A tool may apply this edit verbatim and the file recompiles.
                ("applicability", J::s("machine-applicable")),
            ]),
        ));
    }
    J::Obj(obj)
}

/// Render a set of diagnostic JSON values as one document [decision 9]:
/// `{ "file": …, "diagnostics": [ … ] }`, via the shared serde-free printer.
pub fn diagnostics_document(items: Vec<J>, filename: &str) -> String {
    let doc = J::Obj(vec![
        ("file", J::s(filename)),
        ("diagnostics", J::Arr(items)),
    ]);
    crate::json::to_string(&doc)
}
