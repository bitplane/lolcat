#!/usr/bin/env python3
"""Compare this port with the real Ruby lolcat, which must be installed.

    oracle.py record        rewrite tests/golden from the real lolcat
    oracle.py fuzz [COUNT]  compare the two on random inputs and options
    oracle.py pty           compare terminal output and Ctrl-C handling

Not part of just check, because CI has no Ruby lolcat.
"""

import argparse
import os
from pathlib import Path
import pty
import random
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
GOLDEN = ROOT / "tests/golden"
PORT = ROOT / "target/debug/lolcat"
REAL = shutil.which("lolcat") or "/usr/games/lolcat"
ENV = {key: value for key, value in os.environ.items() if key != "COLORTERM"}
ENV["LC_ALL"] = "C.UTF-8"

TEXT = b"The quick brown fox jumps over the lazy dog.\n"
BIG = b"".join(b"%05d %s" % (n, TEXT) for n in range(170))
# A two-byte character across the first 4096-byte read.
SPLIT_CHAR = b"x" * 4095 + "é".encode() + b"tail\n" + TEXT
# An escape sequence across the first 4096-byte read.
SPLIT_ESCAPE = b"x" * 4094 + b"\x1b[31mred\x1b[0m\n" + TEXT

# lol.rb's INCOMPLETE_ESCAPE. Python's multi-line $ is the same as Ruby's $.
INCOMPLETE_ESCAPE = re.compile(rb"\x1b(?:[ -/]*|[\]PX^_][^\x07\x1b]*|\[[0-?]*)$", re.M)

# name: (arguments, stdin)
CASES = {
    "plain": (["-f", "-S", "1"], TEXT * 3),
    "truecolor": (["-f", "-t", "-S", "1"], TEXT * 3),
    "invert": (["-f", "-i", "-S", "7"], TEXT),
    "invert_truecolor": (["-f", "-it", "-S", "7"], TEXT),
    "negative_seed": (["-f", "-S", "-5"], TEXT),
    "spread_freq": (["-f", "-S", "9", "-p", "0.5", "--freq=0.7"], TEXT),
    "empty": (["-f", "-S", "1"], b""),
    "blank_lines": (["-f", "-S", "1"], b"\n\n\nx\n\n"),
    "no_final_newline": (["-f", "-S", "1"], b"one\ntwo"),
    "tabs_crlf": (["-f", "-S", "1"], b"a\tb\t\tc\r\nd\r\n"),
    "unicode": (["-f", "-S", "1"], "héllo 日本語 🌈\n".encode()),
    "csi": (["-f", "-S", "1"], b"\x1b[1;31mbold\x1b[0m plain\x1b[K\n"),
    "osc": (["-f", "-S", "1"], b"\x1b]0;title\x07after \x1b]8;;http://x\x1b\\link\n"),
    "intermediates": (["-f", "-S", "1"], b"\x1b(Bcharset \x1b #8 \x1b\x1bdouble\n"),
    "escape_at_line_end": (["-f", "-S", "1"], b"\x1b[0m"),
    "chunks": (["-f", "-S", "3"], BIG),
    "split_char": (["-f", "-S", "1"], SPLIT_CHAR),
    "split_escape": (["-f", "-S", "1"], SPLIT_ESCAPE),
    "animate": (["-f", "-a", "-d", "2", "-s", "1000", "-S", "1"], b"one\n\ntwo"),
    "animate_erase": (["-f", "-a", "-d", "3", "-s", "1000", "-S", "1", "-t"], b"a\x1b[Kb\x1b[2Jc\n"),
    "two_files": (["-f", "-S", "1", "input", "input"], TEXT),
    "stdin_between": (["-f", "-S", "1", "input", "-", "input"], b"no newline"),
    "no_force": (["-S", "1"], TEXT),
    "no_force_negated": (["--no-force", "-S", "1"], b"\x1b[31mraw\t\xff\n"),
    "bundled": (["-fS", "1", "--truecolor", "--no-invert"], TEXT),
    "double_dash": (["-f", "-S", "1", "--", "-v"], b""),
    "flag_value_ignored": (["-S", "1", "--force=no"], TEXT),
    "default_value": (["-f", "-S", "1", "--spread"], TEXT),
    "missing_file": (["input", "nosuch"], TEXT),
    "directory": (["."], b""),
    "unknown_long": (["--foo"], b""),
    "unknown_short": (["-x"], b""),
    "unknown_negated": (["--no-foo"], b""),
    "bad_syntax": (["---x"], b""),
    "repeated": (["-p", "1", "--spread=2"], b""),
    "bad_float": (["--spread=abc"], b""),
    "bad_integer": (["-d", "1.5"], b""),
    "low_spread": (["-p", "0"], b""),
    "low_duration": (["-d", "0"], b""),
    "low_speed": (["-s", "0.01"], b""),
    "attached_value": (["-p3"], b""),
    "error_before_help": (["--help", "--foo"], b""),
}


def build():
    subprocess.run(["cargo", "build", "--locked", "--quiet"], cwd=ROOT, check=True)


def run(program, args, stdin_path, cwd=None):
    with open(stdin_path, "rb") as stdin:
        return subprocess.run([program, *args], stdin=stdin, capture_output=True, cwd=cwd, env=ENV)


