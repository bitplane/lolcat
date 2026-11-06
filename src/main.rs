use std::{
    collections::hash_map::RandomState,
    env,
    ffi::OsString,
    fs::File,
    hash::{BuildHasher, Hasher},
    io::{self, BufWriter, Cursor, IsTerminal, Read, Write},
    process::ExitCode,
};

use lolcat::{ColorMode, Options, Rainbow};

const HELP: &str = "
Usage: lolcat [OPTION]... [FILE]...

Concatenate FILE(s), or standard input, to standard output.
With no FILE, or when FILE is -, read standard input.

  -p, --spread=<f>      Rainbow spread (default: 3.0)
  -F, --freq=<f>        Rainbow frequency (default: 0.1)
  -S, --seed=<i>        Rainbow seed, 0 = random (default: 0)
  -a, --animate         Enable psychedelics
  -d, --duration=<i>    Animation duration (default: 12)
  -s, --speed=<f>       Animation speed (default: 20.0)
  -i, --invert          Invert fg and bg
  -t, --truecolor       24-bit (truecolor)
  -f, --force           Force color even when stdout is not a tty
  -v, --version         Print version and exit
  -h, --help            Show this message

Examples:
  lolcat f - g      Output f's contents, then stdin, then g's contents.
  lolcat            Copy standard input to standard output.
  fortune | lolcat  Display a rainbow cookie.

Report lolcat bugs to <https://github.com/bitplane/lolcat/issues>
lolcat home page: <https://github.com/bitplane/lolcat/>
Report lolcat translation bugs to <http://speaklolcat.com/>

";

#[derive(Clone, Copy, PartialEq)]
enum Opt {
    Spread,
    Freq,
    Seed,
    Animate,
    Duration,
    Speed,
    Invert,
    Truecolor,
    Force,
    Version,
    Help,
}

const OPTIONS: [(Opt, &str, char); 11] = [
    (Opt::Spread, "spread", 'p'),
    (Opt::Freq, "freq", 'F'),
    (Opt::Seed, "seed", 'S'),
    (Opt::Animate, "animate", 'a'),
    (Opt::Duration, "duration", 'd'),
    (Opt::Speed, "speed", 's'),
    (Opt::Invert, "invert", 'i'),
    (Opt::Truecolor, "truecolor", 't'),
    (Opt::Force, "force", 'f'),
    (Opt::Version, "version", 'v'),
    (Opt::Help, "help", 'h'),
];

impl Opt {
    fn takes_value(self) -> bool {
        matches!(
            self,
            Opt::Spread | Opt::Freq | Opt::Seed | Opt::Duration | Opt::Speed
        )
    }

    fn long(self) -> &'static str {
        OPTIONS
            .iter()
            .find(|(opt, ..)| *opt == self)
            .map_or("", |(_, long, _)| long)
    }
}

/// One option from the command line, in the state Optimist's `given_args` has
/// it before values are checked.
struct Given {
    opt: Opt,
    negated: bool,
    param: Option<String>,
}

#[derive(Default)]
struct Parsed {
    given: Vec<Given>,
    files: Vec<OsString>,
}

impl Parsed {
    /// Optimist's `each_arg` callback: resolves `arg` and records it, taking
    /// the first of `params` if the option has a value. Returns if it did.
    fn record(&mut self, arg: &str, param: Option<&str>) -> Result<bool, String> {
        let (arg, negated) = match arg.strip_prefix("--no-") {
            Some(rest) if !rest.is_empty() && !rest.starts_with('-') => (format!("--{rest}"), true),
            _ => (arg.to_owned(), false),
        };
        let opt = if let Some(long) = arg.strip_prefix("--") {
            if long.is_empty() || long.starts_with('-') {
                return Err(format!("invalid argument syntax: '{arg}'"));
            }
            OPTIONS.iter().find(|(_, name, _)| *name == long)
        } else {
            let mut letters = arg[1..].chars();
            match (letters.next(), letters.next()) {
                (Some(letter), None) => OPTIONS.iter().find(|(.., short)| *short == letter),
                _ => return Err(format!("invalid argument syntax: '{arg}'")),
            }
        };
        let opt = match opt {
            Some((opt, ..)) if !arg.starts_with("--no-") => *opt,
            _ => return Err(format!("unknown argument '{arg}'")),
        };
        if self.given.iter().any(|given| given.opt == opt) {
            return Err(format!("option '{arg}' specified multiple times"));
        }
        let param = param.filter(|_| opt.takes_value()).map(str::to_owned);
        let took_param = param.is_some();
        self.given.push(Given {
            opt,
            negated,
            param,
        });
        Ok(took_param)
    }
}

/// Optimist's `PARAM_RE`: `^-(-|\.$|[^\d\.])`. A negative number isn't a flag.
fn looks_like_flag(arg: &str) -> bool {
    let Some(rest) = arg.strip_prefix('-') else {
        return false;
    };
    match rest.chars().next() {
        None => false,
        Some('.') => rest.len() == 1,
        Some(first) => !first.is_ascii_digit(),
    }
}

