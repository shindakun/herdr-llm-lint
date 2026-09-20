# Changelog

## 0.1.0 (2026-09-20)

First release.

- `lint` finds instruction files under a root (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `.cursorrules`, `.cursor/rules/*.mdc`, `.github/copilot-instructions.md`, nested `CLAUDE.md` and `AGENTS.md`), runs the checks, and prints `file:line: check: message` or JSON. Exit 1 at or above `--fail-on`.
- References: `ref-path`, `ref-command`, `ref-target`, `ref-env`, `ref-skill`. Facts: `fact-version`, `fact-layout`, `fact-tool`. Drift: `drift-copies`, `drift-nested`, `drift-stale`, `drift-age`. Content: `content-dup`, `content-conflict`, `content-enforced`, `content-secret`, `content-denylist`, `content-vague`. Size and shape: `size-bytes`, `size-section`, `shape-headings`, `shape-lines`, `shape-body` (off by default). Git: `git-untracked`, `git-local-tracked`. Opt-in through the workspace agent: `llm-conflict`, `llm-unclear`, `llm-missing`.
- Facts resolve against the nearest project directory of each file, so a vendored subtree is checked against its own root. Git refs, common shell variables, and programs the repo declares are not reported.
- `--fix` wraps long lines, drops exact duplicate lines, and symlinks identical copies, then lints again.
- `.herdr-llm-lint.toml` at the root or in the plugin config dir: `files`, `size_bytes`, `line_chars`, `age_commits`, `disable`, `enable`, `denylist`, `denylist_file`, `fail_on`, `[llm]`.
- Herdr: the `lint` action opens the report popup, `send` prompts the workspace agent with the findings, a `worktree.created` hook notifies on findings, and the popup opens a finding in `$EDITOR` or sends them all.