def record():
    shutil.rmtree(GOLDEN, ignore_errors=True)
    for name, (args, stdin) in CASES.items():
        case = GOLDEN / name
        case.mkdir(parents=True)
        (case / "args").write_text("".join(arg + "\n" for arg in args))
        (case / "input").write_bytes(stdin)
        result = run(REAL, args, case / "input", cwd=case)
        (case / "stdout").write_bytes(result.stdout)
        (case / "stderr").write_bytes(result.stderr)
        (case / "status").write_text(f"{result.returncode}\n")
    print(f"recorded {len(CASES)} cases from {REAL}")


def random_input(rng):
    pieces = [
        b"a", b"word ", b"\n", b"\n", b"\t", b"\r\n", " é".encode(), "日本".encode(), "🌈".encode(),
        b"\x1b[0m", b"\x1b[1;31m", b"\x1b[K", b"\x1b[2J", b"\x1b[", b"\x1b", b"\x1b\x1b", b"\x1b(B",
        b"\x1b #", b"\x1b ", b"\x1b]0;t\x07", b"\x1b]0;t", b"\x1bPdata\x1b\\", b"\x1b_", b"\x07", b"[", b"]",
    ]
    weights = [30, 30, 12, 12, 3, 2, 8, 4, 4, 5, 5, 2, 2, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1]
    if rng.random() < 0.7:
        # Without the pieces that make upstream discard its input, so that most
        # runs compare all of the output.
        for piece in (b"\x1b[", b"\x1b", b"\x1b ", b"\x1b #", b"\x1b]0;t", b"\x1b_"):
            weights[pieces.index(piece)] = 0
    size = rng.choice([5, 50, 500, 6000])
    return b"".join(rng.choices(pieces, weights, k=rng.randint(0, size)))


def random_args(rng):
    args = ["-f", "-S", str(rng.choice([1, 2, 77, 1000, -3]))]
    if rng.random() < 0.5:
        args += ["-t"]
    if rng.random() < 0.2:
        args += ["-i"]
    if rng.random() < 0.3:
        args += ["-p", str(rng.choice([0.1, 1, 2.5, 8, 1000]))]
    if rng.random() < 0.3:
        args += ["-F", str(rng.choice([0, 0.01, 0.3, 1.5, 100]))]
    if rng.random() < 0.05:
        args += ["-a", "-d", "2", "-s", "5000"]
    return args


def fuzz(count):
    build()
    rng = random.Random(42)
    same = dropped = 0
    with tempfile.TemporaryDirectory(prefix="lolcat-oracle-") as directory:
        path = Path(directory) / "input"
        for index in range(count):
            data, args = random_input(rng), random_args(rng)
            path.write_bytes(data)
            real, port = run(REAL, args, path), run(PORT, args, path)
            if real.stdout == port.stdout and real.returncode == port.returncode:
                same += 1
            elif port.stdout.startswith(real.stdout) and INCOMPLETE_ESCAPE.search(data):
                # Upstream loses the rest of its input when an escape looks unfinished.
                dropped += 1
            else:
                keep = Path(tempfile.gettempdir()) / "lolcat-oracle-failure"
                keep.write_bytes(data)
                print(f"case {index} differs: {args}, input saved to {keep}")
                return 1
    print(f"{same} identical, {dropped} where upstream dropped trailing input")
    return 0


def in_pty(program, args, stdin_data, interrupt_after=None):
    master, slave = pty.openpty()
    process = subprocess.Popen(
        [program, *args], stdin=subprocess.PIPE, stdout=slave, stderr=subprocess.DEVNULL, env=ENV
    )
    os.close(slave)
    if stdin_data is not None:
        process.stdin.write(stdin_data)
        process.stdin.close()
    if interrupt_after is not None:
        time.sleep(interrupt_after)
        process.send_signal(signal.SIGINT)
    output = b""
    while True:
        try:
            data = os.read(master, 65536)
        except OSError:
            break
        if not data:
            break
        output += data
    os.close(master)
    return output, process.wait()


def pty_check():
    build()
    failed = False
    for args, stdin_data in [
        (["-S", "1"], TEXT),
        (["-S", "1", "-a", "-d", "2", "-s", "1000"], TEXT),
        (["-S", "1", "nosuch"], b""),
        (["-S", "1", "."], b""),
    ]:
        real, port = in_pty(REAL, args, stdin_data), in_pty(PORT, args, stdin_data)
        print("same" if real == port else "DIFFERENT", args)
        failed |= real != port
    # stdin stays open, so this interrupts a blocked read. Ruby needs a moment to start.
    real = in_pty(REAL, ["-S", "1"], None, interrupt_after=1.0)
    port = in_pty(PORT, ["-S", "1"], None, interrupt_after=0.3)
    print("same" if real == port else f"DIFFERENT {real} {port}", "Ctrl-C", real)
    failed |= real != port
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["record", "fuzz", "pty"])
    parser.add_argument("count", nargs="?", type=int, default=500)
    options = parser.parse_args()
    if options.command == "record":
        record()
        return 0
    if options.command == "fuzz":
        return fuzz(options.count)
    return pty_check()


if __name__ == "__main__":
    sys.exit(main())
