# `waldur-cli team user get`

Get users.

## Usage

```bash
waldur-cli team user get <UUID> [OPTIONS]
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |
| `--format FORMAT` | string | table, json, tsv, toon, or ndjson. |

## Examples

```bash
waldur-cli team user get <uuid> --format json
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
