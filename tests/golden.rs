//! Replays output recorded from the real Ruby lolcat by `scripts/oracle.py record`.

use std::{
    fs::{self, File},
    path::Path,
    process::{Command, Stdio},
};

fn check(case: &Path) -> Result<(), String> {
    let args = fs::read_to_string(case.join("args")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lolcat"))
        .args(args.lines())
        .current_dir(case)
        .env_remove("COLORTERM")
        .stdin(Stdio::from(File::open(case.join("input")).unwrap()))
        .output()
        .unwrap();
    let expected = |name| fs::read(case.join(name)).unwrap();
    let status = fs::read_to_string(case.join("status")).unwrap();
    if output.status.code() != status.trim().parse().ok() {
        return Err(format!("status {:?}, expected {status}", output.status));
    }
    if output.stderr != expected("stderr") {
        return Err(format!(
            "stderr {:?}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if output.stdout != expected("stdout") {
        let wanted = expected("stdout");
        let at = output
            .stdout
            .iter()
            .zip(&wanted)
            .position(|(a, b)| a != b)
            .unwrap_or(output.stdout.len().min(wanted.len()));
        return Err(format!(
            "stdout differs at byte {at}: got {:?}, expected {:?}",
            String::from_utf8_lossy(&output.stdout[at..(at + 40).min(output.stdout.len())]),
            String::from_utf8_lossy(&wanted[at..(at + 40).min(wanted.len())]),
        ));
    }
    Ok(())
}

#[test]
fn output_matches_the_ruby_original() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut count = 0;
    let mut failures = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let case = entry.unwrap().path();
        count += 1;
        if let Err(error) = check(&case) {
            failures.push(format!("{}: {error}", case.display()));
        }
    }
    assert!(count > 0, "no recorded cases");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
