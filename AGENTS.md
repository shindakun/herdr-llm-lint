# herdr-llm-lint

A linter for agent instruction files, shipped as one Rust binary that runs
as a CLI and as a Herdr plugin. This file is also the repo's own test
subject: `tests/checks.rs` lints the repo root and expects zero findings.

## Layout

- `src/main.rs` dispatches argv; everything else is in the library.
- `src/scan.rs` finds instruction files and splits them into sections.
- `src/refs.rs` pulls paths, commands, env vars, and imports out of a file.
- `src/repo.rs` reads Makefile targets, package scripts, git remotes, and `PATH`;
  `src/facts.rs` reads declared versions and the tool table.
- `src/checks/` holds one file per check group; `src/checks/mod.rs` is the
  registry.
- `src/report.rs` renders text and JSON and builds the agent prompt; `src/fix.rs` is `--fix`.
- `src/herdr.rs` parses the Herdr context and event JSON and calls back
  through the Herdr CLI.
- `src/git.rs` shells out to git; `src/diff.rs` counts hunks.
- `src/tui/` is the report popup.
- `fixtures/` are small repos the tests lint; the expected output for each
  is in `tests/expected/`.
- `docs/PLAN.md` is the design; `herdr-plugin.toml` is the manifest;
  `.herdr-llm-lint.toml` limits the self-lint to this file.

## Rules

- Run `make check` before committing. It runs `make fmt-check`,
  `make clippy`, `make test`, `make audit`, and `make md-lint`.
- A new check goes in its group file under `src/checks/` with `default_on`
  set, and gets a line in `fixtures/rotten/CLAUDE.md` plus
  `tests/expected/rotten.txt`.
- Never add a finding that fires on `fixtures/clean/`.
- Do not add dependencies beyond what `Cargo.toml` already lists without
  saying why in the commit message.
- Update `README.md` and `CHANGELOG.md` in the same change as the code.
