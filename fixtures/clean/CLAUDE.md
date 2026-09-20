# clean

A fixture with nothing wrong. Guards against false positives.

## Layout

- Library code is in `src/lib.rs`.
- The build is driven by the `Makefile`; `make check` runs `make build` and `make test`.
- JS tooling: `npm run lint` and `npm run test`.

## Facts

- Requires Go 1.23 and Node 20.
- Tests live in tests/ and library code under src/.
- Format with prettier. Go and Python as words are not tool claims.

## Rules

- Run `make check` before committing.
- Rebase onto `origin/main` first; compare with `HEAD` and `refs/heads/main`.
- Keep `package.json` scripts in sync with the Makefile.
- Identifiers like `Config` and `foo()` are not paths.
- Absolute and home paths such as `/usr/bin/env` and `~/.claude/CLAUDE.md` are not checked.
- `$HOME` is an env var and is not checked yet.

```sh
make nope   # fenced blocks are skipped
```
