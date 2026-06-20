//! Minimal HTTP/1.1 primitives shared by both serve modes — request parsing
//! (headers *and* body, which `POST` needs), response writing, and the small
//! encoders the playground endpoints lean on. No async, no dependencies.

use std::io::{Read, Write};
use std::net::TcpStream;

/// One parsed request. `path` is split from its query string; `body` holds the
/// `Content-Length` bytes (empty for a `GET`).
pub(super) struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    pub body: Vec<u8>,
}

const READ_CHUNK: usize = 2048;
const MAX_HEADER: usize = 64 * 1024;

/// Read a whole request: consume up to the blank line that ends the headers,
/// then read `Content-Length` more bytes for the body.
pub(super) fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut buf = Vec::with_capacity(READ_CHUNK);
    let mut chunk = [0u8; READ_CHUNK];
    let header_end = loop {
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 || buf.len() > MAX_HEADER {
            break buf.len();
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    let head = String::from_utf8_lossy(&buf[..header_end.min(buf.len())]);
    let mut lines = head.lines();
    let mut start = lines.next().unwrap_or("").split_whitespace();
    let method = start.next().unwrap_or("").to_string();
    let raw_path = start.next().unwrap_or("");
    let (path, query) = match raw_path.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (raw_path.to_string(), String::new()),
    };

    let content_length = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = buf.get(header_end..).unwrap_or(&[]).to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    Ok(Request {
        method,
        path,
        query,
        body,
    })
}

/// Write one complete `Connection: close` response.
pub(super) fn write_response(
    stream: &mut TcpStream,
    code: u16,
    ctype: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let status = match code {
        200 => "200 OK",
        204 => "204 No Content",
        400 => "400 Bad Request",
        404 => "404 Not Found",
        _ => "500 Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

/// The value of one `key=value` pair in a query string, percent-decoded.
pub(super) fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

/// Decode `%XX` escapes and `+`-as-space. Invalid escapes pass through verbatim.
pub(super) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match (hex(b[i + 1]), hex(b[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Escape a string for embedding inside a JSON double-quoted value (no quotes
/// added). Enough for our short diagnostics and the SVG payload.
pub(super) fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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
    out
}

/// Escape for an HTML text/attribute context — used to splice the file title
/// into the single-file page.
pub(super) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escapes_quotes_slashes_and_controls() {
        assert_eq!(json_escape(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(json_escape("line\nbreak\ttab"), "line\\nbreak\\ttab");
        assert_eq!(json_escape("\u{1}"), "\\u0001");
        assert_eq!(json_escape("<svg/>"), "<svg/>"); // angle brackets are JSON-safe
    }

    #[test]
    fn percent_decode_handles_escapes_and_plus() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("sub%2Ffile.lini"), "sub/file.lini");
        assert_eq!(percent_decode("bad%zz"), "bad%zz"); // invalid escape, verbatim
    }

    #[test]
    fn query_param_picks_the_named_value() {
        assert_eq!(
            query_param("path=a.lini", "path").as_deref(),
            Some("a.lini")
        );
        assert_eq!(
            query_param("x=1&path=sub%2Fb.lini&y=2", "path").as_deref(),
            Some("sub/b.lini")
        );
        assert_eq!(query_param("x=1", "path"), None);
    }
}
