use std::process::Command;

fn dbscope_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dbscope"))
}

#[test]
fn help_exits_zero() {
    let output = dbscope_bin().arg("--help").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("schema intelligence"));
}

#[test]
fn version_exits_zero() {
    let output = dbscope_bin()
        .arg("--version")
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dbscope"));
}

#[test]
fn demo_produces_reports() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let output = dbscope_bin()
        .args(["demo", "-o", dir.path().to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success(), "demo failed: {:?}", output);
    assert!(dir.path().join("dbscope-report.html").exists());
    assert!(dir.path().join("dbscope-report.json").exists());
    assert!(dir.path().join("dbscope-graph.dot").exists());
}

#[test]
fn completions_bash_exits_zero() {
    let output = dbscope_bin()
        .args(["completions", "bash"])
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dbscope"));
}

#[test]
fn analyze_missing_schema_exits_nonzero() {
    let output = dbscope_bin()
        .arg("analyze")
        .output()
        .expect("failed to run");
    assert!(!output.status.success());
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let output = dbscope_bin()
        .arg("nonexistent")
        .output()
        .expect("failed to run");
    assert!(!output.status.success());
}

#[test]
fn impact_bad_uri_exits_two() {
    let output = dbscope_bin()
        .args(["impact", "users", "--schema", "badscheme://nope"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success());
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2, "connection errors should exit 2");
}

#[test]
fn lint_bad_uri_exits_two() {
    let output = dbscope_bin()
        .args(["lint", "--schema", "badscheme://nope"])
        .output()
        .expect("failed to run");
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2);
}

#[test]
fn diff_missing_file_exits_nonzero() {
    let output = dbscope_bin()
        .args(["diff", "nonexistent.json", "also-nonexistent.json"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success());
}
