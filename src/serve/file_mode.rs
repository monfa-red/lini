//! Single-file mode: `lini serve FILE`. Three routes —
//!
//! - `GET /`        → the preview page
//! - `GET /svg`     → compile FILE, return SVG (or a plain-text error)
//! - `GET /events`  → SSE stream pushing `reload` whenever FILE changes
//!
//! A background thread polls FILE's mtime/size every 200 ms and bumps a
//! generation counter; the SSE handler watches that counter. A 15 s heartbeat
//! keeps proxies from closing the idle stream.

use super::{ServeTarget, State, http};
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

const PAGE: &str = include_str!("single.html");

fn file(state: &State) -> &Path {
    match &state.target {
        ServeTarget::File(f) => f,
        ServeTarget::Dir(_) => unreachable!("file_mode runs only for a File target"),
    }
}

pub(super) fn watch(state: Arc<State>) {
    let file = file(&state).to_path_buf();
    let mut last: Option<(SystemTime, u64)> = None;
    loop {
        let sig = std::fs::metadata(&file)
            .and_then(|m| Ok((m.modified()?, m.len())))
            .ok();
        if sig != last {
            if last.is_some() {
                *state.generation.lock().unwrap() += 1;
            }
            last = sig;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub(super) fn handle(
    stream: &mut TcpStream,
    req: &http::Request,
    state: &State,
) -> std::io::Result<()> {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => serve_page(stream, state),
        ("GET", "/svg") => serve_svg(stream, state),
        ("GET", "/events") => serve_events(stream, state),
        ("GET", "/favicon.ico") => http::write_response(stream, 204, "text/plain", b""),
        _ => http::write_response(stream, 404, "text/plain", b"not found\n"),
    }
}

fn serve_page(stream: &mut TcpStream, state: &State) -> std::io::Result<()> {
    let title = file(state).display().to_string();
    let html = PAGE.replace("{{TITLE}}", &http::html_escape(&title));
    http::write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes())
}

fn serve_svg(stream: &mut TcpStream, state: &State) -> std::io::Result<()> {
    let path = file(state);
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("read error: {e}");
            return http::write_response(stream, 200, "text/plain; charset=utf-8", msg.as_bytes());
        }
    };
    // A file target's asset root is its own directory [SPEC 19]: image paths
    // resolve there and may not escape it.
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let opts = crate::Options {
        base_dir: Some(dir.clone()),
        asset_root: Some(dir),
        ..state.opts.clone()
    };
    match crate::compile_str_with(&src, &opts) {
        Ok(svg) => http::write_response(stream, 200, "image/svg+xml", svg.as_bytes()),
        Err(e) => {
            let filename = path.display().to_string();
            let msg = e.display_with_source(&src, &filename).to_string();
            http::write_response(stream, 200, "text/plain; charset=utf-8", msg.as_bytes())
        }
    }
}

fn serve_events(stream: &mut TcpStream, state: &State) -> std::io::Result<()> {
    let header = "HTTP/1.1 200 OK\r\n\
                  Content-Type: text/event-stream\r\n\
                  Cache-Control: no-cache\r\n\
                  Connection: keep-alive\r\n\
                  \r\n";
    stream.write_all(header.as_bytes())?;
    stream.flush()?;

    let mut last_gen = *state.generation.lock().unwrap();
    let mut last_beat = Instant::now();
    loop {
        let current = *state.generation.lock().unwrap();
        if current != last_gen {
            last_gen = current;
            stream.write_all(b"event: reload\ndata: 1\n\n")?;
            stream.flush()?;
        }
        if last_beat.elapsed() >= Duration::from_secs(15) {
            stream.write_all(b": heartbeat\n\n")?;
            stream.flush()?;
            last_beat = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
