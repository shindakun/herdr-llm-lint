//! `lint --fix` against a temp copy of the fixtures.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("herdr-llm-lint-fix-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    for e in std::fs::read_dir(from).unwrap().filter_map(Result::ok) {
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

fn run(root: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_herdr-llm-lint"))
        .arg("lint")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn wraps_long_lines_and_leaves_near_duplicates() {
    let root = temp("rotten");
    copy_dir(&fixture("rotten"), &root);
    let (code, stdout, stderr) = run(&root, &["--fix"]);
    assert_eq!(code, 1);
    assert_eq!(
        stderr,
        "CLAUDE.md:35: wrapped into 2 lines\nCLAUDE.md:36: wrapped into 2 lines\n\
CLAUDE.md:42: wrapped into 2 lines\nCLAUDE.md:44: wrapped into 2 lines\n"
    );
    assert!(!stdout.contains("shape-"), "{stdout}");
    assert!(
        stdout.contains("content-dup"),
        "near-duplicates stay: {stdout}"
    );
    let text = std::fs::read_to_string(root.join("CLAUDE.md")).unwrap();
    assert!(text.contains("- This list item is deliberately longer than one hundred and twenty characters so that the shape-lines check has\n  something to report.\n"));
    assert!(text.ends_with('\n'));

    let (_, _, stderr) = run(&root, &["--fix"]);
    assert_eq!(stderr, "", "a second pass has nothing to do");
}

#[test]
fn drops_exact_duplicates_and_symlinks_identical_copies() {
    let root = temp("copies");
    let text = "# t\n\n- Run the full test suite before every commit.\n- Keep the changelog current.\n- Run the full test suite before every commit.\n";
    std::fs::write(root.join("CLAUDE.md"), text).unwrap();
    std::fs::write(root.join("AGENTS.md"), text).unwrap();
    std::fs::write(root.join("GEMINI.md"), "# other\n").unwrap();
    std::fs::write(
        root.join(".herdr-llm-lint.toml"),
        "disable = [\"drift-stale\", \"drift-age\", \"git-untracked\", \"git-local-tracked\"]\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&root, &["--fix"]);
    assert_eq!(
        stderr,
        "AGENTS.md:5: dropped duplicate line\nCLAUDE.md:5: dropped duplicate line\nAGENTS.md: symlinked to CLAUDE.md\n"
    );
    assert_eq!(code, 1, "GEMINI.md still differs from CLAUDE.md");
    assert_eq!(
        stdout,
        "GEMINI.md:1: drift-copies: differs from CLAUDE.md in 1 hunk\n"
    );
    let link = std::fs::read_link(root.join("AGENTS.md")).unwrap();
    assert_eq!(link, PathBuf::from("CLAUDE.md"));
    assert_eq!(
        std::fs::read_to_string(root.join("CLAUDE.md")).unwrap(),
        "# t\n\n- Run the full test suite before every commit.\n- Keep the changelog current.\n"
    );
}
