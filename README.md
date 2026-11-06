# lolcat

A Rust rewrite of [lolcat](https://github.com/busyloop/lolcat), the rainbow
`cat`. Its output matches the Ruby original byte for byte, apart from the
differences listed below, and has no dependencies. Available as a command-line
tool and Rust library.

## Usage

```sh
cargo install --path .

fortune | lolcat
lolcat f - g
lolcat --animate --truecolor file.txt
lolcat --help
```

Reads stdin when no file is given or when a file is `-`. Colour is only added
when stdout is a terminal, unless you pass `--force`. Truecolor is used with
`--truecolor` or when `COLORTERM` is `truecolor` or `24bit`.

For Rust projects:

```rust
let mut rainbow = lolcat::Rainbow::new(lolcat::Options::default());
rainbow.cat(&mut std::io::stdin().lock(), &mut std::io::stdout().lock())?;
```

## Differences from the Ruby version

- Invalid UTF-8 and escape sequences left unfinished at the end of the input
  are painted. The Ruby version silently discards the whole buffer holding them,
  which can be everything it read.
- Input is always treated as UTF-8. The Ruby version uses the locale's encoding,
  so under `LC_ALL=C` it prints nothing for non-ASCII input.
- Mistyped options get no "Did you mean" suggestion.
- `LOLCAT_DEBUG` isn't supported.
- Only Ctrl-C restores the terminal. The Ruby version also does so on `SIGTERM`
  and exits by the signal.
- `--version` prints this port's version, and the bug report and home page
  addresses in `--help` point to this repository.

## Testing

`tests/golden` holds output recorded from the real Ruby lolcat, so `just check`
doesn't need Ruby. With it installed, `scripts/oracle.py` re-records those cases
(`record`), compares the two on random input (`fuzz`), and compares them on a
terminal (`pty`).

## License

BSD 3-Clause, the same as the original; see [LICENSE](LICENSE). Parts follow MIT
licensed Ruby libraries; see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
