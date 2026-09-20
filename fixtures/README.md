# Fixtures

Each directory is a small repo with instruction files. `tests/checks.rs`
lints every directory and compares the text output with
`tests/expected/<name>.txt`, so a new check that changes a fixture's
findings updates that file. The expectations live outside the fixtures
because `ref-env` reads every file in the project.

- `clean/`: zero findings; guards against false positives.
- `rotten/`: one of every finding that fits in a single file, plus a
  `.herdr-llm-lint.toml` that lowers the size budget.
- `monorepo/`: a nested file that repeats the root; `drift-nested`.
- `copies/`: `CLAUDE.md` and `AGENTS.md` that diverged; `drift-copies`.

Every fixture config disables the history checks, since the fixtures sit
inside this repo's git log; `tests/git.rs` covers those.

`rotten` names `frobnicate`, which must stay off `PATH` on every machine
that runs the tests.
