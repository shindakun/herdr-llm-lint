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
- Set `$UNDEFINED_SERVICE_TOKEN` before running; `/exists` works but `/missing-one` and the `nope` skill do not.

## Facts

- Requires Go 1.21 and Node 18.
- Tests live in tests/ and handlers under internal/handlers/.
- Format with prettier before committing.

## Content

- Always run the integration tests before merging a branch.
- Always run the integration tests before merging a branch!
- Never run the integration tests before merging a branch.
- Run gofmt before committing.
- The database password = "hunter2hunter2" is in the README.
- This is my personal rule and does not belong in a shared repo.
- Be careful and use best practices.

## Shape

Padding so that one section holds more than half of this file, which the size-section check reports.
Padding so that one section holds more than half of this file, and this line differs enough not to be a duplicate of the one above by token overlap, since it carries extra words.
Padding, third line, with other words: sections are measured in bytes including the heading line and blank lines that follow it.
More padding, fourth line, kept under the line limit: the section total is what matters for the size check here.
More padding, fifth line, kept under the line limit: every one of these lines is worded differently on purpose.
More padding, sixth line, kept under the line limit: different words again so the duplicate check stays quiet.
More padding, seventh line, kept under the line limit: enough bytes now for the section to pass half the file.

- This list item is deliberately longer than one hundred and twenty characters so that the shape-lines check has something to report.

This body line is deliberately longer than one hundred and twenty characters so that the shape-body check has something to report on.

Padding to push the file past the 512 byte budget set in the fixture config, one more sentence should do it here.
