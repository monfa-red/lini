//! Local dev-server for `lini serve`. Hand-rolled HTTP/1.1 over `std::net` —
//! no async runtime, no extra deps.
//!
//! Two modes, chosen by what the command is pointed at ([`ServeTarget`]):
//!
//! - **File** — a live-reloading preview of one `.lini` file (the original
//!   behavior). See [`file_mode`].
//! - **Dir** — the playground: browse, edit, and render a directory's `.lini`
//!   files in the browser. See [`dir_mode`].

mod dir_mode;
mod file_mode;
mod http;

use crate::Options;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

/// What `lini serve` was pointed at — a single file, or a directory to open as
/// the playground.
pub enum ServeTarget {
    /// One `.lini` file: live-reloading preview of just that file.
    File(PathBuf),
    /// A directory: the playground over every `.lini` file beneath it.
    Dir(PathBuf),
}

/// Shared, read-mostly server state handed to every connection thread.
pub(crate) struct State {
    pub target: ServeTarget,
    pub opts: Options,
    /// Bumped by the file-mode watcher on each on-disk change; read by the SSE
    /// stream. Unused in dir mode.
    pub generation: Mutex<u64>,
}

/// Bind `127.0.0.1:port` and serve until interrupted.
pub fn serve(target: ServeTarget, port: u16, opts: Options) -> std::io::Result<()> {
    let state = Arc::new(State {
        target,
        opts,
        generation: Mutex::new(0),
    });

    if let ServeTarget::File(_) = state.target {
        let watcher = state.clone();
        thread::spawn(move || file_mode::watch(watcher));
    }

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)?;
    match &state.target {
        ServeTarget::File(f) => {
            eprintln!(
                "lini serve: {} → http://{addr}/  (Ctrl-C to stop)",
                f.display()
            )
        }
        ServeTarget::Dir(d) => eprintln!(
            "lini playground: {} → http://{addr}/  (Ctrl-C to stop)",
            d.display()
        ),
    }

    for stream in listener.incoming() {
        let stream = stream?;
        let state = state.clone();
        thread::spawn(move || {
            if let Err(e) = handle(stream, &state) {
                // Browsers drop SSE streams on navigation; that's normal, not noise.
                if e.kind() != std::io::ErrorKind::BrokenPipe
                    && e.kind() != std::io::ErrorKind::ConnectionReset
                {
                    eprintln!("conn: {e}");
                }
            }
        });
    }
    Ok(())
}

/// The served page's own copy of the theme's variables, declared on `:root`.
/// A figure paints no background it was not given [SPEC 18], so the *page* is
/// what stands in for the paper — and it can only paint `var(--lini-bg)` if the
/// palette reaches it. Empty when no theme is set: each page's own fallback then
/// keeps its default look. A declaration carrying `<` is dropped — a theme file
/// is CSS, and CSS must not close the `<style>` it lands in.
fn theme_style(opts: &Options) -> String {
    let Some(css) = &opts.theme_css else {
        return String::new();
    };
    let decls: Vec<String> = crate::extract_lini_vars(css)
        .into_iter()
        .filter(|(n, v)| !n.contains('<') && !v.contains('<'))
        .map(|(n, v)| format!("--lini-{n}: {v};"))
        .collect();
    if decls.is_empty() {
        return String::new();
    }
    format!("<style>:root {{ {} }}</style>", decls.join(" "))
}

fn handle(mut stream: TcpStream, state: &State) -> std::io::Result<()> {
    let req = http::read_request(&mut stream)?;
    match state.target {
        ServeTarget::File(_) => file_mode::handle(&mut stream, &req, state),
        ServeTarget::Dir(_) => dir_mode::handle(&mut stream, &req, state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_reaches_the_page_as_root_variables() {
        let opts = Options {
            theme_css: crate::builtin_css("blueprint"),
            ..Default::default()
        };
        let style = theme_style(&opts);
        assert!(style.starts_with("<style>:root {"), "{style}");
        assert!(style.contains("--lini-bg: #00509e;"), "{style}");
        // No theme, no declarations — the pages keep their own fallbacks.
        assert!(theme_style(&Options::default()).is_empty());
    }

    #[test]
    fn both_pages_take_the_theme_and_paint_its_paper() {
        // Both halves of the one mechanism: the slot the palette lands in, and
        // the pane that reads `--lini-bg` from it (with today's look as the
        // fallback, so an unthemed serve is unchanged).
        for page in [file_mode::PAGE, dir_mode::PAGE] {
            assert!(page.contains("{{THEME}}"), "no theme slot");
            assert!(page.contains("background: var(--lini-bg, "), "no paper");
        }
    }
}
