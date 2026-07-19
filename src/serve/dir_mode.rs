//! Playground mode: `lini serve [DIR]`. Serves an editor + live render over a
//! directory's `.lini` files.
//!
//! - `GET /`              → the playground page
//! - `GET /api/files`     → newline-delimited `*.lini` paths under DIR
//! - `GET /api/file?path` → one file's text
//! - `POST /compile`      → compile the posted source → JSON `{ok, svg?, error?, diagnostics}`
//! - `POST /save?path`    → overwrite an existing listed file with the posted text
//!
//! Every caller-supplied path goes through [`resolve_in_root`], so a request can
//! only ever read or write a `.lini` file inside DIR — never escape it.

use super::{ServeTarget, State, http};
use crate::Options;
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};

const PAGE: &str = include_str!("playground.html");

fn root(state: &State) -> &Path {
    match &state.target {
        ServeTarget::Dir(d) => d,
        ServeTarget::File(_) => unreachable!("dir_mode runs only for a Dir target"),
    }
}

pub(super) fn handle(
    stream: &mut TcpStream,
    req: &http::Request,
    state: &State,
) -> std::io::Result<()> {
    let root = root(state);
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => {
            http::write_response(stream, 200, "text/html; charset=utf-8", PAGE.as_bytes())
        }
        ("GET", "/api/files") => serve_list(stream, root),
        ("GET", "/api/file") => serve_file(stream, root, &req.query),
        ("POST", "/compile") => serve_compile(stream, req, state),
        ("POST", "/save") => serve_save(stream, root, &req.query, &req.body),
        ("GET", "/favicon.ico") => http::write_response(stream, 204, "text/plain", b""),
        _ => http::write_response(stream, 404, "text/plain", b"not found\n"),
    }
}

fn serve_list(stream: &mut TcpStream, root: &Path) -> std::io::Result<()> {
    let body = lini_files(root).join("\n");
    http::write_response(stream, 200, "text/plain; charset=utf-8", body.as_bytes())
}

fn serve_file(stream: &mut TcpStream, root: &Path, query: &str) -> std::io::Result<()> {
    let Some(rel) = http::query_param(query, "path") else {
        return http::write_response(stream, 400, "text/plain", b"missing path\n");
    };
    let Some(path) = resolve_in_root(root, &rel) else {
        return http::write_response(stream, 400, "text/plain", b"invalid path\n");
    };
    match std::fs::read(&path) {
        Ok(bytes) => http::write_response(stream, 200, content_type(&path), &bytes),
        Err(_) => http::write_response(stream, 404, "text/plain", b"not found\n"),
    }
}

/// The served content type by extension. Reads are not limited to `.lini` —
/// the directory boundary is the security wall, and a scene will reference
/// sibling images (`|image| { src: }`) once serve renders them (alpha.5).
fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("lini") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn serve_compile(
    stream: &mut TcpStream,
    req: &http::Request,
    state: &State,
) -> std::io::Result<()> {
    let root = root(state);
    // Image assets resolve from the compiling file's directory, confined to
    // the served root [SPEC 7/19] — the same boundary as the file list. An
    // unsaved buffer (no `path`) anchors at the root itself.
    let base_dir = http::query_param(&req.query, "path")
        .and_then(|rel| resolve_in_root(root, &rel))
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| root.to_path_buf());
    let opts = Options {
        base_dir: Some(base_dir),
        asset_root: Some(root.to_path_buf()),
        ..state.opts.clone()
    };
    let opts = &opts;
    let src = String::from_utf8_lossy(&req.body);
    let name = "playground.lini";
    let lints = crate::lint_str(&src).unwrap_or_default();
    // Best-effort extras, present whenever the source parses: render also
    // formats the buffer, and the desugar pane shows the expanded form.
    let formatted = opt_json(crate::format_source(&src).ok());
    let desugared = opt_json(crate::desugar_source(&src).ok());

    let json = match crate::compile_str_checked(&src, opts) {
        Ok((svg, route_diags)) => {
            let diags: Vec<String> = lints
                .iter()
                .chain(route_diags.iter())
                .map(|d| d.display_with_source(&src, name).to_string())
                .collect();
            format!(
                "{{\"ok\":true,\"svg\":\"{}\",\"diagnostics\":{},\"formatted\":{},\"desugared\":{}}}",
                http::json_escape(&svg),
                json_string_array(&diags),
                formatted,
                desugared
            )
        }
        Err(e) => format!(
            "{{\"ok\":false,\"error\":\"{}\",\"diagnostics\":[],\"formatted\":{},\"desugared\":{}}}",
            http::json_escape(&e.display_with_source(&src, name).to_string()),
            formatted,
            desugared
        ),
    };
    http::write_response(
        stream,
        200,
        "application/json; charset=utf-8",
        json.as_bytes(),
    )
}

