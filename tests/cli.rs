//! Behaviour that can't be recorded from upstream: random output, our own
//! version, and the places where this port deliberately differs.

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn lolcat(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lolcat"))
        .args(args)
        .env_remove("COLORTERM")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn strip_colors(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut plain = String::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find('\x1b') {
        plain.push_str(&rest[..start]);
        let end = rest[start..].find('m').unwrap();
        rest = &rest[start + end + 1..];
    }
    plain + rest
}

#[test]
fn version_prints_our_version_and_keeps_the_credit() {
    for flag in ["-v", "--version", "--no-version"] {
        let output = lolcat(&[flag], b"");
        assert!(output.status.success());
        let expected = format!(
            "lolcat {} (c)2011 moe@busyloop.net\n",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn version_wins_over_an_invalid_value() {
    assert!(lolcat(&["-v", "-p", "abc"], b"").status.success());
}

#[test]
fn help_is_rainbow_coloured_and_exits_one() {
    let output = lolcat(&["--help"], b"");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.starts_with(b"\x1b[38;5;"));
    let text = strip_colors(&output.stdout);
    assert!(text.starts_with("\nUsage: lolcat [OPTION]... [FILE]...\n"));
    assert!(text.contains("  -p, --spread=<f>      Rainbow spread (default: 3.0)\n"));
    assert!(text.ends_with("<http://speaklolcat.com/>\n\n\n"));
}

#[test]
fn colorterm_selects_truecolor() {
    for value in ["truecolor", "24bit"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lolcat"))
            .args(["-f", "-S", "1"])
            .env("COLORTERM", value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"a\n").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.stdout.starts_with(b"\x1b[38;2;"));
    }
}

#[test]
fn random_seed_still_paints_every_character() {
    let output = lolcat(&["-f"], b"hello\n");
    assert_eq!(strip_colors(&output.stdout), "hello\n");
}

// Upstream prints nothing at all for each of these inputs: it waits for the
// sequence to finish, reaches the end of the input and discards its buffer.
#[test]
fn deviation_invalid_utf8_is_not_lost() {
    let output = lolcat(&["-f", "-S", "1"], b"ok\xff bad\n");
    let plain: Vec<u8> = output
        .stdout
        .split(|&byte| byte == 0x1b)
        .flat_map(|part| match part.iter().position(|&byte| byte == b'm') {
            Some(end) => part[end + 1..].to_vec(),
            None => part.to_vec(),
        })
        .collect();
    assert_eq!(plain, b"ok\xff bad\n");
}

#[test]
fn deviation_unfinished_escape_is_not_lost() {
    let output = lolcat(&["-f", "-S", "1"], b"abc\x1b[");
    assert!(output.stdout.windows(2).any(|pair| pair == b"\x1b["));
    assert!(output.stdout.contains(&b'a') && output.stdout.contains(&b'c'));
    let output = lolcat(&["-f", "-S", "1"], b"one\ntwo\x1b[\nthree\n");
    assert!(output.stdout.contains(&b'e'));
    assert!(output.stdout.ends_with(b"\x1b[39m\n"));
}

#[test]
fn closed_stdout_exits_one_without_a_message() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lolcat"))
        .args(["-f", "-S", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    // The child may exit before all of this is written.
    let _ = child.stdin.take().unwrap().write_all(&[b'x'; 1 << 20]);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
}
