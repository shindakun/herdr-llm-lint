# Fixtures

Each directory is a small repo with instruction files. `tests/checks.rs`
lints every directory and compares the text output with its `expected.txt`,
so a new check that changes a fixture's findings updates the file.

- `clean/`: zero findings; guards against false positives.
- `rotten/`: one of every implemented finding, plus a `.herdr-llm-lint.toml`
  that lowers the size budget.
- `monorepo/`: a root file and a nested one; for the `drift-nested` check.
- `copies/`: `CLAUDE.md` and `AGENTS.md` that diverged; for `drift-copies`.

`rotten` names `frobnicate`, which must stay off `PATH` on every machine
that runs the tests.
