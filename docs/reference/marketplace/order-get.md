# `waldur-cli marketplace order get`

Get marketplace orders (check status of a submitted provision/terminate).

## Usage

```bash
waldur-cli marketplace order get <UUID> [OPTIONS]
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |
| `--web` | flag | Open this resource's page in Waldur's web UI (HomePort) instead of printing it. |
| `--format FORMAT` | string | table, json, tsv, toon, or ndjson. |

## Examples

```bash
waldur-cli marketplace order get <uuid> --format json
```

Open it in the browser instead:

```bash
waldur-cli marketplace order get <uuid> --web
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
