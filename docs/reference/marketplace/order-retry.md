# `waldur-cli marketplace order retry`

Retry marketplace orders (check status of a submitted provision/terminate). Batch-capable: pass several UUIDs, or omit them and pipe UUIDs in on stdin (one per line -- a bare UUID or a JSON object with a `uuid` field, so `list --format ndjson` composes directly). One failure doesn't stop the rest; the command exits non-zero afterward if any item failed.

## Usage

```bash
waldur-cli marketplace order retry [UUID]...
```

| Flag | Type | Description |
| --- | --- | --- |
| `[UUID]...` | positional, 0 or more | uuid(s) of the resource. Reads from stdin if omitted. |

## Examples

```bash
waldur-cli marketplace order retry <uuid>
```

Several at once:

```bash
waldur-cli marketplace order retry <uuid-1> <uuid-2>
```

(sends a bodyless POST to `/api/marketplace-orders/{uuid}/retry/` for each)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
