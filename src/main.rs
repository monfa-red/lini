use clap::{Parser, error::ErrorKind};
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
)]
struct Cli {
    /// Input .lini file (use '-' for stdin)
    input: String,

    /// Output path (default: stdout)
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Output wrapper: `svg` (default) or `html`.
    #[arg(long = "format", default_value = "svg")]
    format: String,

    /// Force-embed the default `<style>` block — already the default; accepted for compatibility.
    #[arg(long = "standalone")]
    standalone: bool,

    /// Emit `var()` values inline as their resolved literal. Necessary for
    /// renderers without CSS-variable support (resvg, librsvg, raster
    /// converters).
    #[arg(long = "bake-vars")]
    bake_vars: bool,

    /// Parse and validate only — no layout, no render.
    #[arg(long = "check")]
    check: bool,

    /// CSS file with `--lini-*` overrides. Applied over the built-in defaults;
    /// layout vars from the theme bake into the layout.
    #[arg(long = "theme", value_name = "FILE")]
    theme: Option<PathBuf>,

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
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "fmt" {
        return run_fmt(&args[2..]);
    }
    if args.len() >= 2 && args[1] == "serve" {
        return run_serve(&args[2..]);
    }
    if args.len() >= 2 && args[1] == "desugar" {
        return run_desugar(&args[2..]);
    }

    let cli = match Cli::try_parse() {
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

    let format = match cli.format.as_str() {
        "svg" => lini::OutputFormat::Svg,
        "html" => lini::OutputFormat::Html,
        other => {
            eprintln!("error: invalid --format '{}' (expected svg|html)", other);
            return ExitCode::from(3);
        }
    };
    // `--standalone` is explicitly the default and is therefore a no-op flag.
    // Accept it for spec compliance.
    let _ = cli.standalone;

    if cli.watch {
        let out_path = cli.output.clone().expect("clap enforces -o with --watch");
        if cli.input == "-" {
            eprintln!("error: --watch cannot read from stdin");
            return ExitCode::from(3);
        }
        let theme_css = match &cli.theme {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!("error: {}: {}", path.display(), e);
                    return ExitCode::from(2);
                }
            },
            None => None,
        };
        let opts = lini::Options {
            bake_vars: cli.bake_vars,
            format,
            theme_css,
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

    let theme_css = match &cli.theme {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("error: {}: {}", path.display(), e);
                return ExitCode::from(2);
            }
        },
        None => None,
    };

    let opts = lini::Options {
        bake_vars: cli.bake_vars,
        format,
        theme_css,
    };

    if cli.check {
        return match lini::check_with(&source, &opts) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}", e.display_with_source(&source, &filename));
                ExitCode::from(1)
            }
        };
    }

    let mut warnings_were_emitted = false;
    if !cli.no_warn
        && let Ok(diags) = lini::lint_str(&source)
    {
        for d in &diags {
            eprintln!("{}", d.display_with_source(&source, &filename));
        }
        warnings_were_emitted |= !diags.is_empty();
    }

    // Compile and collect the routing relaxations in one layout pass — the wire
    // router is expensive, so we don't route once for the SVG and again for warnings.
    match lini::compile_str_checked(&source, &opts) {
        Ok((svg, route_diags)) => {
            if !cli.no_warn {
                // Impossible wires and law breaches — WIRING requires these
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

fn run_serve(args: &[String]) -> ExitCode {
    let mut port: u16 = 7700;
    let mut bake_vars = false;
    let mut input: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--port" => {
                i += 1;
                let p = match args.get(i) {
                    Some(s) => s,
                    None => {
                        eprintln!("error: --port needs a value");
                        return ExitCode::from(3);
                    }
                };
                port = match p.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("error: --port: invalid number '{}'", p);
                        return ExitCode::from(3);
                    }
                };
            }
            "--bake-vars" => bake_vars = true,
            "-h" | "--help" => {
                println!("lini serve [PATH] [--port N] [--bake-vars]");
                println!();
                println!("  Serves a live preview at http://127.0.0.1:<port>/.");
                println!();
                println!("  PATH a .lini file   Preview that one file; reloads on every save.");
                println!("  PATH a directory    Open the playground over its .lini files.");
                println!("  PATH omitted        Playground over the current directory.");
                println!();
                println!("  --port N    Port to bind (default 7700).");
                println!(
                    "  --bake-vars Bake CSS variables as literals (for non-browser renderers)."
                );
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with("--") => {
                eprintln!("error: unknown flag for `lini serve`: {}", flag);
                return ExitCode::from(3);
            }
            other => {
                if input.is_some() {
                    eprintln!("error: `lini serve` takes one input file");
                    return ExitCode::from(3);
                }
                input = Some(other.to_string());
            }
        }
        i += 1;
    }
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
        bake_vars,
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

