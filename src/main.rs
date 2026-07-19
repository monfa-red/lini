use clap::{Args, Parser, Subcommand, error::ErrorKind};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "lini",
    version,
    about = "Compile Lini diagrams to SVG",
    long_about = None,
    disable_help_flag = false,
    args_conflicts_with_subcommands = true,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// The default (no-subcommand) invocation: compile a file to SVG.
    #[command(flatten)]
    compile: Option<CompileArgs>,
}

#[derive(Subcommand)]
enum Command {
    /// Reformat a file to canonical style in place
    Fmt {
        /// Input .lini file (use '-' for stdin → stdout)
        input: String,
        /// Exit 1 if reformatting would change the file. Write nothing.
        #[arg(long = "check")]
        check: bool,
        /// Print formatted output to stdout instead of rewriting.
        #[arg(long = "stdout")]
        stdout: bool,
    },
    /// Serve a live preview (a file) or the playground (a directory)
    Serve {
        /// A .lini file (live-reload preview) or a directory (playground);
        /// omitted → the current directory.
        path: Option<String>,
        /// Port to bind (default 7700).
        #[arg(long = "port", default_value_t = 7700)]
        port: u16,
        /// Bake CSS variables and outline text (for non-browser renderers).
        #[arg(long = "static")]
        static_mode: bool,
    },
    /// Print a file lowered to primitives + .lini-* classes
    Desugar {
        /// Input .lini file (use '-' for stdin)
        input: String,
    },
    /// List the built-in themes, or print one as a --lini-* CSS file
    Theme {
        /// A theme name to print; omitted → list them all.
        name: Option<String>,
    },
}

#[derive(Args)]
struct CompileArgs {
    /// Input .lini file (use '-' for stdin)
    input: String,

    /// Output path (default: stdout)
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Output wrapper: `svg` (default) or `html`.
    #[arg(long = "format", default_value = "svg")]
    format: String,

    /// Inline `var()`s as literals **and** outline text to paths —
    /// self-contained for any renderer (resvg, librsvg, raster converters).
    #[arg(long = "static")]
    static_mode: bool,

    /// Embed the used bundled font family × weights as base64 `@font-face` —
    /// browser-only (resvg/librsvg ignore `@font-face`).
    #[arg(long = "embed-font")]
    embed_font: bool,

    /// Parse and validate only — no layout, no render.
    #[arg(long = "check")]
    check: bool,

    /// Emit diagnostics as a JSON document (stable codes, spans, and
    /// machine-applicable fixes) instead of SVG — the tooling/LSP form
    /// [SPEC 19/20]. Exit 1 if any error-level diagnostic fired.
    #[arg(long = "json")]
    json: bool,

    /// A theme: a built-in name (`dark`, `high-contrast`, …), a CSS file of `--lini-*`
    /// overrides, or a light/dark pair (`light/dark`). See `lini theme`.
    #[arg(long = "theme", value_name = "NAME|FILE|A/B")]
    theme: Option<String>,

    /// Recompile on every change to the input file. Requires `-o`.
    #[arg(long = "watch", requires = "output")]
    watch: bool,

    /// Suppress lint warnings.
    #[arg(long = "no-warn", conflicts_with = "strict")]
    no_warn: bool,

    /// Treat lint warnings as errors. Useful for CI.
    #[arg(long = "strict")]
    strict: bool,
}

