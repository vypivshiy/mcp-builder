use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl DiagnosticSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub line: usize,   // 1-indexed
    pub column: usize, // 1-indexed
    pub offset: usize, // 0-indexed byte offset
    pub length: usize,
}

impl SourceLocation {
    pub fn from_offset(source: &str, offset: usize, length: usize) -> Self {
        let mut line = 1;
        let mut col = 1;
        let mut curr = 0;

        for ch in source.chars() {
            if curr >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            curr += ch.len_utf8();
        }

        Self {
            line,
            column: col,
            offset,
            length,
        }
    }

    pub fn find_in_source(source: &str, search_term: &str, context_offset: Option<usize>) -> Option<Self> {
        if search_term.is_empty() {
            return None;
        }

        let start_pos = context_offset.unwrap_or(0);
        let search_slice = if start_pos < source.len() {
            &source[start_pos..]
        } else {
            source
        };

        let found_offset = if let Some(pos) = search_slice.find(search_term) {
            start_pos + pos
        } else if let Some(pos) = source.find(search_term) {
            pos
        } else {
            return None;
        };

        Some(Self::from_offset(source, found_offset, search_term.len()))
    }

    pub fn find_node_name(source: &str, node_name: &str, context_offset: Option<usize>) -> Option<Self> {
        if node_name.is_empty() {
            return None;
        }

        let start_pos = context_offset.unwrap_or(0);
        let search_slice = if start_pos < source.len() {
            &source[start_pos..]
        } else {
            source
        };

        for (idx, _) in search_slice.match_indices(node_name) {
            let abs_idx = start_pos + idx;
            let prev_ok = if abs_idx == 0 {
                true
            } else {
                let prev_ch = source[..abs_idx].chars().next_back().unwrap_or(' ');
                prev_ch.is_whitespace() || prev_ch == ';' || prev_ch == '{' || prev_ch == '}' || prev_ch == '\n' || prev_ch == '\r'
            };

            let after_idx = abs_idx + node_name.len();
            let next_ok = if after_idx >= source.len() {
                true
            } else {
                let next_ch = source[after_idx..].chars().next().unwrap_or(' ');
                next_ch.is_whitespace() || next_ch == '{' || next_ch == '"' || next_ch == '=' || next_ch == ';' || next_ch == '\n' || next_ch == '\r'
            };

            if prev_ok && next_ok {
                return Some(Self::from_offset(source, abs_idx, node_name.len()));
            }
        }

        Self::find_in_source(source, node_name, context_offset)
    }

