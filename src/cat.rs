//! `lol.rb`: splits input into chunks and lines and paints them.

use std::{
    io::{self, Read, Write},
    thread,
    time::Duration,
};

use crate::{
    ColorMode,
    ansi::{has_incomplete_char, has_incomplete_escape, strip_erasing_sequences, tokens},
    color::{rainbow, write_color},
};

/// Written after each input when stdout is a terminal: resets colours, shows
/// the cursor and turns off the modes a misbehaving input may have left on.
pub const RESTORE_TERMINAL: &[u8] = b"\x1b[m\x1b[?25h\x1b[?1;5;2004l";

const CHUNK: usize = 4096;
const TAB: &[u8] = b"        ";

/// Rainbow settings. The defaults are those of the command line.
#[derive(Clone, Debug)]
pub struct Options {
    /// Characters per step along the rainbow.
    pub spread: f64,
    /// How quickly the colours change.
    pub freq: f64,
    /// Starting position in the rainbow.
    pub seed: f64,
    /// Repaint each line several times with a moving rainbow.
    pub animate: bool,
    /// Frames per line when animating.
    pub duration: u64,
    /// Frames per second when animating.
    pub speed: f64,
    /// Paint the background instead of the foreground.
    pub invert: bool,
    /// Colour sequence format.
    pub color: ColorMode,
    /// Write [`RESTORE_TERMINAL`] after each input.
    pub restore_terminal: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            spread: 3.0,
            freq: 0.1,
            seed: 0.0,
            animate: false,
            duration: 12,
            speed: 20.0,
            invert: false,
            color: ColorMode::Ansi256,
            restore_terminal: false,
        }
    }
}

/// Paints a series of inputs.
///
/// A single painter must see all of them, because an input with no final
/// newline changes where the colours of the next one go.
#[derive(Clone, Debug)]
pub struct Rainbow {
    options: Options,
    position: f64,
    resume_at: Option<f64>,
}

impl Rainbow {
    pub fn new(options: Options) -> Self {
        Self {
            position: options.seed,
            options,
            resume_at: None,
        }
    }

    /// Paints all of `input` to `output`, flushing as each chunk is finished.
    ///
    /// The rainbow restarts from the seed for each input.
    pub fn cat(&mut self, input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
        self.position = self.options.seed;
        let result = self.paint_input(input, output);
        if self.options.restore_terminal {
            // Any error from painting matters more than one from this.
            let restored = output
                .write_all(RESTORE_TERMINAL)
                .and_then(|()| output.flush());
            return result.and(restored);
        }
        result.and_then(|()| output.flush())
    }

    fn paint_input(&mut self, input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
        if self.options.animate {
            output.write_all(b"\x1b[?25l")?;
        }
        let mut chunk = [0; CHUNK];
        let mut buffer = Vec::new();
        loop {
            let count = match input.read(&mut chunk) {
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            buffer.extend_from_slice(&chunk[..count]);
            // Line and colour positions depend on where the chunks end, so
            // they have to end in the same places as upstream's do.
            if count > 0 && (has_incomplete_char(&buffer) || has_incomplete_escape(&buffer)) {
                continue;
            }
            for line in buffer.split_inclusive(|&byte| byte == b'\n') {
                self.position += 1.0;
                self.print_line(line, output)?;
            }
            buffer.clear();
            output.flush()?;
            if count == 0 {
                return Ok(());
            }
        }
    }

    fn print_line(&mut self, line: &[u8], output: &mut impl Write) -> io::Result<()> {
        let (text, ended) = match line.split_last() {
            Some((b'\n', text)) => (text, true),
            _ => (line, false),
        };
        let mut text = expand_tabs(text);
        if self.options.animate {
            self.print_animated(&mut text, ended, output)?;
        } else {
            self.print_plain(&text, ended, output)?;
        }
        if ended {
            output.write_all(b"\n")?;
        }
        Ok(())
    }

    fn print_plain(&mut self, text: &[u8], ended: bool, output: &mut impl Write) -> io::Result<()> {
        let options = &self.options;
        let reset: &[u8] = if options.invert {
            b"\x1b[49m"
        } else {
            b"\x1b[39m"
        };
        let mut count = 0;
        for (index, (escapes, character)) in tokens(text).enumerate() {
            let position = self.position + index as f64 / options.spread;
            output.write_all(escapes)?;
            write_color(
                output,
                rainbow(options.freq, position),
                options.color,
                options.invert,
            )?;
            output.write_all(character)?;
            output.write_all(reset)?;
            count += 1;
        }
        if !ended {
            self.resume_at = Some(self.position);
            self.position += count as f64 / options.spread;
        } else if let Some(position) = self.resume_at.take() {
            self.position = position;
        }
        Ok(())
    }

    fn print_animated(
        &mut self,
        text: &mut Vec<u8>,
        ended: bool,
        output: &mut impl Write,
    ) -> io::Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        output.write_all(b"\x1b7")?;
        let start = self.position;
        for _ in 0..self.options.duration {
            output.write_all(b"\x1b8")?;
            self.position += self.options.spread;
            self.print_plain(text, ended, output)?;
            strip_erasing_sequences(text);
            output.flush()?;
            thread::sleep(Duration::from_secs_f64(1.0 / self.options.speed));
        }
        self.position = start;
        Ok(())
    }
}

fn expand_tabs(text: &[u8]) -> Vec<u8> {
    let mut expanded = Vec::with_capacity(text.len());
    for part in text.split_inclusive(|&byte| byte == b'\t') {
        match part.split_last() {
            Some((b'\t', rest)) => {
                expanded.extend_from_slice(rest);
                expanded.extend_from_slice(TAB);
            }
            _ => expanded.extend_from_slice(part),
        }
    }
    expanded
}
