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

Exit 1 when any finding is at or above `--fail-on` (default `warn`, so any finding).

`--fix` applies the safe subset, prints each action to stderr, then lints again and prints what remains:

- long lines reported by `shape-lines` or `shape-body` are wrapped at `line_chars`, with a list item's continuation indented under its text and backtick spans kept whole; headings are left alone
- a line reported by `content-dup` is dropped when it is an exact repeat (after trimming) of an earlier line; near-duplicates stay
- in each directory, an instruction file identical to `CLAUDE.md` (or the first by name) becomes a relative symlink to it

```sh
herdr-llm-lint lint --fix
```

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
| `ref-command` | error | a backticked command whose first word is not on `PATH`, not a tool the repo declares (a `justfile` declares `just`), not a Makefile target, and not a `package.json` script |
| `ref-target` | error | `make X` where `X` is not a target; `npm run X` (or pnpm, yarn, bun) where `X` is not a script |
| `fact-version` | error | "Go 1.21", "Node 18", "Python 3.11", "Rust 1.80" in the file where `go.mod`, `.nvmrc`, `.node-version`, `package.json` engines, `.python-version`, `rust-toolchain.toml`, or `.tool-versions` says otherwise; compared on the components both sides state |
| `fact-layout` | error | a bare path after in, under, at, into, inside, from, or to (`tests live in tests/`) that does not exist; backticked paths are `ref-path` |
| `fact-tool` | warn | a linter, formatter, or test runner named in the file with no config file, dependency, or toolchain in the repo; only names that are unambiguous in prose (prettier, eslint, ruff, golangci-lint, not go, make, black) |
| `drift-copies` | warn | two instruction files in one directory that are neither identical nor symlinked, with the hunk count; `CLAUDE.md` is the reference |
| `drift-nested` | warn | a nested instruction file whose lines (20+ chars, not headings) repeat an ancestor's, three or more of them; reports the overlap |
| `drift-stale` | info | referenced paths committed after the file's last commit; count and the newest. Skipped while the file has uncommitted changes |
| `drift-age` | info | more than `age_commits` (default 50) commits since the file last changed |
| `git-untracked` | warn | an instruction file inside a git checkout that git does not track |
| `git-local-tracked` | warn | a `*.local.md` file that git tracks |
| `content-dup` | warn | a line that repeats an earlier one: same words after list markers and punctuation are stripped, or 80%+ of them; five or more words; lines of opposite polarity are not repeats |
| `content-conflict` | warn | a positive rule (always, must, prefer) and a negative one (never, do not, avoid) sharing three or more object words and 60% of the smaller set; reported on the later line |
| `content-enforced` | info | a rule naming a tool within three words of run, use, with, via, keep, and so on, where `.pre-commit-config.yaml`, `lefthook.yml`, or a `.github/workflows` file already runs that tool; `cargo fmt` counts as rustfmt, `go fmt` as gofmt |
| `content-secret` | error | an AWS key, GitHub token, Slack token, `sk-` API key, Google API key, private-key header, or `password`/`secret`/`token`/`api_key` assigned a value of 8+ chars that is not a placeholder |
| `content-denylist` | warn | a phrase from `denylist` or `denylist_file`, case-insensitive; for keeping personal rules out of shared repos |
| `content-vague` | warn | "be careful", "use best practices", "as appropriate", and about twenty similar phrases with no object or checkable condition |
| `size-bytes` | warn | over budget; default 8 KiB |
| `size-section` | warn | one section over half the file, when the file has four or more sections |
| `shape-headings` | warn | twenty or more lines with no headings, or under a single heading |
| `shape-lines` | warn | headings and list items over 120 chars, outside code blocks and tables |
| `shape-body` | warn, off by default | every other line over 120 chars; files written one paragraph per line trip it on every line, so it needs `enable = ["shape-body"]` |

Paths, targets, tools, and versions are resolved against the project the file belongs to: the nearest ancestor with `.git`, `go.mod`, `Cargo.toml`, `package.json`, `pyproject.toml`, `build.zig`, `Makefile`, or `justfile`, else the lint root. A vendored subtree with its own build file is checked against itself.

Backtick spans are classified by shape: a slash or a known file extension makes a path, two or more words starting with a lowercase program name make a command, `$NAME` or `NAME=` is an env var. Fenced code blocks are skipped. Absolute and `~` paths are left alone, and so are git refs: `HEAD`, `refs/...`, and `<remote>/<branch>` for any remote of the checkout (`origin` and `upstream` when the root is not a git repo).

Planned, with ids reserved so a config can name them: `ref-env`, `ref-skill`, and the opt-in `llm-conflict`, `llm-unclear`, `llm-missing`. Each is described in [docs/PLAN.md](docs/PLAN.md).

## Configure

`.herdr-llm-lint.toml` at the repo root, else `config.toml` in the plugin config dir (`herdr plugin config-dir shindakun.llm-lint`), else defaults:

```toml
files = ["CLAUDE.md", "AGENTS.md", "**/CLAUDE.md"]   # globs relative to the root
size_bytes = 8192
line_chars = 120
age_commits = 50
disable = ["shape-lines"]
enable = ["shape-body"]                              # off-by-default checks to run
fail_on = "warn"                                     # info, warn, or error
denylist = ["my personal"]                           # content-denylist phrases
denylist_file = "~/.config/herdr-llm-lint/denylist.txt"   # one phrase per line, # comments

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

`tests/fix.rs` runs `--fix` on temp copies. `tests/checks.rs` lints each directory under `fixtures/` and compares the output with its `expected.txt`, runs the real binary on the rotten fixture, and lints this repo's root, where `AGENTS.md` must come back clean. `.herdr-llm-lint.toml` at the root limits that run to `AGENTS.md` so the fixtures' deliberate findings stay out of it. The fixtures and the root config disable the history checks (`drift-stale`, `drift-age`, `git-*`), since those read this repo's own git log; `tests/git.rs` exercises them in a throwaway repo.

Adding a check: a function in the group's file under `src/checks/` with `default_on` set, its id moved out of `PLANNED` in `src/checks/mod.rs`, a line in `fixtures/rotten/CLAUDE.md` that trips it, and the matching line in `fixtures/rotten/expected.txt`.

## License

MIT.
