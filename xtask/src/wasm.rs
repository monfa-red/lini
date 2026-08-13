//! Build the browser artifact — `cargo xtask wasm`.
//!
//! Three tools in a row, each doing one job:
//!
//! ```text
//!   cargo build --profile wasm-release --target wasm32-unknown-unknown
//!         │  the compiler, minus the font subsets (see crates/lini-wasm)
//!   wasm-bindgen --target web
//!         │  the JS glue — strings across the boundary, no `unsafe` on our side
//!   wasm-opt -Oz
//!         ▼  ~10 % off the raw module
//!   crates/lini-wasm/pkg/
//! ```
//!
//! `wasm-bindgen`'s CLI must match the `wasm-bindgen` crate version exactly, so
//! the mismatch is reported here rather than as a confusing runtime failure.
//! `wasm-opt` is optional: without it the artifact still works, just larger —
//! CI installs it, a local build need not.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const TARGET: &str = "wasm32-unknown-unknown";
const PROFILE: &str = "wasm-release";

pub fn build() -> ExitCode {
    let root = match workspace_root() {
        Some(r) => r,
        None => {
            eprintln!("cannot locate the workspace root");
            return ExitCode::FAILURE;
        }
    };
    let out = root.join("crates/lini-wasm/pkg");

    if !run(
        "cargo",
        &[
            "build",
            "--profile",
            PROFILE,
            "--target",
            TARGET,
            "-p",
            "lini-wasm",
        ],
        &root,
    ) {
        return ExitCode::FAILURE;
    }

    let module = root
        .join("target")
        .join(TARGET)
        .join(PROFILE)
        .join("lini_wasm.wasm");
    if !module.is_file() {
        eprintln!("expected {} after the build", module.display());
        return ExitCode::FAILURE;
    }

    if let Err(e) = check_bindgen_version(&root) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    if !run(
        "wasm-bindgen",
        &[
            "--target",
            "web",
            "--no-typescript",
            "--out-dir",
            &out.to_string_lossy(),
            &module.to_string_lossy(),
        ],
        &root,
    ) {
        return ExitCode::FAILURE;
    }

    let bg = out.join("lini_wasm_bg.wasm");
    optimize(&bg, &root);

    report(&out, &bg);
    ExitCode::SUCCESS
}

/// Run `wasm-opt -Oz` in place. Absent, the build still succeeds — the artifact
/// is simply the unoptimized one, and the size report says so.
fn optimize(bg: &Path, cwd: &Path) {
    let tmp = bg.with_extension("opt");
    let ok = run(
        "wasm-opt",
        &[
            "-Oz",
            // Rust emits both by default on wasm32 now; wasm-opt still gates
            // them, so a plain `-Oz` fails validation without these.
            "--enable-bulk-memory-opt",
            "--enable-nontrapping-float-to-int",
            "--strip-debug",
            "--strip-producers",
            "-o",
            &tmp.to_string_lossy(),
            &bg.to_string_lossy(),
        ],
        cwd,
    );
    if ok {
        let _ = std::fs::rename(&tmp, bg);
    } else {
        let _ = std::fs::remove_file(&tmp);
        eprintln!("note: wasm-opt not run — the module is larger than it needs to be");
        eprintln!("      install it with `brew install binaryen` (or via npm)");
    }
}

/// The CLI and the crate must agree exactly; a mismatch produces glue that
/// throws on load, which is a miserable thing to debug from the browser.
fn check_bindgen_version(root: &Path) -> Result<(), String> {
    let cli = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .map_err(|_| {
            "wasm-bindgen not found — install it with `cargo install wasm-bindgen-cli --version \
             <the version in Cargo.lock>`"
                .to_string()
        })?;
    let cli = String::from_utf8_lossy(&cli.stdout);
    let cli = cli.split_whitespace().nth(1).unwrap_or("").to_string();

    let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap_or_default();
    let crate_version = lock
        .split("[[package]]")
        .find(|p| p.contains("name = \"wasm-bindgen\"\n"))
        .and_then(|p| p.lines().find(|l| l.starts_with("version = ")))
        .and_then(|l| l.split('"').nth(1))
        .unwrap_or_default()
        .to_string();

    if crate_version.is_empty() || cli == crate_version {
        Ok(())
    } else {
        Err(format!(
            "wasm-bindgen CLI is {cli}, but the crate is {crate_version}\n  \
             fix: cargo install wasm-bindgen-cli --version {crate_version}"
        ))
    }
}

fn report(out: &Path, bg: &Path) {
    let bytes = std::fs::read(bg).unwrap_or_default();
    eprintln!("wrote {}", out.display());
    eprintln!("  lini_wasm_bg.wasm  {}", human(bytes.len()));
    if let Ok(js) = std::fs::metadata(out.join("lini_wasm.js")) {
        eprintln!("  lini_wasm.js       {}", human(js.len() as usize));
    }
}

fn human(n: usize) -> String {
    if n >= 1 << 20 {
        format!("{:.2} MB", n as f64 / (1 << 20) as f64)
    } else {
        format!("{:.1} KB", n as f64 / 1024.0)
    }
}

fn run(cmd: &str, args: &[&str], cwd: &Path) -> bool {
    Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .status()
        .is_ok_and(|s| s.success())
}

fn workspace_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
}
