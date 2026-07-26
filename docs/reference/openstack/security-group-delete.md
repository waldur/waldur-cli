# `waldur-cli openstack security-group delete`

Delete openstack security groups. Batch-capable: pass several UUIDs, or omit them and pipe UUIDs in on stdin (one per line -- a bare UUID or a JSON object with a `uuid` field, so `list --format ndjson` composes directly). One failure doesn't stop the rest; the command exits non-zero afterward if any item failed.

## Usage

```bash
waldur-cli openstack security-group delete [UUID]...
```

| Flag | Type | Description |
| --- | --- | --- |
| `[UUID]...` | positional, 0 or more | uuid(s) of the resource. Reads from stdin if omitted. |

## Examples

```bash
waldur-cli openstack security-group delete <uuid>
```

Several at once:

```bash
waldur-cli openstack security-group delete <uuid-1> <uuid-2>
```

From a filtered list, without an intermediate `jq`:

```bash
waldur-cli openstack security-group list --format ndjson --filter state=OK | waldur-cli openstack security-group delete
```

Preview without deleting:

```bash
waldur-cli openstack security-group delete <uuid> --dry-run
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
