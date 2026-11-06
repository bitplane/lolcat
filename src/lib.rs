//! A byte-compatible Rust rewrite of [lolcat](https://github.com/busyloop/lolcat).
//!
//! ```
//! use lolcat::{ColorMode, Options, Rainbow};
//!
//! let mut output = Vec::new();
//! let options = Options { seed: 1.0, color: ColorMode::TrueColor, ..Options::default() };
//! Rainbow::new(options).cat(&mut &b"hi\n"[..], &mut output)?;
//! assert!(output.starts_with(b"\x1b[38;2;153;223;7mh\x1b[39m"));
//! # Ok::<(), std::io::Error>(())
//! ```

mod ansi;
mod cat;
mod color;

pub use cat::{Options, RESTORE_TERMINAL, Rainbow};
pub use color::{ColorMode, rainbow};

#[cfg(test)]
mod tests;
