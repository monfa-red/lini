//! The compiler never calls the platform's libm.
//!
//! `src/math.rs` explains why: `f64::tan` and its neighbours resolve to Apple's
//! libm on macOS, glibc's on Linux, and a Rust one under `wasm32`, and the three
//! disagree by up to 1 ULP — enough to break the byte-identical output
//! [ROADMAP §2] promises and the README sells. Routing every call through
//! `crate::math` fixes it, and this test is what keeps it fixed: a single
//! `.atan2(` typed in a year from now fails here, naming the file and line,
//! instead of silently splitting the platforms again.
//!
//! Same shape as `tests/schema.rs` and `tests/grammar.rs` — a generated or
//! disciplined artifact guarded against drift by the suite rather than by
//! anyone's memory.

use std::fs;
use std::path::{Path, PathBuf};

/// The inherent `f64` methods that dispatch to the platform library. Each maps
/// to the `crate::math` function that replaces it.
const BANNED: &[(&str, &str)] = &[
    (".sin(", "math::sin(x)"),
    (".cos(", "math::cos(x)"),
    (".tan(", "math::tan(x)"),
    (".asin(", "math::asin(x) — add the wrapper"),
    (".acos(", "math::acos(x)"),
    (".atan(", "math::atan(x) — add the wrapper"),
    (".atan2(", "math::atan2(y, x)"),
    (".exp(", "math::exp(x)"),
    (".ln(", "math::ln(x)"),
    (".log10(", "math::log10(x)"),
    (".log2(", "math::log2(x) — add the wrapper"),
    (".powf(", "math::powf(base, exp)"),
    (".hypot(", "math::hypot(x, y)"),
    (".cbrt(", "math::cbrt(x) — add the wrapper"),
];

/// `src/math.rs` is the one file allowed to name them — it is the wrapper.
const EXEMPT: &str = "math.rs";

#[test]
fn no_source_file_calls_the_platform_libm() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offences = Vec::new();

    for file in rust_files(&src) {
        if file.file_name().is_some_and(|n| n == EXEMPT) {
            continue;
        }
        let text = fs::read_to_string(&file).expect("read a source file");
        for (line_no, line) in text.lines().enumerate() {
            // A doc comment may legitimately name the method it is warning about.
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for (method, fix) in BANNED {
                if code.contains(method) {
                    offences.push(format!(
                        "  {}:{}\n    {}\n    → use {fix}",
                        file.strip_prefix(&src).unwrap_or(&file).display(),
                        line_no + 1,
                        code.trim(),
                    ));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "these call the platform's libm, so macOS, Linux and wasm32 will disagree \
         by up to 1 ULP and the output stops being byte-identical across targets \
         (see src/math.rs):\n\n{}\n",
        offences.join("\n")
    );
}

/// A guard on the guard: if the walk ever stops finding files, the test above
/// passes vacuously and the rule quietly stops being enforced.
#[test]
fn the_scan_actually_reaches_the_source() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let n = rust_files(&src).len();
    assert!(
        n > 100,
        "only {n} source files scanned — has the tree moved?"
    );
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}
