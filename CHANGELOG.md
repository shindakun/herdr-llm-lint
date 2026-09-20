# Changelog

## Unreleased

Scaffold.

- `lint` finds instruction files under a root, runs the checks, and prints findings as `file:line: check: message` or JSON. Exit 1 at or above `--fail-on`.
- `--fix` wraps long lines, drops exact duplicate lines, and symlinks identical copies, then lints again.
- Checks: `ref-path`, `ref-command`, `ref-target`, `fact-version`, `fact-layout`, `fact-tool`, `drift-copies`, `drift-nested`, `drift-stale`, `drift-age`, `git-untracked`, `git-local-tracked`, `content-dup`, `content-conflict`, `content-enforced`, `content-secret`, `content-denylist`, `content-vague`, `size-bytes`, `size-section`, `shape-headings`, `shape-lines` (headings and list items), and `shape-body` (other lines, off by default). The rest of the plan's ids are registered as planned so configs can name them. `ref-path` skips git refs such as `origin/main`. `ref-command` accepts a program the repo declares by config file or toolchain.
- `.herdr-llm-lint.toml` with `files`, `size_bytes`, `line_chars`, `age_commits`, `disable`, `enable`, `denylist`, `denylist_file`, `fail_on`, and `[llm]`.
- Herdr entrypoints: `herdr-action` opens the report popup, `herdr-send` prompts the workspace agent, `herdr-event` notifies on `worktree.created`, `herdr-pane` is the popup.
- Fixtures with expected output, and the repo lints its own `AGENTS.md`.
