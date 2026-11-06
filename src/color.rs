//! The rainbow function and the colour sequences it turns into.

use std::{
    f64::consts::PI,
    io::{self, Write},
};

/// How colours are written to the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// `38;5;n`, the nearest colour in the xterm 256-colour palette.
    Ansi256,
    /// `38;2;r;g;b`, exact 24-bit colour.
    TrueColor,
}

/// The colour at position `i` of a rainbow with frequency `freq`.
///
/// Channels truncate rather than round, as Ruby's `%02X` does for a float.
pub fn rainbow(freq: f64, i: f64) -> [u8; 3] {
    let channel = |phase: f64| ((freq * i + phase).sin() * 127.0 + 128.0) as u8;
    [
        channel(0.0),
        channel(2.0 * PI / 3.0),
        channel(4.0 * PI / 3.0),
    ]
}

/// Paint's `rgb_to_256`: a grey ramp index when all three channels fall in the
/// same band below the first colour step, otherwise a 6x6x6 cube index.
fn to_256([red, green, blue]: [u8; 3]) -> u16 {
    let (r, g, b) = (f64::from(red), f64::from(green), f64::from(blue));
    let mut sep = 42.5;
    let grey = loop {
        if r < sep || g < sep || b < sep {
            break r < sep && g < sep && b < sep;
        }
        sep += 42.5;
    };
    if grey {
        232 + ((r + g + b) / 33.0).round() as u16
    } else {
        let step = |channel: f64| (6.0 * (channel / 256.0)) as u16;
        16 + step(r) * 36 + step(g) * 6 + step(b)
    }
}

pub(crate) fn write_color(
    output: &mut impl Write,
    rgb: [u8; 3],
    mode: ColorMode,
    background: bool,
) -> io::Result<()> {
    let target = if background { 48 } else { 38 };
    match mode {
        ColorMode::TrueColor => {
            let [r, g, b] = rgb;
            write!(output, "\x1b[{target};2;{r};{g};{b}m")
        }
        ColorMode::Ansi256 => write!(output, "\x1b[{target};5;{}m", to_256(rgb)),
    }
}
