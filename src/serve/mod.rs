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

fn handle(mut stream: TcpStream, state: &State) -> std::io::Result<()> {
    let req = http::read_request(&mut stream)?;
    match state.target {
        ServeTarget::File(_) => file_mode::handle(&mut stream, &req, state),
        ServeTarget::Dir(_) => dir_mode::handle(&mut stream, &req, state),
    }
}