    pub fn find_property(source: &str, prop_name: &str, context_offset: Option<usize>) -> Option<Self> {
        if prop_name.is_empty() {
            return None;
        }
        if let Some(mut loc) = Self::find_in_source(source, &format!("{}=\"", prop_name), context_offset) {
            loc.length = prop_name.len();
            return Some(loc);
        }
        if let Some(mut loc) = Self::find_in_source(source, &format!("{}=", prop_name), context_offset) {
            loc.length = prop_name.len();
            return Some(loc);
        }
        if let Some(mut loc) = Self::find_in_source(source, &format!("{} =", prop_name), context_offset) {
            loc.length = prop_name.len();
            return Some(loc);
        }
        Self::find_node_name(source, prop_name, context_offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub location: Option<SourceLocation>,
    pub label: Option<String>,
    pub help: Option<String>,
    pub note: Option<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            location: None,
            label: None,
            help: None,
            note: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            location: None,
            label: None,
            help: None,
            note: None,
        }
    }

    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Info,
            message: message.into(),
            location: None,
            label: None,
            help: None,
            note: None,
        }
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub file_name: String,
    pub source: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    pub fn new(file_name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            file_name: file_name.into(),
            source: source.into(),
            diagnostics: Vec::new(),
        }
    }

    pub fn add(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count()
    }

    pub fn render_plain(&self) -> String {
        self.render(false)
    }

    pub fn render_ansi(&self) -> String {
        self.render(true)
    }

    pub fn render(&self, color: bool) -> String {
        let (red, yellow, cyan, green, bold, reset) = if color {
            (
                "\x1b[31;1m",
                "\x1b[33;1m",
                "\x1b[36;1m",
                "\x1b[32;1m",
                "\x1b[1m",
                "\x1b[0m",
            )
        } else {
            ("", "", "", "", "", "")
        };

        let mut out = String::new();

        for (i, diag) in self.diagnostics.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }

            let (sev_color, sev_text) = match diag.severity {
                DiagnosticSeverity::Error => (red, "error"),
                DiagnosticSeverity::Warning => (yellow, "warning"),
                DiagnosticSeverity::Info => (cyan, "info"),
            };

            // Header line: error[E0020]: message
            out.push_str(&format!(
                "{}{}[{}]{}: {}{}{}\n",
                sev_color, sev_text, diag.code, reset, bold, diag.message, reset
            ));

            if let Some(loc) = &diag.location {
                // Location line: --> file:line:col
                out.push_str(&format!(
                    "  {}-->{} {}:{}:{}\n",
                    cyan, reset, self.file_name, loc.line, loc.column
                ));

                // Find the source line corresponding to loc.line
                let mut current_line = 1;
                let mut line_start = 0;
                let mut line_end = self.source.len();

                for (idx, ch) in self.source.char_indices() {
                    if current_line == loc.line && line_start == 0 && loc.line > 1 {
                        line_start = idx;
                    }
                    if ch == '\n' {
                        if current_line == loc.line {
                            line_end = idx;
                            break;
                        }
                        current_line += 1;
                        if current_line == loc.line {
                            line_start = idx + 1;
                        }
                    }
                }

                if line_start < self.source.len() && line_end >= line_start {
                    let raw_line = &self.source[line_start..std::cmp::min(line_end, self.source.len())];
                    let line_trimmed = raw_line.trim_end_matches('\r');

                    let line_num_str = format!("{}", loc.line);
                    let _padding = " ".repeat(line_num_str.len());

                    out.push_str(&format!("   {}|{}\n", cyan, reset));
                    out.push_str(&format!(
                        " {}{}{} | {}\n",
                        cyan, line_num_str, reset, line_trimmed
                    ));

                    // Caret line
                    let col_offset = loc.column.saturating_sub(1);
                    let mut caret_pad = String::new();
                    let mut curr_col = 0;
                    for ch in line_trimmed.chars() {
                        if curr_col >= col_offset {
                            break;
                        }
                        if ch == '\t' {
                            caret_pad.push_str("    ");
                            curr_col += 4;
                        } else {
                            caret_pad.push(' ');
                            curr_col += 1;
                        }
                    }

                    let caret_len = std::cmp::max(1, std::cmp::min(loc.length, line_trimmed.len().saturating_sub(col_offset)));
                    let carets = "^".repeat(caret_len);

                    if let Some(label) = &diag.label {
                        out.push_str(&format!(
                            "   {}|{} {}{}{}{} {}{}\n",
                            cyan, reset, caret_pad, sev_color, carets, reset, label, reset
                        ));
                    } else {
                        out.push_str(&format!(
                            "   {}|{} {}{}{}{}\n",
                            cyan, reset, caret_pad, sev_color, carets, reset
                        ));
                    }
                    out.push_str(&format!("   {}|{}\n", cyan, reset));
                }
            }

            if let Some(help) = &diag.help {
                out.push_str(&format!(
                    "   {}={}{} {}help:{} {}\n",
                    cyan, reset, bold, green, reset, help
                ));
            }

            if let Some(note) = &diag.note {
                out.push_str(&format!(
                    "   {}={}{} {}note:{} {}\n",
                    cyan, reset, bold, cyan, reset, note
                ));
            }
        }

        out
    }
}

// -----------------------------------------------------------------------------
// Levenshtein Typo Suggestions Engine (Re-exported from helpers)
// -----------------------------------------------------------------------------
pub use crate::helpers::{find_closest_match, levenshtein_distance};
