use crate::{
    ColorMode, Options, Rainbow,
    ansi::{has_incomplete_char, has_incomplete_escape, strip_erasing_sequences, tokens},
    color::write_color,
    rainbow,
};

fn scan(line: &[u8]) -> Vec<(&[u8], &[u8])> {
    tokens(line).collect()
}

/// Expected values are what `line.scan(Lol::ANSI_ESCAPE)` gives in Ruby.
#[test]
fn tokens_match_ruby_scan() {
    type Token = (&'static [u8], &'static [u8]);
    let cases: &[(&[u8], &[Token])] = &[
        (b"", &[(b"", b"")]),
        (b"ab", &[(b"", b"a"), (b"", b"b"), (b"", b"")]),
        ("é".as_bytes(), &[(b"", "é".as_bytes()), (b"", b"")]),
        (b"\x1b[0mx", &[(b"\x1b[0m", b"x"), (b"", b"")]),
        (b"\x1b[0m", &[(b"\x1b[0m", b""), (b"", b"")]),
        (b"\x1b", &[(b"", b"\x1b"), (b"", b"")]),
        (b"\x1b[", &[(b"\x1b[", b""), (b"", b"")]),
        (b"\x1b[12", &[(b"\x1b[12", b""), (b"", b"")]),
        (b"\x1b ", &[(b"\x1b ", b""), (b"", b"")]),
        (b"\x1b  ", &[(b"\x1b  ", b""), (b"", b"")]),
        (b"\x1b(Bx", &[(b"\x1b(B", b"x"), (b"", b"")]),
        (
            b"\x1b]0;t\x07x",
            &[(b"\x1b]0;t", b"\x07"), (b"", b"x"), (b"", b"")],
        ),
        (b"\x1b]0;t\x1b\\x", &[(b"\x1b]0;t\x1b\\", b"x"), (b"", b"")]),
        (b"\x1b\x1bx", &[(b"\x1b\x1b", b"x"), (b"", b"")]),
    ];
    for (line, expected) in cases {
        assert_eq!(&scan(line), expected, "{line:?}");
    }
}

#[test]
fn invalid_bytes_are_single_characters() {
    assert_eq!(
        scan(b"\xc3x\xff"),
        [
            (&b""[..], &b"\xc3"[..]),
            (b"", b"x"),
            (b"", b"\xff"),
            (b"", b"")
        ]
    );
}

#[test]
fn incomplete_escape_uses_rubys_multiline_end_anchor() {
    for bytes in [
        &b"\x1b"[..],
        b"\x1b ",
        b"\x1b[",
        b"\x1b[12",
        b"\x1b]0;title",
        b"x\x1b[\nmore",
        b"\x1b\nmore",
        b"\x1b]0;a\nb\x07",
    ] {
        assert!(has_incomplete_escape(bytes), "{bytes:?}");
    }
    for bytes in [
        &b""[..],
        b"plain\n",
        b"\x1b[0m",
        b"\x1b]0;title\x07",
        b"\x1bc",
        b"\x1b(B",
    ] {
        assert!(!has_incomplete_escape(bytes), "{bytes:?}");
    }
}

#[test]
fn incomplete_char_only_looks_at_the_tail() {
    assert!(has_incomplete_char(b"x\xc3"));
    assert!(has_incomplete_char(b"\xe6\x97"));
    assert!(has_incomplete_char(b"\xf0\x9f\x8c"));
    assert!(!has_incomplete_char("é".as_bytes()));
    assert!(!has_incomplete_char(b"\xff"));
    assert!(!has_incomplete_char(b"\xc3x"));
    assert!(!has_incomplete_char(b""));
}

#[test]
fn erasing_sequences_are_stripped() {
    let mut line = b"a\x1b[Kb\x1b[2Jc\x1b[1;31md\x1b[x".to_vec();
    strip_erasing_sequences(&mut line);
    assert_eq!(line, b"abc\x1b[1;31md\x1b[x");
}

#[test]
fn colour_formats() {
    let render = |rgb, mode, background| {
        let mut output = Vec::new();
        write_color(&mut output, rgb, mode, background).unwrap();
        String::from_utf8(output).unwrap()
    };
    assert_eq!(
        render([1, 2, 3], ColorMode::TrueColor, false),
        "\x1b[38;2;1;2;3m"
    );
    assert_eq!(
        render([1, 2, 3], ColorMode::TrueColor, true),
        "\x1b[48;2;1;2;3m"
    );
    // From Paint: grey ramp, cube, and white, which lands on the grey ramp.
    assert_eq!(
        render([10, 10, 10], ColorMode::Ansi256, false),
        "\x1b[38;5;233m"
    );
    assert_eq!(
        render([255, 0, 0], ColorMode::Ansi256, true),
        "\x1b[48;5;196m"
    );
    assert_eq!(
        render([255, 255, 255], ColorMode::Ansi256, false),
        "\x1b[38;5;255m"
    );
}

#[test]
fn rainbow_truncates_channels() {
    // Ruby: Lol.rainbow(0.1, 1) => "#8CE70C"
    assert_eq!(rainbow(0.1, 1.0), [0x8c, 0xe7, 0x0c]);
}

#[test]
fn state_carries_between_inputs() {
    let options = Options {
        seed: 1.0,
        color: ColorMode::TrueColor,
        ..Options::default()
    };
    let paint = |inputs: &[&[u8]]| {
        let mut rainbow = Rainbow::new(options.clone());
        let mut output = Vec::new();
        for input in inputs {
            rainbow.cat(&mut &input[..], &mut output).unwrap();
        }
        output
    };
    let mut twice = paint(&[b"a\n"]);
    twice.extend(paint(&[b"a\n"]));
    assert_eq!(paint(&[b"a\n", b"a\n"]), twice, "each input restarts");
    let mut fresh = paint(&[b"x\ny"]);
    fresh.extend(paint(&[b"a\nb\n"]));
    assert_ne!(
        paint(&[b"x\ny", b"a\nb\n"]),
        fresh,
        "a missing newline carries over"
    );
}
