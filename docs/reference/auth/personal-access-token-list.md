# `waldur-cli auth personal-access-token list`

List personal access tokens (named, scoped, time-limited api credentials).

## Usage

```bash
waldur-cli auth personal-access-token list [OPTIONS]
```

| Flag | Type | Description |
| --- | --- | --- |
| `--fields FIELDS` | string | Fetch only these fields from the server (comma-separated). |
| `--jmespath EXPR` | string | Reshape the already-fetched result client-side (https://jmespath.org). |
| `--limit N` | integer | Stop after this many items. |
| `--format FORMAT` | string | table, json, tsv, toon, or ndjson. |

## Examples

```bash
waldur-cli auth personal-access-token list --fields uuid,name --format json
```

Project just the columns you need, client-side:

```bash
waldur-cli auth personal-access-token list --jmespath '[].[uuid, name]'
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
