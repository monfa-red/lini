//! Source trivia — comments and blank lines — scanned straight from the bytes
//! (the lexer drops them). The formatter replays them between AST items so a
//! `// note` and a blank-line grouping survive a round-trip. A run of two or
//! more blank lines collapses to one.

#[derive(Debug, Clone)]
pub enum Trivia {
    Comment(String),
    BlankLine,
}

#[derive(Debug, Clone)]
pub struct TriviaToken {
    pub pos: usize,
    pub kind: Trivia,
}

pub fn scan_trivia(src: &str) -> Vec<TriviaToken> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut at_line_start = true;
    let mut blank_run = 0usize;

    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                if at_line_start {
                    blank_run += 1;
                    if blank_run == 2 {
                        out.push(TriviaToken {
                            pos: i,
                            kind: Trivia::BlankLine,
                        });
                    }
                } else {
                    blank_run = 1;
                }
                at_line_start = true;
                i += 1;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let start = i;
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                let text = src[start..i].trim_end().to_string();
                out.push(TriviaToken {
                    pos: start,
                    kind: Trivia::Comment(text),
                });
                at_line_start = false;
                blank_run = 0;
            }
            _ => {
                at_line_start = false;
                blank_run = 0;
                if c == b'"' {
                    i += 1;
                    while i < bytes.len() {
                        let cc = bytes[i];
                        if cc == b'\\' && i + 1 < bytes.len() {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        if cc == b'"' {
                            break;
                        }
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
    out
}