/// Optimist's `each_arg`.
fn scan_args(args: &[OsString]) -> Result<Parsed, String> {
    let mut parsed = Parsed::default();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].to_string_lossy();
        let next_param = args
            .get(index + 1)
            .map(|next| next.to_string_lossy())
            .filter(|next| !looks_like_flag(next));
        let has_space = arg.chars().any(char::is_whitespace);
        if arg == "--" {
            parsed.files.extend_from_slice(&args[index + 1..]);
            break;
        } else if arg.starts_with("--") && arg.len() > 2 {
            // `--(\S+?)=(.*)`: the name is at least one character long, and
            // that character may itself be an equals sign.
            let equals = arg
                .char_indices()
                .skip(3)
                .find_map(|(at, letter)| (letter == '=').then_some(at));
            match equals.filter(|&at| !arg[..at].chars().any(char::is_whitespace)) {
                Some(at) => {
                    parsed.record(&arg[..at], Some(&arg[at + 1..]))?;
                }
                None if has_space => parsed.files.push(args[index].clone()),
                None => {
                    if parsed.record(&arg, next_param.as_deref())? {
                        index += 1;
                    }
                }
            }
        } else if arg.starts_with('-') && arg.len() > 1 && !has_space {
            let letters: Vec<char> = arg[1..].chars().collect();
            for (at, letter) in letters.iter().enumerate() {
                let param = next_param.as_deref().filter(|_| at + 1 == letters.len());
                if parsed.record(&format!("-{letter}"), param)? {
                    index += 1;
                }
            }
        } else {
            parsed.files.push(args[index].clone());
        }
        index += 1;
    }
    Ok(parsed)
}

/// Optimist's `FLOAT_RE`: `^-?((\d+(\.\d+)?)|(\.\d+))([eE][-+]?[\d]+)?$`.
fn parse_float(text: &str) -> Option<f64> {
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, Some(exponent)),
        None => (unsigned, None),
    };
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole.is_empty() || digits(whole)) && digits(fraction),
        None => digits(mantissa),
    };
    let exponent_ok =
        exponent.is_none_or(|exponent| digits(exponent.trim_start_matches(['-', '+'])));
    if mantissa_ok && exponent_ok {
        text.parse().ok()
    } else {
        None
    }
}

/// `^-?[\d_]+$`, then Ruby's `to_i`, which stops at a leading or doubled
/// underscore. Values past the range of an `i64` saturate.
fn parse_integer(text: &str) -> Option<i64> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(unsigned) => (true, unsigned),
        None => (false, text),
    };
    if unsigned.is_empty() || !unsigned.bytes().all(|b| b.is_ascii_digit() || b == b'_') {
        return None;
    }
    let mut value = 0i64;
    let mut previous = b'_';
    for byte in unsigned.bytes() {
        if byte == b'_' {
            if previous == b'_' {
                break;
            }
        } else {
            value = value
                .saturating_mul(10)
                .saturating_add(i64::from(byte - b'0'));
        }
        previous = byte;
    }
    Some(if negative { -value } else { value })
}

fn random_u64() -> u64 {
    RandomState::new().build_hasher().finish()
}

fn color_mode(truecolor: bool) -> ColorMode {
    let detected =
        env::var_os("COLORTERM").is_some_and(|value| value == "truecolor" || value == "24bit");
    if truecolor || detected {
        ColorMode::TrueColor
    } else {
        ColorMode::Ansi256
    }
}

enum Error {
    /// A command line error: reported on stderr, exit 255.
    Usage(String),
    /// Already reported, or nothing to say: exit with this status.
    Exit(u8),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

/// Opens a named file, or stdin for `-`.
fn open(file: &OsString) -> io::Result<Box<dyn Read>> {
    Ok(if file == "-" {
        Box::new(io::stdin().lock())
    } else {
        Box::new(File::open(file)?)
    })
}

/// Copies without painting, flushing as data arrives so that interactive input
/// isn't held back.
fn copy(input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
    let mut buffer = [0; 65536];
    loop {
        let count = match input.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        output.write_all(&buffer[..count])?;
        output.flush()?;
    }
}

/// The message upstream prints for this error, if it has one.
fn file_error_message(error: &io::Error) -> Option<&'static str> {
    match error.kind() {
        io::ErrorKind::NotFound => Some("No such file or directory"),
        io::ErrorKind::PermissionDenied => Some("Permission denied"),
        io::ErrorKind::IsADirectory => Some("Is a directory"),
        _ if cfg!(unix) && error.raw_os_error() == Some(6) => Some("Is not a regular file"),
        _ => None,
    }
}

fn help(output: &mut impl Write, tty: bool) -> Result<(), Error> {
    // Optimist's help goes through the rainbow too, with its own settings.
    let mut rainbow = Rainbow::new(Options {
        seed: random_u64() as f64 / u64::MAX as f64 * 8192.0,
        spread: 8.0,
        freq: 0.3,
        color: color_mode(false),
        restore_terminal: tty,
        ..Options::default()
    });
    rainbow.cat(&mut Cursor::new(HELP), output)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Err(Error::Exit(1))
}

