# herdr-llm-lint

A linter for agent instruction files: `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `.cursorrules`, `.cursor/rules/*.mdc`, `.github/copilot-instructions.md`. Runs as a CLI and as a [Herdr](https://herdr.dev) plugin. Rust, one binary. Findings look like compiler output and carry a file and line.

Instruction files rot. A path moves, a Makefile target is renamed, a rule gets pasted into three files and edited in one. The agent reads the stale version every session and nobody notices until it does something wrong.

The design is in [docs/PLAN.md](docs/PLAN.md). This is the scaffold: the pipeline, the Herdr wiring, and the first checks are in; most of the planned checks are not written yet.

## Install

```sh
herdr plugin install shindakun/herdr-llm-lint
```

Needs `cargo`; the install step builds the binary. Linux and macOS.

For local development, link the checkout instead:

```sh
cargo build --release
herdr plugin link /path/to/herdr-llm-lint
```

## Use it

```sh
herdr-llm-lint lint                      # the nearest git root, text output
herdr-llm-lint lint path/to/repo
herdr-llm-lint lint --format json
herdr-llm-lint lint --fail-on error      # exit 1 only on errors
```

Output:

```text
CLAUDE.md:7: ref-path: `internal/httpx/` does not exist
CLAUDE.md:9: ref-target: `make check` is not a Makefile target (have: build lint test)
CLAUDE.md:11: ref-command: `frobnicate` is not on PATH
CLAUDE.md: size-bytes: 9400 bytes, budget is 8192
```

Exit 1 when any finding is at or above `--fail-on` (default `warn`, so any finding). `--fix` is accepted but not implemented yet.

Under Herdr, bind the actions in `~/.config/herdr/config.toml`, then `herdr server reload-config`:

```toml
[[keys.command]]
key = "prefix+l"
type = "plugin_action"
command = "shindakun.llm-lint.lint"
description = "lint instruction files"

[[keys.command]]
key = "prefix+shift+l"
type = "plugin_action"
command = "shindakun.llm-lint.send"
description = "send lint findings to agent"
```

`lint` checks the workspace root and opens the report popup when there are findings, or shows a notification when there are none. `send` gives the findings to the workspace's agent as one prompt through `herdr agent prompt`; the prompt says to fix the file, not to change code to match a stale rule unless the rule is right. A `worktree.created` hook lints each new worktree and notifies only if it finds something.

The popup:

| Key | Does |
| --- | --- |
| `enter` | Open the selected finding in `$EDITOR` at its line |
| `a` | Send every finding to the workspace's agent |
| `j` `k` `g` `G`, arrows, mouse | Move the selection |
| `q` | Quit |

Outside Herdr, `herdr-llm-lint herdr-pane path/to/repo` opens the same popup in the current terminal, and `herdr-llm-lint herdr-send path/to/repo --print` prints the prompt instead of sending it.

## Checks

Implemented:

| Id | Severity | Finds |
| --- | --- | --- |
| `ref-path` | error | a path in backticks or an `@import` line that does not exist, relative to the file's directory or the root |
| `ref-command` | error | a backticked command whose first word is not on `PATH`, not a Makefile target, and not a `package.json` script |
| `ref-target` | error | `make X` where `X` is not a target; `npm run X` (or pnpm, yarn, bun) where `X` is not a script |
| `size-bytes` | warn | over budget; default 8 KiB |
| `shape-headings` | warn | twenty or more lines with no headings, or under a single heading |
| `shape-lines` | warn | lines over 120 chars, outside code blocks and tables |

Backtick spans are classified by shape: a slash or a known file extension makes a path, two or more words starting with a lowercase program name make a command, `$NAME` or `NAME=` is an env var. Fenced code blocks are skipped. Absolute and `~` paths are left alone.

Planned, with ids reserved so a config can name them: `ref-env`, `ref-skill`, `fact-version`, `fact-layout`, `fact-tool`, `drift-copies`, `drift-nested`, `drift-stale`, `drift-age`, `content-dup`, `content-conflict`, `content-enforced`, `content-secret`, `content-denylist`, `content-vague`, `size-section`, `git-untracked`, `git-local-tracked`, and the opt-in `llm-conflict`, `llm-unclear`, `llm-missing`. Each is described in [docs/PLAN.md](docs/PLAN.md).

## Configure

`.herdr-llm-lint.toml` at the repo root, else `config.toml` in the plugin config dir (`herdr plugin config-dir shindakun.llm-lint`), else defaults:

```toml
files = ["CLAUDE.md", "AGENTS.md", "**/CLAUDE.md"]   # globs relative to the root
size_bytes = 8192
line_chars = 120
disable = ["shape-lines"]
fail_on = "warn"                                     # info, warn, or error
denylist_file = "~/.config/herdr-llm-lint/denylist.txt"   # for content-denylist, not read yet

[llm]
enabled = false
```

The default `files` list covers the six file types at the root plus nested `CLAUDE.md` and `AGENTS.md`. The walk honours `.gitignore` and skips `.git`.

## Development

```sh
make check       # fmt, clippy, tests, audit, markdown lint; same as CI
make self-lint   # lint this repo's own AGENTS.md
make hooks       # install pre-commit
```

`tests/checks.rs` lints each directory under `fixtures/` and compares the output with its `expected.txt`, runs the real binary on the rotten fixture, and lints this repo's root, where `AGENTS.md` must come back clean. `.herdr-llm-lint.toml` at the root limits that run to `AGENTS.md` so the fixtures' deliberate findings stay out of it.

Adding a check: a function in the group's file under `src/checks/`, its id moved out of `PLANNED` in `src/checks/mod.rs`, a line in `fixtures/rotten/CLAUDE.md` that trips it, and the matching line in `fixtures/rotten/expected.txt`.

## License

MIT.
