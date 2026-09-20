# rotten

Every implemented check fires at least once here.

## References

- HTTP helpers live in `internal/httpx/`.
- Config is in `config/app.toml`.
- Run `make check` before committing.
- Run `npm run lint` for JS.
- Run `frobnicate --all` to regenerate.
- Also `npm run typecheck`.
@docs/missing.md

## Facts

- Requires Go 1.21 and Node 18.
- Tests live in tests/ and handlers under internal/handlers/.
- Format with prettier before committing.

## Shape

- This list item is deliberately longer than one hundred and twenty characters so that the shape-lines check has something to report.

This body line is deliberately longer than one hundred and twenty characters so that the shape-body check has something to report on.

Padding to push the file past the 512 byte budget set in the fixture config, one more sentence should do it here.