fn main() -> ExitCode {
    let parsed = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            // clap prints help/version to stdout and errors to stderr itself.
            let _ = e.print();
            return match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::SUCCESS,
                _ => ExitCode::from(3),
            };
        }
    };
    let cli = match parsed.command {
        Some(Command::Fmt {
            input,
            check,
            stdout,
        }) => return run_fmt(&input, check, stdout),
        Some(Command::Serve {
            path,
            port,
            static_mode,
        }) => return run_serve(path, port, static_mode),
        Some(Command::Desugar { input }) => return run_desugar(&input),
        Some(Command::Theme { name }) => return run_theme(name.as_deref()),
        None => match parsed.compile {
            Some(c) => c,
            None => {
                eprintln!("error: an input file (or a subcommand) is required — see 'lini --help'");
                return ExitCode::from(3);
            }
        },
    };

    // The two font flags need the subset bytes — the default-on `font`
    // feature [SPEC 19]. Name-only output never does.
    if (cli.static_mode || cli.embed_font) && !lini::font_support() {
        eprintln!(
            "error: --static and --embed-font need the bundled fonts — rebuild with the `font` feature (on by default)"
        );
        return ExitCode::from(3);
    }

    let format = match cli.format.as_str() {
        "svg" => lini::OutputFormat::Svg,
        "html" => lini::OutputFormat::Html,
        other => {
            eprintln!("error: invalid --format '{}' (expected svg|html)", other);
            return ExitCode::from(3);
        }
    };
    // Resolve `--theme` (a built-in name, a light/dark pair, or a file) to the
    // `--lini-*` CSS the resolver layers over the defaults.
    let theme_css = match &cli.theme {
        Some(arg) => match theme_css_for(arg) {
            Ok(css) => Some(css),
            Err(code) => return code,
        },
        None => None,
    };

    // Local image paths resolve against the source file's directory [SPEC 7];
    // stdin has none. A plain CLI compile is unbounded — no asset root [SPEC 19].
    let base_dir = (cli.input != "-")
        .then(|| Path::new(&cli.input).parent().map(Path::to_path_buf))
        .flatten();

    if cli.watch {
        let out_path = cli.output.clone().expect("clap enforces -o with --watch");
        if cli.input == "-" {
            eprintln!("error: --watch cannot read from stdin");
            return ExitCode::from(3);
        }
        let opts = lini::Options {
            static_mode: cli.static_mode,
            embed_font: cli.embed_font,
            format,
            theme_css,
            base_dir,
            ..Default::default()
        };
        return watch_loop(Path::new(&cli.input), &out_path, &opts, cli.check);
    }

    let (filename, source) = match cli.input.as_str() {
        "-" => {
            let mut buf = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
                eprintln!("error: failed to read stdin: {}", e);
                return ExitCode::from(2);
            }
            ("<stdin>".to_string(), buf)
        }
        path => match std::fs::read_to_string(path) {
            Ok(s) => (path.to_string(), s),
            Err(e) => {
                eprintln!("error: {}: {}", path, e);
                return ExitCode::from(2);
            }
        },
    };

    let opts = lini::Options {
        static_mode: cli.static_mode,
        embed_font: cli.embed_font,
        format,
        theme_css,
        base_dir,
        ..Default::default()
    };

    if cli.json {
        let (doc, had_error) = lini::diagnostics_json(&source, &opts, &filename);
        print!("{}", doc);
        return if had_error {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }

    if cli.check {
        return match lini::check_with(&source, &opts) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}", e.display_with_source(&source, &filename));
                ExitCode::from(1)
            }
        };
    }

    // Lint + the property validation pass [SPEC 16/20]: error-level
    // diagnostics always print and fail the compile; warnings print unless
    // `--no-warn` and fail only under `--strict`.
    let mut warnings_were_emitted = false;
    let mut validation_failed = false;
    if let Ok(diags) = lini::lint_str(&source) {
        for d in &diags {
            let is_error = d.level == lini::Level::Error;
            if is_error || !cli.no_warn {
                eprintln!("{}", d.display_with_source(&source, &filename));
            }
            validation_failed |= is_error;
            warnings_were_emitted |= !is_error && !cli.no_warn;
        }
    }
    if validation_failed {
        return ExitCode::from(1);
    }

    // Compile and collect the routing relaxations in one layout pass — the link
    // router is expensive, so we don't route once for the SVG and again for warnings.
    match lini::compile_str_checked(&source, &opts) {
        Ok((svg, route_diags)) => {
            if !cli.no_warn {
                // Impossible links and law breaches — ROUTING requires these
                // never be silent.
                for d in &route_diags {
                    eprintln!("{}", d.display_with_source(&source, &filename));
                }
                warnings_were_emitted |= !route_diags.is_empty();
            }
            if let Some(out_path) = cli.output {
                if let Err(e) = std::fs::write(&out_path, svg.as_bytes()) {
                    eprintln!("error: write {}: {}", out_path.display(), e);
                    return ExitCode::from(2);
                }
            } else {
                print!("{}", svg);
            }
            if cli.strict && warnings_were_emitted {
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", e.display_with_source(&source, &filename));
            ExitCode::from(1)
        }
    }
}

