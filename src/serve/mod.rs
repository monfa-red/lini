//! Local dev-server for `lini serve FILE`. Hand-rolled HTTP/1.1 over std::net,
//! no async runtime, no extra deps.
//!
//! Three routes:
//! - `GET /`        → playground HTML page
//! - `GET /svg`     → compile FILE, return SVG (or plain-text error)
//! - `GET /events`  → Server-Sent Events stream pushing `reload` on file change
//!
//! A background thread polls the file's mtime/size every 200 ms and bumps a
//! generation counter when it changes. SSE handlers poll that counter and emit
//! `event: reload` on change. Heartbeat every 15 s keeps proxies happy.

use crate::Options;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

const PAGE_TEMPLATE: &str = include_str!("playground.html");

pub fn serve(file: PathBuf, port: u16, opts: Options) -> std::io::Result<()> {
    let state = Arc::new(State {
        file: file.clone(),
        opts,
        generation: Mutex::new(0u64),
    });

    let watcher_state = state.clone();
    thread::spawn(move || file_watcher(watcher_state));

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;
    eprintln!(
        "lini serve: {} → http://{}/  (Ctrl-C to stop)",
        file.display(),
        addr
    );

    for stream in listener.incoming() {
        let stream = stream?;
        let s = state.clone();
        thread::spawn(move || {
            if let Err(e) = handle(stream, s) {
                // Browsers close SSE streams on navigation; that's normal, not noise.
                if e.kind() != std::io::ErrorKind::BrokenPipe
                    && e.kind() != std::io::ErrorKind::ConnectionReset
                {
                    eprintln!("conn: {}", e);
                }
            }
        });
    }
    Ok(())
}

struct State {
    file: PathBuf,
    opts: Options,
    generation: Mutex<u64>,
}

fn file_watcher(state: Arc<State>) {
    let mut last: Option<(SystemTime, u64)> = None;
    loop {
        let sig = std::fs::metadata(&state.file)
            .and_then(|m| Ok((m.modified()?, m.len())))
            .ok();
        if sig != last {
            if last.is_some() {
                *state.generation.lock().unwrap() += 1;
            }
            last = sig;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn handle(mut stream: TcpStream, state: Arc<State>) -> std::io::Result<()> {
    let req_line = read_request_line(&mut stream)?;
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    match (method, path) {
        ("GET", "/") => serve_page(&mut stream, &state),
        ("GET", "/svg") => serve_svg(&mut stream, &state),
        ("GET", "/events") => serve_events(&mut stream, &state),
        ("GET", "/favicon.ico") => write_response(&mut stream, 204, "text/plain", b""),
        _ => write_response(&mut stream, 404, "text/plain", b"not found\n"),
    }
}

fn read_request_line(stream: &mut TcpStream) -> std::io::Result<String> {
    // Read just enough to get the request line + headers. We don't care about
    // the body — these are all GETs.
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf)?;
    let s = String::from_utf8_lossy(&buf[..n]);
    Ok(s.lines().next().unwrap_or("").to_string())
}

fn serve_page(stream: &mut TcpStream, state: &State) -> std::io::Result<()> {
    let title = state.file.display().to_string();
    let html = PAGE_TEMPLATE.replace("{{TITLE}}", &html_escape(&title));
    write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes())
}

fn serve_svg(stream: &mut TcpStream, state: &State) -> std::io::Result<()> {
    let src = match std::fs::read_to_string(&state.file) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("read error: {}", e);
            return write_response(stream, 200, "text/plain; charset=utf-8", msg.as_bytes());
        }
    };
    match crate::compile_str_with(&src, &state.opts) {
        Ok(svg) => write_response(stream, 200, "image/svg+xml", svg.as_bytes()),
        Err(e) => {
            let filename = state.file.display().to_string();
            let msg = e.display_with_source(&src, &filename).to_string();
            write_response(stream, 200, "text/plain; charset=utf-8", msg.as_bytes())
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
    let mut last_heartbeat = Instant::now();
    loop {
        let current = *state.generation.lock().unwrap();
        if current != last_gen {
            last_gen = current;
            stream.write_all(b"event: reload\ndata: 1\n\n")?;
            stream.flush()?;
        }
        if last_heartbeat.elapsed() >= Duration::from_secs(15) {
            stream.write_all(b": heartbeat\n\n")?;
            stream.flush()?;
            last_heartbeat = Instant::now();
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn write_response(
    stream: &mut TcpStream,
    code: u16,
    ctype: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let status = match code {
        200 => "200 OK",
        204 => "204 No Content",
        404 => "404 Not Found",
        _ => "500 Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        ctype,
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
