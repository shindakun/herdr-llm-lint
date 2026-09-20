# herdr-llm-lint

A linter for agent instruction files: `CLAUDE.md`, `AGENTS.md`,
`GEMINI.md`, `.cursorrules`, `.cursor/rules/*.mdc`,
`.github/copilot-instructions.md`. Runs as a CLI and as a herdr plugin. One
Rust binary. Findings look like compiler output and carry a file and line.

## Why

Instruction files rot. A path moves, a Makefile target is renamed, a rule
gets pasted into three files and edited in one. The agent reads the stale
version every session and nobody notices until it does something wrong.

## Checks

Each check has an id, a severity, and a one-line message with a location.
All run without a model. The last group calls the agent and is off by
default.

### References

| Id | Finds |
|---|---|
| `ref-path` | a path in backticks or an `@import` line that does not exist in the worktree |
| `ref-command` | a backticked command whose first word is not on PATH, not a Makefile target, not a `package.json` script, and not a `cargo`/`go`/`npm` subcommand |
| `ref-target` | `make X` where `X` is not a target; `npm run X` where `X` is not a script |
| `ref-env` | an `$ENV_VAR` or `ENV_VAR=` that no `.env.example`, `Makefile`, CI file, or source file mentions |
| `ref-skill` | `.claude/skills/<name>` or `.claude/commands/<name>` named but absent |

### Facts about the repo

| Id | Finds |
|---|---|
| `fact-version` | a language or tool version in the file that disagrees with `go.mod`, `rust-toolchain.toml`, `.nvmrc`, `package.json` engines, `.python-version`, `.tool-versions` |
| `fact-layout` | a claim like "tests live in `tests/`" or "entry point is `cmd/x`" where the path is missing |
| `fact-tool` | a named linter, formatter, or test runner with no config file and no dependency in the repo |

### Drift

| Id | Finds |
|---|---|
| `drift-copies` | two instruction files in the same directory that are neither identical nor symlinked, with the diff hunks |
| `drift-nested` | a subdirectory instruction file that repeats lines from the root file (report overlap ratio) |
| `drift-stale` | referenced paths changed in git after the instruction file was last edited; report the count and the newest one |
| `drift-age` | commits since the file last changed, past a threshold |

### Content

| Id | Finds |
|---|---|
| `content-dup` | near-duplicate lines inside one file |
| `content-conflict` | an "always X" and a "never X" on the same object, by token overlap |
| `content-enforced` | a rule the repo already enforces by tooling (gofmt in pre-commit, prettier in CI), so the sentence is dead weight |
| `content-secret` | a token, key, or password pattern |
| `content-denylist` | a phrase from a user-supplied list; for keeping personal rules out of shared repos |
| `content-vague` | an imperative with no object and no checkable condition: "be careful", "use best practices" |

### Size and shape

| Id | Finds |
|---|---|
| `size-bytes` | over budget; default 8 KiB, configurable |
| `size-section` | one section over a third of the file |
| `shape-headings` | no headings, or a single heading over a wall of text |
| `shape-lines` | lines over 120 chars |

### Git

| Id | Finds |
|---|---|
| `git-untracked` | `CLAUDE.md` or `AGENTS.md` not tracked |
| `git-local-tracked` | `CLAUDE.local.md` tracked |

### Model-assisted, opt-in

| Id | Finds |
|---|---|
| `llm-conflict` | rules that contradict each other in meaning |
| `llm-unclear` | rules an agent could read two ways |
| `llm-missing` | things the repo does that no rule covers: an unusual build step, a generated directory, a required env var |

These send the file and a short repo summary to the workspace agent through
`herdr agent prompt` and parse a fixed reply format. They never run in CI.

## Output

```text
CLAUDE.md:14: ref-path: `internal/httpx/` does not exist
CLAUDE.md:22: ref-target: `make check` is not a Makefile target (have: build test lint)
CLAUDE.md:31: fact-version: says Go 1.21, go.mod says 1.23
AGENTS.md:1: drift-copies: differs from CLAUDE.md in 3 hunks
CLAUDE.md:40: content-enforced: gofmt already runs in .pre-commit-config.yaml
CLAUDE.md: drift-stale: 6 referenced paths changed since 2026-06-02 (newest: cmd/serve/main.go)
```

