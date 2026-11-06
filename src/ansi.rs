//! Hand-written equivalents of the two regular expressions in `lol.rb`.
//!
//! Everything works on bytes. Every byte the patterns name is ASCII, so it can
//! never appear inside a multi-byte UTF-8 character.

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

fn is_intermediate(byte: u8) -> bool {
    (0x20..=0x2f).contains(&byte)
}

fn is_parameter(byte: u8) -> bool {
    (0x30..=0x3f).contains(&byte)
}

fn is_string_intro(byte: u8) -> bool {
    matches!(byte, b']' | b'P' | b'X' | b'^' | b'_')
}

fn run_end(bytes: &[u8], start: usize, class: impl Fn(u8) -> bool) -> usize {
    bytes[start..]
        .iter()
        .position(|&byte| !class(byte))
        .map_or(bytes.len(), |offset| start + offset)
}

/// Length of the character at `at`, which is one byte if it isn't valid UTF-8.
fn char_len(bytes: &[u8], at: usize) -> usize {
    let width = match bytes[at] {
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return 1,
    };
    match bytes.get(at..at + width).map(std::str::from_utf8) {
        Some(Ok(_)) => width,
        _ => 1,
    }
}

/// One iteration of the group in `ANSI_ESCAPE`, starting just after an ESC:
///
/// ```text
/// [ -\/]+.  |  []PX^_][^\a\e]*  |  \[[0-?]*.  |  .
/// ```
///
/// A run that reaches the end of the line gives back its last byte to satisfy
/// `.`, which is the only backtracking this pattern can do.
fn escape_end(bytes: &[u8], start: usize) -> Option<usize> {
    let first = *bytes.get(start)?;
    if is_intermediate(first) {
        let end = run_end(bytes, start, is_intermediate);
        if end < bytes.len() {
            return Some(end + char_len(bytes, end));
        }
        if end - start >= 2 {
            return Some(end);
        }
    } else if is_string_intro(first) {
        return Some(run_end(bytes, start + 1, |byte| byte != BEL && byte != ESC));
    } else if first == b'[' {
        let end = run_end(bytes, start + 1, is_parameter);
        if end < bytes.len() {
            return Some(end + char_len(bytes, end));
        }
        if end > start + 1 {
            return Some(end);
        }
    }
    Some(start + char_len(bytes, start))
}

/// Iterates over what `line.scan(ANSI_ESCAPE)` yields: any escape sequences,
/// then the single visible character they precede.
///
/// Like `scan`, this always ends with one empty match. It gets painted too, so
/// it has to be here for the output and the colour position to come out equal.
pub(crate) struct Tokens<'a> {
    bytes: &'a [u8],
    at: usize,
    done: bool,
}

pub(crate) fn tokens(bytes: &[u8]) -> Tokens<'_> {
    Tokens {
        bytes,
        at: 0,
        done: false,
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = (&'a [u8], &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let start = self.at;
        let mut end = start;
        while self.bytes.get(end) == Some(&ESC) {
            match escape_end(self.bytes, end + 1) {
                Some(next) => end = next,
                None => break,
            }
        }
        let char_end = if end < self.bytes.len() {
            end + char_len(self.bytes, end)
        } else {
            end
        };
        self.at = char_end;
        // Only the match at the very end of the line can be empty.
        self.done = start == char_end;
        Some((&self.bytes[start..end], &self.bytes[end..char_end]))
    }
}

fn at_line_end(bytes: &[u8], at: usize) -> bool {
    bytes.get(at).is_none_or(|&byte| byte == b'\n')
}

/// `INCOMPLETE_ESCAPE`:
///
/// ```text
/// \e(?:[ -\/]*|[\]PX^_][^\a\e]*|\[[0-?]*)$
/// ```
///
/// Ruby's `$` matches before any newline, not just at the end of the string, so
/// this can match in the middle of `bytes`.
pub(crate) fn has_incomplete_escape(bytes: &[u8]) -> bool {
    bytes.iter().enumerate().any(|(index, &byte)| {
        if byte != ESC {
            return false;
        }
        let start = index + 1;
        if at_line_end(bytes, run_end(bytes, start, is_intermediate)) {
            return true;
        }
        match bytes.get(start) {
            Some(&intro) if is_string_intro(intro) => {
                let end = run_end(bytes, start + 1, |byte| byte != BEL && byte != ESC);
                end == bytes.len() || bytes[start + 1..end].contains(&b'\n')
            }
            Some(b'[') => at_line_end(bytes, run_end(bytes, start + 1, is_parameter)),
            _ => false,
        }
    })
}

/// Whether `bytes` stops part way through a UTF-8 character.
pub(crate) fn has_incomplete_char(bytes: &[u8]) -> bool {
    (bytes.len().saturating_sub(3)..bytes.len()).any(|start| {
        bytes[start] >= 0xc0
            && std::str::from_utf8(&bytes[start..]).is_err_and(|error| error.error_len().is_none())
    })
}

/// `str.gsub!(/\e\[[0-?]*[@JKPX]/, "")`: strips the sequences that would erase
/// text, which animation frames must not repeat.
pub(crate) fn strip_erasing_sequences(line: &mut Vec<u8>) {
    let mut kept = Vec::with_capacity(line.len());
    let mut at = 0;
    while at < line.len() {
        if line[at] == ESC && line.get(at + 1) == Some(&b'[') {
            let end = run_end(line, at + 2, is_parameter);
            if matches!(line.get(end), Some(b'@' | b'J' | b'K' | b'P' | b'X')) {
                at = end + 1;
                continue;
            }
        }
        kept.push(line[at]);
        at += 1;
    }
    *line = kept;
}
