//! Lints each fixture and compares the output with its `expected.txt`, then
//! runs the real binary on the rotten fixture and on this repo's own root.

use std::path::{Path, PathBuf};
use std::process::Command;

use herdr_llm_lint::report;
use herdr_llm_lint::Lint;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn lint_text(root: &Path) -> String {
    let lint = Lint::load(root, None).unwrap();
    report::text(&lint.run())
}

fn check_fixture(name: &str) {
    let root = fixture(name);
    let expected = std::fs::read_to_string(root.join("expected.txt")).unwrap();
    assert_eq!(lint_text(&root), expected, "fixture {name}");
}

#[test]
fn clean_has_no_findings() {
    check_fixture("clean");
}

#[test]
fn rotten_has_one_of_each() {
    check_fixture("rotten");
}

#[test]
fn monorepo_matches_expected() {
    check_fixture("monorepo");
}

#[test]
fn copies_matches_expected() {
    check_fixture("copies");
}

#[test]
fn this_repo_is_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        lint_text(root),
        "",
        "AGENTS.md at the repo root has findings"
    );
}

#[test]
fn binary_lints_and_exits_one() {
    let out = Command::new(env!("CARGO_BIN_EXE_herdr-llm-lint"))
        .args(["lint", fixture("rotten").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let expected = std::fs::read_to_string(fixture("rotten").join("expected.txt")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);

    let out = Command::new(env!("CARGO_BIN_EXE_herdr-llm-lint"))
        .args([
            "lint",
            fixture("clean").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "[]");

    let out = Command::new(env!("CARGO_BIN_EXE_herdr-llm-lint"))
        .args([
            "lint",
            fixture("rotten").to_str().unwrap(),
            "--fail-on",
            "error",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 9);
}

#[test]
fn send_print_writes_the_prompt() {
    let out = Command::new(env!("CARGO_BIN_EXE_herdr-llm-lint"))
        .args(["herdr-send", fixture("rotten").to_str().unwrap(), "--print"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("herdr-llm-lint found problems"));
    assert!(text.contains("CLAUDE.md:7: ref-path"));
}