fn run(args: &[OsString], output: &mut impl Write) -> Result<(), Error> {
    let tty = io::stdout().is_terminal();
    let parsed = scan_args(args).map_err(Error::Usage)?;
    let given = |opt| parsed.given.iter().find(|given| given.opt == opt);
    if given(Opt::Version).is_some() {
        writeln!(
            output,
            "lolcat {} (c)2011 moe@busyloop.net",
            env!("CARGO_PKG_VERSION")
        )?;
        output.flush()?;
        return Ok(());
    }
    if given(Opt::Help).is_some() {
        return help(output, tty);
    }

    let (mut spread, mut freq, mut seed, mut duration, mut speed) = (3.0, 0.1, 0, 12, 20.0);
    let mut flags = (false, false, false, false);
    for entry in &parsed.given {
        let float = |default| match &entry.param {
            None => Ok(default),
            Some(param) => parse_float(param).ok_or_else(|| {
                let name = entry.opt.long();
                Error::Usage(format!("option '{name}' needs a floating-point number"))
            }),
        };
        let integer = |default| match &entry.param {
            None => Ok(default),
            Some(param) => parse_integer(param).ok_or_else(|| {
                let name = entry.opt.long();
                Error::Usage(format!("option '{name}' needs an integer"))
            }),
        };
        match entry.opt {
            Opt::Spread => spread = float(spread)?,
            Opt::Freq => freq = float(freq)?,
            Opt::Seed => seed = integer(seed)?,
            Opt::Duration => duration = integer(duration)?,
            Opt::Speed => speed = float(speed)?,
            Opt::Animate => flags.0 = !entry.negated,
            Opt::Invert => flags.1 = !entry.negated,
            Opt::Truecolor => flags.2 = !entry.negated,
            Opt::Force => flags.3 = !entry.negated,
            Opt::Version | Opt::Help => {}
        }
    }
    let (animate, invert, truecolor, force) = flags;
    for (value, name) in [
        (spread, "spread"),
        (duration as f64, "duration"),
        (speed, "speed"),
    ] {
        if value < 0.1 {
            return Err(Error::Usage(format!("argument --{name} must be >= 0.1")));
        }
    }
    if seed == 0 {
        seed = (random_u64() % 256) as i64;
    }

    let mut rainbow = Rainbow::new(Options {
        spread,
        freq,
        seed: seed as f64,
        animate,
        duration: duration as u64,
        speed,
        invert,
        color: color_mode(truecolor),
        restore_terminal: tty,
    });
    let stdin = [OsString::from("-")];
    let files = if parsed.files.is_empty() {
        &stdin[..]
    } else {
        &parsed.files
    };
    for file in files {
        let result = open(file).and_then(|mut input| {
            if tty || force {
                rainbow.cat(&mut input, output)
            } else {
                copy(&mut input, output)
            }
        });
        match result {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Err(Error::Exit(1)),
            Err(error) => match file_error_message(&error) {
                // Upstream reports these on stdout.
                Some(message) => {
                    writeln!(output, "lolcat: {}: {message}", file.to_string_lossy())?;
                    output.flush()?;
                    return Err(Error::Exit(1));
                }
                None => return Err(Error::Io(error)),
            },
        }
    }
    Ok(())
}

/// Restores the terminal on Ctrl-C, like upstream's `ensure` and
/// `rescue Interrupt`. A read blocked on stdin wouldn't notice a flag, so the
/// handler finishes the job itself, using only async-signal-safe calls.
#[cfg(unix)]
mod interrupt {
    use std::{
        ffi::c_int,
        sync::atomic::{AtomicBool, Ordering},
    };

    const SIGINT: c_int = 2;
    const STDOUT: c_int = 1;
    static RESTORE: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" {
        fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> usize;
        fn write(fd: c_int, buffer: *const u8, count: usize) -> isize;
        fn _exit(status: c_int) -> !;
    }

    extern "C" fn on_interrupt(_: c_int) {
        if RESTORE.load(Ordering::Relaxed) {
            let sequence = lolcat::RESTORE_TERMINAL;
            // SAFETY: the pointer and length describe a static byte string.
            unsafe { write(STDOUT, sequence.as_ptr(), sequence.len()) };
        }
        // SAFETY: _exit is async-signal-safe and doesn't return.
        unsafe { _exit(0) }
    }

    pub fn install(restore: bool) {
        RESTORE.store(restore, Ordering::Relaxed);
        // SAFETY: the handler only makes async-signal-safe calls.
        unsafe { signal(SIGINT, on_interrupt) };
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    #[cfg(unix)]
    interrupt::install(io::stdout().is_terminal());
    let mut output = BufWriter::new(io::stdout().lock());
    let result = run(&args, &mut output);
    let status = match result {
        Ok(()) => 0,
        Err(Error::Exit(status)) => status,
        Err(Error::Usage(message)) => {
            eprintln!("Error: {message}.\nTry --help for help.");
            255
        }
        Err(Error::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe => 1,
        Err(Error::Io(error)) => {
            eprintln!("lolcat: {error}");
            1
        }
    };
    // Nothing is left buffered on the paths that matter, and a failure here
    // can't be reported anywhere useful.
    let _ = output.flush();
    ExitCode::from(status)
}
