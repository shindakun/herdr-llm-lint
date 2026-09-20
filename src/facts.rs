use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub value: String,
    pub source: String,
}

pub fn versions(root: &Path) -> BTreeMap<&'static str, Version> {
    let mut out = BTreeMap::new();
    let mut put = |lang: &'static str, value: Option<String>, source: &str| {
        if let Some(v) = value.filter(|v| is_numeric(v)) {
            out.entry(lang).or_insert(Version {
                value: v,
                source: source.to_string(),
            });
        }
    };
    let read = |name: &str| std::fs::read_to_string(root.join(name)).ok();

    if let Some(t) = read("go.mod") {
        put("go", go_mod_version(&t), "go.mod");
    }
    if let Some(t) = read("rust-toolchain.toml") {
        put("rust", toolchain_channel(&t), "rust-toolchain.toml");
    }
    if let Some(t) = read("rust-toolchain") {
        put("rust", first_line(&t), "rust-toolchain");
    }
    for name in [".nvmrc", ".node-version"] {
        if let Some(t) = read(name) {
            put("node", first_line(&t).map(|v| strip_v(&v)), name);
        }
    }
    if let Some(t) = read("package.json") {
        put("node", engines_node(&t), "package.json engines");
    }
    if let Some(t) = read(".python-version") {
        put("python", first_line(&t), ".python-version");
    }
    if let Some(t) = read(".tool-versions") {
        for line in t.lines() {
            let mut w = line.split_whitespace();
            let (Some(name), Some(v)) = (w.next(), w.next()) else {
                continue;
            };
            let lang = match name {
                "golang" | "go" => "go",
                "nodejs" | "node" => "node",
                "python" => "python",
                "rust" => "rust",
                _ => continue,
            };
            put(lang, Some(v.to_string()), ".tool-versions");
        }
    }
    out
}

fn first_line(t: &str) -> Option<String> {
    t.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
}

fn strip_v(v: &str) -> String {
    v.trim_start_matches('v').to_string()
}

fn go_mod_version(t: &str) -> Option<String> {
    t.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("go "))
        .map(|v| v.trim().to_string())
}

fn toolchain_channel(t: &str) -> Option<String> {
    let v: toml::Value = toml::from_str(t).ok()?;
    v.get("toolchain")?
        .get("channel")?
        .as_str()
        .map(str::to_string)
}

// Only `^`, `~`, `=`, or a bare version pins a major; `>=18` says nothing
// about which major the doc should name.
fn engines_node(t: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let spec = v.get("engines")?.get("node")?.as_str()?.trim();
    let spec = spec.trim_start_matches(['^', '~', '=']);
    let ver: String = spec
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if ver.is_empty() || !spec.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(ver)
}

pub fn is_numeric(v: &str) -> bool {
    !v.is_empty()
        && v.split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether the components both sides state agree; the shorter side wins on
/// precision, so "20" matches "20.11.0".
pub fn versions_agree(a: &str, b: &str) -> bool {
    a.split('.').zip(b.split('.')).all(|(x, y)| x == y)
}

/// The language names a doc may pair with a version.
pub fn lang_key(word: &str) -> Option<&'static str> {
    match word {
        "go" | "golang" => Some("go"),
        "rust" | "rustc" => Some("rust"),
        "node" | "nodejs" | "node.js" => Some("node"),
        "python" | "python3" => Some("python"),
        _ => None,
    }
}

pub fn lang_label(key: &str) -> &'static str {
    match key {
        "go" => "Go",
        "rust" => "Rust",
        "node" => "Node",
        _ => "Python",
    }
}

struct Tool {
    name: &'static str,
    /// Root entries that start with any of these.
    files: &'static [&'static str],
    /// package.json dependency names.
    deps: &'static [&'static str],
    /// Any word match in pyproject.toml or requirements*.txt.
    python: bool,
    /// Present whenever this root file is, because the toolchain ships it.
    bundled_with: &'static [&'static str],
    /// The name is unambiguous in prose; `go`, `make`, `black` are not.
    prose: bool,
}

const fn t(
    name: &'static str,
    files: &'static [&'static str],
    deps: &'static [&'static str],
    python: bool,
    bundled_with: &'static [&'static str],
    prose: bool,
) -> Tool {
    Tool {
        name,
        files,
        deps,
        python,
        bundled_with,
        prose,
    }
}