`--format json` for tooling. `--fix` for the safe subset: symlink identical
copies, wrap long lines, drop exact duplicate lines. Exit 1 on any finding
at or above `--fail-on` (default `warn`).

## Config

`.herdr-llm-lint.toml` at repo root, or the plugin config dir for user
defaults:

```toml
files = ["CLAUDE.md", "AGENTS.md", "**/CLAUDE.md"]
size_bytes = 8192
line_chars = 120
disable = ["content-vague"]
fail_on = "warn"
denylist_file = "~/.config/herdr-llm-lint/denylist.txt"
[llm]
enabled = false
```

Every check id, implemented or not, is a valid entry in `disable`, so a
config written against the full list keeps working as checks land.

## Herdr wiring

```toml
[[actions]]
id = "lint"
title = "Lint instruction files"
contexts = ["workspace"]
command = ["./target/release/herdr-llm-lint", "herdr-action"]

[[actions]]
id = "send"
title = "Send findings to agent"
contexts = ["workspace"]
command = ["./target/release/herdr-llm-lint", "herdr-send"]

[[events]]
on = "worktree.created"
command = ["./target/release/herdr-llm-lint", "herdr-event"]

[[panes]]
id = "report"
title = "Instruction lint"
placement = "popup"
width = "80%"
height = "70%"
command = ["./target/release/herdr-llm-lint", "herdr-pane"]
```

- `herdr-action`: lint the workspace root from `HERDR_PLUGIN_CONTEXT_JSON`,
  open the popup with the report.
- `herdr-send`: format findings as one prompt, `herdr agent prompt` to the
  workspace's agent. Prompt says: fix the file, do not change code to match
  a stale rule unless the rule is right.
- `herdr-event`: lint on new worktree, notify only if findings.
- `herdr-pane`: the report, keys `enter` open the line in `$EDITOR`, `a`
  send to agent, `q` quit.

## Repo layout

```text
Cargo.toml
herdr-plugin.toml
.herdr-llm-lint.toml   limits the self-lint to AGENTS.md; fixtures are excluded
AGENTS.md              this repo's own instruction file; the tests lint it
src/
  main.rs         argv dispatch only
  lib.rs          Lint: load config, docs, repo facts; run the checks
  cli.rs          lint + the herdr-* subcommands, root detection
  model.rs        Finding, Severity, the file:line: check: message format
  config.rs       .herdr-llm-lint.toml, plugin config dir fallback
  scan.rs         find instruction files, parse into lines and sections
  refs.rs         extract paths, commands, env vars, imports
  repo.rs         Makefile targets, package scripts, PATH; versions, tool configs, git to come
  checks/         mod.rs is the registry; one file per group: refs, facts, drift, content, size, git, llm
  report.rs       text and json output, agent prompt, fix mode
  herdr.rs        plugin env, context json, worktree.created event, calls into herdr
  tui/            the report popup (ratatui): mod.rs loop, app.rs state, keys.rs, ui.rs
  testutil.rs     temp dirs for unit tests
fixtures/
  clean/          zero findings, guards against false positives
  rotten/         one of every finding, plus a config that lowers the size budget
  monorepo/       nested files, drift
  copies/         CLAUDE.md and AGENTS.md diverged
  */expected.txt  the exact text output for that fixture
tests/
  checks.rs       lint each fixture against expected.txt, run the binary, lint this repo
  fixtures/       recorded herdr JSON: agent list, worktree.created event
```

The pane is a ratatui TUI like the other herdr plugins rather than a plain
pager, because `enter` and `a` need key handling and a selection. It is
small: four files and no background work.

## Order

1. `scan`, `refs`, `ref-path`, `ref-command`, `ref-target`. Text output. Done.
2. `repo` facts, `fact-*`.
3. `drift-*`, `git-*`.
4. `content-*`, `size-*`, `shape-*`. `size-bytes`, `shape-headings`, and
   `shape-lines` are done; `size-section` and `content-*` are not.
5. `--fix`, `--format json`. JSON is done; `--fix` is accepted and errors.
6. Herdr subcommands and manifest. Done, not yet run under a live Herdr.
7. `llm-*`.

## Open questions

- Which instruction file formats matter beyond the six listed. Add on
  demand.
- Whether `content-conflict` by token overlap is worth having or produces
  noise. Run it on real repos before keeping it.
- How `@import` resolution works for paths outside the repo (`~/.claude/`).
  Resolve them but do not fail on them by default.