fn run_fmt(args: &[String]) -> ExitCode {
    let mut check = false;
    let mut to_stdout = false;
    let mut input: Option<String> = None;
    for a in args {
        match a.as_str() {
            "--check" => check = true,
            "--stdout" => to_stdout = true,
            "-h" | "--help" => {
                println!("lini fmt [--check] [--stdout] <input.lini>");
                println!();
                println!(
                    "  --check   Exit 1 if reformatting would change the file. Write nothing."
                );
                println!("  --stdout  Print formatted output to stdout instead of rewriting.");
                println!("  -         Read stdin → stdout.");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with("--") => {
                eprintln!("error: unknown flag for `lini fmt`: {}", flag);
                return ExitCode::from(3);
            }
            other => {
                if input.is_some() {
                    eprintln!("error: `lini fmt` takes one input file");
                    return ExitCode::from(3);
                }
                input = Some(other.to_string());
            }
        }
    }
    let input_arg = match input {
        Some(p) => p,
        None => {
            eprintln!("error: `lini fmt` requires an input file (or '-' for stdin)");
            return ExitCode::from(3);
        }
    };

    let (filename, source) = if input_arg == "-" {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("error: failed to read stdin: {}", e);
            return ExitCode::from(2);
        }
        ("<stdin>".to_string(), buf)
    } else {
        match std::fs::read_to_string(&input_arg) {
            Ok(s) => (input_arg.clone(), s),
            Err(e) => {
                eprintln!("error: {}: {}", input_arg, e);
                return ExitCode::from(2);
            }
        }
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
        && let Err(e) = std::fs::write(&input_arg, formatted.as_bytes())
    {
        eprintln!("error: write {}: {}", input_arg, e);
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

fn run_desugar(args: &[String]) -> ExitCode {
    let mut input: Option<String> = None;
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                println!("lini desugar <input.lini>");
                println!();
                println!("  Expand label and wire-label sugar into the explicit children it");
                println!("  stands for, and print to stdout. Types, vars, and attrs are kept");
                println!("  as written; comments are dropped. Use '-' to read stdin.");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with('-') && flag != "-" => {
                eprintln!("error: unknown flag for `lini desugar`: {}", flag);
                return ExitCode::from(3);
            }
            other => {
                if input.is_some() {
                    eprintln!("error: `lini desugar` takes one input file");
                    return ExitCode::from(3);
                }
                input = Some(other.to_string());
            }
        }
    }
    let input_arg = match input {
        Some(p) => p,
        None => {
            eprintln!("error: `lini desugar` requires an input file (or '-' for stdin)");
            return ExitCode::from(3);
        }
    };

    let (filename, source) = if input_arg == "-" {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("error: failed to read stdin: {}", e);
            return ExitCode::from(2);
        }
        ("<stdin>".to_string(), buf)
    } else {
        match std::fs::read_to_string(&input_arg) {
            Ok(s) => (input_arg.clone(), s),
            Err(e) => {
                eprintln!("error: {}: {}", input_arg, e);
                return ExitCode::from(2);
            }
        }
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