#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    t("go", &["go.mod"], &[], false, &[], false),
    t("gofmt", &[], &[], false, &["go.mod"], true),
    t("goimports", &[], &[], false, &["go.mod"], true),
    t("govulncheck", &[], &[], false, &["go.mod"], true),
    t("golangci-lint", &[".golangci."], &[], false, &[], true),
    t("cargo", &["Cargo.toml"], &[], false, &[], false),
    t("rustfmt", &["rustfmt.toml", ".rustfmt.toml"], &[], false, &["Cargo.toml"], true),
    t("clippy", &["clippy.toml"], &[], false, &["Cargo.toml"], true),
    t("cargo-audit", &[], &[], false, &["Cargo.toml"], true),
    t("node", &["package.json"], &[], false, &[], false),
    t("npm", &["package.json"], &[], false, &[], false),
    t("npx", &["package.json"], &[], false, &[], false),
    t("pnpm", &["pnpm-lock.yaml"], &[], false, &[], true),
    t("yarn", &["yarn.lock"], &[], false, &[], false),
    t("bun", &["bun.lockb", "bun.lock"], &[], false, &[], false),
    t("prettier", &[".prettierrc", "prettier.config."], &["prettier"], false, &[], true),
    t("eslint", &[".eslintrc", "eslint.config."], &["eslint"], false, &[], true),
    t("biome", &["biome.json"], &["@biomejs/biome"], false, &[], true),
    t("tsc", &["tsconfig.json"], &["typescript"], false, &[], true),
    t("jest", &["jest.config."], &["jest"], false, &[], true),
    t("vitest", &["vitest.config."], &["vitest"], false, &[], true),
    t("mocha", &[".mocharc"], &["mocha"], false, &[], false),
    t("python", &["pyproject.toml", "setup.py", "requirements.txt", ".python-version"], &[], false, &[], false),
    t("pip", &["requirements.txt", "pyproject.toml"], &[], false, &[], false),
    t("uv", &["uv.lock"], &[], false, &[], false),
    t("poetry", &["poetry.lock"], &[], false, &[], false),
    t("pytest", &["pytest.ini", "conftest.py"], &[], true, &[], true),
    t("black", &[], &[], true, &[], false),
    t("ruff", &["ruff.toml", ".ruff.toml"], &[], true, &[], true),
    t("mypy", &["mypy.ini", ".mypy.ini"], &[], true, &[], true),
    t("flake8", &[".flake8"], &[], true, &[], true),
    t("isort", &[".isort.cfg"], &[], true, &[], true),
    t("make", &["Makefile", "makefile", "GNUmakefile"], &[], false, &[], false),
    t("just", &["justfile", "Justfile", ".justfile"], &[], false, &[], false),
    t("pre-commit", &[".pre-commit-config.yaml"], &[], false, &[], true),
    t("markdownlint", &[".markdownlint"], &["markdownlint-cli", "markdownlint-cli2"], false, &[], true),
    t("markdownlint-cli2", &[".markdownlint"], &["markdownlint-cli2"], false, &[], true),
    t("shellcheck", &[".shellcheckrc"], &[], false, &[], true),
    t("stylua", &["stylua.toml", ".stylua.toml"], &[], false, &[], true),
    t("swiftlint", &[".swiftlint.yml"], &[], false, &[], true),
    t("swiftformat", &[".swiftformat"], &[], false, &[], true),
    t("zig", &["build.zig"], &[], false, &[], false),
    t("docker", &["Dockerfile", "docker-compose.", "compose.yaml", "compose.yml"], &[], false, &[], false),
];

const ENFORCERS: &[&str] = &[".pre-commit-config.yaml", "lefthook.yml", ".lefthook.yml"];

// `cargo fmt` and `go fmt` name rustfmt and gofmt without the word.
const ALIASES: &[(&str, &str)] = &[
    ("cargo fmt", "rustfmt"),
    ("cargo clippy", "clippy"),
    ("go fmt", "gofmt"),
    ("go vet", "govet"),
    ("go-fmt", "gofmt"),
    ("golangci", "golangci-lint"),
];

/// Tools that hooks or CI run, mapped to the file that runs them.
pub fn enforced(root: &Path) -> BTreeMap<&'static str, String> {
    let mut sources: Vec<(String, String)> = Vec::new();
    for name in ENFORCERS {
        if let Ok(t) = std::fs::read_to_string(root.join(name)) {
            sources.push((name.to_string(), t));
        }
    }
    if let Ok(d) = std::fs::read_dir(root.join(".github/workflows")) {
        let mut files: Vec<_> = d.filter_map(Result::ok).map(|e| e.path()).collect();
        files.sort();
        for f in files {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned());
            if let (Some(name), Ok(t)) = (name, std::fs::read_to_string(&f)) {
                sources.push((format!(".github/workflows/{name}"), t));
            }
        }
    }
    let mut out = BTreeMap::new();
    for (source, text) in &sources {
        let lower = text.to_ascii_lowercase();
        let ws: BTreeSet<String> = words(text).collect();
        for t in TOOLS.iter().filter(|t| t.prose) {
            let named = ws.contains(t.name)
                || ALIASES
                    .iter()
                    .any(|(alias, tool)| *tool == t.name && lower.contains(alias));
            if named {
                out.entry(t.name).or_insert_with(|| source.clone());
            }
        }
    }
    out
}

/// `text` lowercased with alias phrases replaced by the tool name, so
/// `cargo fmt` tokenises as `rustfmt`.
pub fn unalias(text: &str) -> String {
    let mut lower = text.to_ascii_lowercase();
    for (alias, tool) in ALIASES {
        lower = lower.replace(alias, tool);
    }
    lower
}

pub fn is_prose_tool(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name && t.prose)
}

