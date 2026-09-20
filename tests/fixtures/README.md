# Fixtures

Recorded against the Herdr 0.9 schemas.

- `agent_list.json`: `herdr agent list`, trimmed to three agents. The command prints JSON without a flag.
- `worktree_created_event.json`: the `HERDR_PLUGIN_EVENT_JSON` an event hook receives for `worktree.created`. Built from the schema (`EventEnvelope` with `EventKind` in snake_case and `EventData` tagged by `type`; `WorkspaceInfo` and `WorktreeInfo` fields), not captured from a live hook.
