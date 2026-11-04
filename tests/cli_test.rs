use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_obstui"))
}

#[test]
fn version_and_help() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("obstui {}", env!("CARGO_PKG_VERSION"))
    );
    let out = bin().arg("--help").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("--probe"));
}

#[test]
fn unknown_argument_fails_with_code_2() {
    let out = bin().arg("--nope").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("argument desconegut"));
}

#[test]
fn probe_prints_a_report() {
    let out = bin().arg("--probe").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for needle in [
        "cpu:",
        "memòria:",
        "gpu:",
        "temperatures",
        "xarxa:",
        "disc:",
        "processos:",
    ] {
        assert!(text.contains(needle), "falta {needle}\n{text}");
    }
}
