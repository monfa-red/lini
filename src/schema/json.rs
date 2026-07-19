//! A tiny serde-free JSON value + deterministic pretty-printer [Decision 8].
//! Objects keep insertion order (never a `HashMap`), so a regenerated schema is
//! byte-identical to the committed file — the drift check depends on it.

/// A JSON value. `Raw` carries an already-formatted token (a number literal).
pub enum J {
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<J>),
    /// An object as ordered `(key, value)` pairs — order is the emission order.
    Obj(Vec<(&'static str, J)>),
}

impl J {
    pub fn s(text: impl Into<String>) -> J {
        J::Str(text.into())
    }
}

/// Render a value as pretty JSON with two-space indent and a trailing newline.
pub fn to_string(root: &J) -> String {
    let mut out = String::new();
    write(root, 0, &mut out);
    out.push('\n');
    out
}

fn write(j: &J, indent: usize, out: &mut String) {
    match j {
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Int(n) => out.push_str(&n.to_string()),
        J::Str(s) => escape(s, out),
        J::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                pad(indent + 1, out);
                write(item, indent + 1, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            pad(indent, out);
            out.push(']');
        }
        J::Obj(entries) => {
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for (i, (key, val)) in entries.iter().enumerate() {
                pad(indent + 1, out);
                escape(key, out);
                out.push_str(": ");
                write(val, indent + 1, out);
                if i + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            pad(indent, out);
            out.push('}');
        }
    }
}

fn pad(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
