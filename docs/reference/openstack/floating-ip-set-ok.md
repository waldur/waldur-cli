# `waldur-cli openstack floating-ip set-ok`

Set ok openstack floating ips. Batch-capable: pass several UUIDs, or omit them and pipe UUIDs in on stdin (one per line -- a bare UUID or a JSON object with a `uuid` field, so `list --format ndjson` composes directly). One failure doesn't stop the rest; the command exits non-zero afterward if any item failed.

## Usage

```bash
waldur-cli openstack floating-ip set-ok [UUID]...
```

| Flag | Type | Description |
| --- | --- | --- |
| `[UUID]...` | positional, 0 or more | uuid(s) of the resource. Reads from stdin if omitted. |

## Examples

```bash
waldur-cli openstack floating-ip set-ok <uuid>
```

Several at once:

```bash
waldur-cli openstack floating-ip set-ok <uuid-1> <uuid-2>
```

(sends a bodyless POST to `/api/openstack-floating-ips/{uuid}/set_ok/` for each)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
