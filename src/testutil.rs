use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

// Not removed: the OS cleans temp, and the dir helps when a test fails.
pub fn tempdir(tag: &str) -> PathBuf {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("herdr-llm-lint-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