fn watch_loop(input: &Path, output: &Path, opts: &lini::Options, check_only: bool) -> ExitCode {
    eprintln!("watching {} → {}", input.display(), output.display());
    let mut last_signature = None;
    loop {
        let signature = std::fs::metadata(input)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| (t, std::fs::metadata(input).map(|m| m.len()).unwrap_or(0)));

        if signature != last_signature {
            last_signature = signature;
            recompile(input, output, opts, check_only);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn run_serve(input: Option<String>, port: u16, static_mode: bool) -> ExitCode {
    let target = match input {
        Some(p) => {
            let path = PathBuf::from(&p);
            if !path.exists() {
                eprintln!("error: {}: no such file or directory", path.display());
                return ExitCode::from(2);
            }
            if path.is_dir() {
                lini::ServeTarget::Dir(path)
            } else {
                lini::ServeTarget::File(path)
            }
        }
        None => {
            lini::ServeTarget::Dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        }
    };
    let opts = lini::Options {
        static_mode,
        ..Default::default()
    };
    match lini::serve(target, port, opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
fn run_fmt(input_arg: &str, check: bool, to_stdout: bool) -> ExitCode {
    let (filename, source) = match read_input(input_arg) {
        Ok(fs) => fs,
        Err(code) => return code,
    };

    let formatted = match lini::format_source(&source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.display_with_source(&source, &filename));
            return ExitCode::from(1);
        }
    };

    if check {
        if formatted == source {
            return ExitCode::SUCCESS;
        }
        eprintln!("{}: would reformat", filename);
        return ExitCode::from(1);
    }

    if input_arg == "-" || to_stdout {
        print!("{}", formatted);
    } else if formatted != source
        && let Err(e) = std::fs::write(input_arg, formatted.as_bytes())
    {
        eprintln!("error: write {}: {}", input_arg, e);
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

/// Read a subcommand's input file — `-` is stdin (named `<stdin>` in errors).
fn read_input(input_arg: &str) -> Result<(String, String), ExitCode> {
    if input_arg == "-" {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("error: failed to read stdin: {}", e);
            return Err(ExitCode::from(2));
        }
        Ok(("<stdin>".to_string(), buf))
    } else {
        match std::fs::read_to_string(input_arg) {
            Ok(s) => Ok((input_arg.to_string(), s)),
            Err(e) => {
                eprintln!("error: {}: {}", input_arg, e);
                Err(ExitCode::from(2))
            }
        }
    }
}
fn run_desugar(input_arg: &str) -> ExitCode {
    let (filename, source) = match read_input(input_arg) {
        Ok(fs) => fs,
        Err(code) => return code,
    };
    match lini::desugar_source(&source) {
        Ok(s) => {
            print!("{}", s);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", e.display_with_source(&source, &filename));
            ExitCode::from(1)
        }
    }
}
fn recompile(input: &Path, output: &Path, opts: &lini::Options, check_only: bool) {
    let start = Instant::now();
    let source = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: read {}: {}", input.display(), e);
            return;
        }
    };
    let filename = input.display().to_string();
    let result = if check_only {
        lini::check_with(&source, opts).map(|_| String::new())
    } else {
        lini::compile_str_with(&source, opts)
    };
    match result {
        Ok(_) if check_only => {
            eprintln!("ok ({} ms) — check passed", start.elapsed().as_millis());
        }
        Ok(svg) => match std::fs::write(output, svg.as_bytes()) {
            Ok(()) => eprintln!(
                "ok ({} ms) → {}",
                start.elapsed().as_millis(),
                output.display()
            ),
            Err(e) => eprintln!("error: write {}: {}", output.display(), e),
        },
        Err(e) => eprintln!("{}", e.display_with_source(&source, &filename)),
    }
}

/// Resolve a `--theme` argument to its `--lini-*` CSS: a built-in name, then a
/// `light/dark` pair of built-ins, else a file path.
fn theme_css_for(arg: &str) -> Result<String, ExitCode> {
    if let Some(css) = lini::builtin_css(arg) {
        return Ok(css);
    }
    if let Some((l, r)) = arg.split_once('/')
        && let Some(css) = lini::pair_css(l.trim(), r.trim())
    {
        return Ok(css);
    }
    match std::fs::read_to_string(arg) {
        Ok(s) => Ok(s),
        Err(e) => {
            eprintln!(
                "error: theme '{}': not a built-in (try: {}), and reading it as a file failed: {}",
                arg,
                theme_names(),
                e
            );
            Err(ExitCode::from(2))
        }
    }
}

fn theme_names() -> String {
    lini::list_themes()
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `lini theme [NAME]` — list the built-in themes, or print one as `--lini-*`
/// CSS for a user to copy [SPEC 17].
fn run_theme(name: Option<&str>) -> ExitCode {
    match name {
        None => {
            for (n, desc) in lini::list_themes() {
                println!("{:15} {}", n, desc);
            }
            ExitCode::SUCCESS
        }
        Some(n) => match lini::builtin_css(n) {
            Some(css) => {
                print!("{}", css);
                ExitCode::SUCCESS
            }
            None => {
                eprintln!(
                    "error: unknown theme '{}' (try one of: {})",
                    n,
                    theme_names()
                );
                ExitCode::from(3)
            }
        },
    }
}
