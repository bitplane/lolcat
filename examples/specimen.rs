//! Prints a colour specimen, which the release check keeps as an artifact.

use std::io::{self, Write};

use lolcat::{ColorMode, Options, Rainbow};

fn main() -> io::Result<()> {
    let text = "The quick brown fox jumps over the lazy dog.\n".repeat(12);
    let mut output = io::stdout().lock();
    for (color, invert) in [
        (ColorMode::Ansi256, false),
        (ColorMode::TrueColor, false),
        (ColorMode::TrueColor, true),
    ] {
        let options = Options {
            seed: 1.0,
            color,
            invert,
            ..Options::default()
        };
        Rainbow::new(options).cat(&mut text.as_bytes(), &mut output)?;
    }
    output.flush()
}