pub fn tools(root: &Path) -> BTreeSet<&'static str> {
    let entries: Vec<String> = std::fs::read_dir(root)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let has = |name: &str| entries.iter().any(|e| e == name);
    let deps = package_deps(root);
    let python_text = python_text(root);

    TOOLS
        .iter()
        .filter(|t| {
            t.files
                .iter()
                .any(|f| entries.iter().any(|e| e.starts_with(f)))
                || t.deps.iter().any(|d| deps.contains(*d))
                || t.bundled_with.iter().any(|b| has(b))
                || (t.python && has_word(&python_text, t.name))
        })
        .map(|t| t.name)
        .collect()
}

fn package_deps(root: &Path) -> BTreeSet<String> {
    let Some(text) = std::fs::read_to_string(root.join("package.json")).ok() else {
        return BTreeSet::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return BTreeSet::new();
    };
    ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .filter_map(|k| v.get(k)?.as_object())
        .flat_map(|o| o.keys().cloned())
        .collect()
}

fn python_text(root: &Path) -> String {
    let mut out = String::new();
    if let Ok(t) = std::fs::read_to_string(root.join("pyproject.toml")) {
        out.push_str(&t);
        out.push('\n');
    }
    if let Ok(d) = std::fs::read_dir(root) {
        for e in d.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("requirements") && name.ends_with(".txt") {
                if let Ok(t) = std::fs::read_to_string(e.path()) {
                    out.push_str(&t);
                    out.push('\n');
                }
            }
        }
    }
    out
}

fn has_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .any(|w| w.eq_ignore_ascii_case(word))
}

/// Lowercased runs of `[A-Za-z0-9_.+-]`, trailing dots stripped.
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || "_.+-".contains(c)))
        .map(|w| w.trim_end_matches('.').to_ascii_lowercase())
        .filter(|w| !w.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_versions_from_each_source() {
        let root = crate::testutil::tempdir("versions");
        std::fs::write(root.join("go.mod"), "module x\n\ngo 1.23\n").unwrap();
        std::fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"stable\"\n",
        )
        .unwrap();
        std::fs::write(root.join(".nvmrc"), "v20.11.0\n").unwrap();
        std::fs::write(root.join(".python-version"), "3.12\n").unwrap();
        let v = versions(&root);
        assert_eq!(v["go"].value, "1.23");
        assert_eq!(v["go"].source, "go.mod");
        assert!(!v.contains_key("rust"));
        assert_eq!(v["node"].value, "20.11.0");
        assert_eq!(v["python"].value, "3.12");

        let root = crate::testutil::tempdir("versions2");
        std::fs::write(root.join("package.json"), r#"{"engines":{"node":">=18"}}"#).unwrap();
        std::fs::write(root.join(".tool-versions"), "golang 1.22.1\nnodejs 22\n").unwrap();
        let v = versions(&root);
        assert_eq!(v["go"].value, "1.22.1");
        assert_eq!(v["node"].value, "22");
        assert_eq!(v["node"].source, ".tool-versions");
    }

    #[test]
    fn engines_pin_only_with_caret_tilde_or_bare() {
        assert_eq!(
            engines_node(r#"{"engines":{"node":"^20.1"}}"#).as_deref(),
            Some("20.1")
        );
        assert_eq!(
            engines_node(r#"{"engines":{"node":"18"}}"#).as_deref(),
            Some("18")
        );
        assert_eq!(engines_node(r#"{"engines":{"node":">=18"}}"#), None);
        assert_eq!(engines_node(r#"{"name":"x"}"#), None);
    }

    #[test]
    fn version_agreement_is_by_shared_components() {
        assert!(versions_agree("20", "20.11.0"));
        assert!(versions_agree("1.23.4", "1.23"));
        assert!(!versions_agree("1.21", "1.23"));
        assert!(is_numeric("1.23"));
        assert!(!is_numeric("stable"));
        assert!(!is_numeric("1."));
    }

    #[test]
    fn detects_tools_by_file_dep_bundle_and_python_text() {
        let root = crate::testutil::tempdir("tools");
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        std::fs::write(root.join(".prettierrc.json"), "{}").unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"vitest":"1"}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("pyproject.toml"),
            "[tool.ruff]\nline-length = 100\n",
        )
        .unwrap();
        std::fs::write(root.join("justfile"), "x:\n  true\n").unwrap();
        let t = tools(&root);
        for name in [
            "cargo", "clippy", "rustfmt", "prettier", "vitest", "ruff", "just", "npm",
        ] {
            assert!(t.contains(name), "{name}");
        }
        for name in ["go", "gofmt", "eslint", "black", "make", "zig"] {
            assert!(!t.contains(name), "{name}");
        }
        assert!(is_prose_tool("eslint"));
        assert!(!is_prose_tool("go"));
        assert!(!is_prose_tool("frobnicate"));
    }

    #[test]
    fn words_lowercase_and_strip_trailing_dots() {
        let w: Vec<String> = words("Run Prettier. Use golangci-lint, then `Node.js`!").collect();
        assert_eq!(
            w,
            vec!["run", "prettier", "use", "golangci-lint", "then", "node.js"]
        );
    }
}