fn serve_save(
    stream: &mut TcpStream,
    root: &Path,
    query: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let Some(rel) = http::query_param(query, "path") else {
        return err_json(stream, 400, "missing path");
    };
    // `resolve_in_root` only resolves files that already exist, so Save can
    // only ever overwrite a listed file — never create one (out of scope for
    // v1) — and writes stay `.lini`-only (reads are the generalized side).
    let Some(path) = resolve_in_root(root, &rel) else {
        return err_json(stream, 400, "invalid path");
    };
    if path.extension().and_then(|e| e.to_str()) != Some("lini") {
        return err_json(stream, 400, "invalid path");
    }
    match std::fs::write(&path, body) {
        Ok(()) => http::write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            b"{\"ok\":true}",
        ),
        Err(e) => err_json(stream, 500, &format!("write failed: {e}")),
    }
}

fn err_json(stream: &mut TcpStream, code: u16, msg: &str) -> std::io::Result<()> {
    let json = format!("{{\"ok\":false,\"error\":\"{}\"}}", http::json_escape(msg));
    http::write_response(
        stream,
        code,
        "application/json; charset=utf-8",
        json.as_bytes(),
    )
}

fn json_string_array(items: &[String]) -> String {
    let inner: Vec<String> = items
        .iter()
        .map(|s| format!("\"{}\"", http::json_escape(s)))
        .collect();
    format!("[{}]", inner.join(","))
}

/// A JSON string literal, or `null` when the value is absent.
fn opt_json(s: Option<String>) -> String {
    match s {
        Some(v) => format!("\"{}\"", http::json_escape(&v)),
        None => "null".to_string(),
    }
}

/// Every `.lini` file under `root`, as `/`-separated paths relative to it,
/// sorted. Hidden directories (`.git`, …) are skipped.
fn lini_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let hidden = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'));
            if !hidden {
                walk(root, &path, out);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("lini")
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel_to_slash(rel));
        }
    }
}

fn rel_to_slash(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// Resolve a caller-supplied relative path against `root`, returning the
/// canonical path only if it genuinely lives inside `root`. Rejects absolute
/// paths, `..` traversal, and (via canonicalization) symlink escapes — the
/// directory is the boundary; what may be *written* is the save handler's
/// stricter call.
fn resolve_in_root(root: &Path, rel: &str) -> Option<PathBuf> {
    let decoded = http::percent_decode(rel);
    let rel = Path::new(&decoded);
    if rel.is_absolute() || rel.components().any(|c| c == Component::ParentDir) {
        return None;
    }
    let canon_root = root.canonicalize().ok()?;
    let canon = root.join(rel).canonicalize().ok()?;
    canon.starts_with(&canon_root).then_some(canon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_samples_sorted_and_slashed() {
        let files = lini_files(Path::new("samples"));
        assert!(files.contains(&"hello.lini".to_string()));
        assert!(files.iter().all(|f| f.ends_with(".lini")));
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
    }

    #[test]
    fn resolve_accepts_in_root_files_assets_included() {
        // The boundary generalized past `.lini` [SPEC 19]: served assets
        // (a scene's images) resolve under the same wall.
        assert!(resolve_in_root(Path::new("samples"), "hello.lini").is_some());
        assert!(resolve_in_root(Path::new("samples"), "assets/logo.svg").is_some());
    }

    #[test]
    fn resolve_rejects_traversal_absolute_and_missing() {
        let root = Path::new("samples");
        assert!(resolve_in_root(root, "../Cargo.toml").is_none()); // traversal
        assert!(resolve_in_root(root, "../samples/hello.lini").is_none()); // traversal even if it lands back inside
        assert!(resolve_in_root(root, "/etc/hosts").is_none()); // absolute
        assert!(resolve_in_root(root, "nope.lini").is_none()); // does not exist
    }
}
