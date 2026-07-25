# `waldur-cli marketplace order unlink`

Unlink marketplace orders (check status of a submitted provision/terminate).

## Usage

```bash
waldur-cli marketplace order unlink <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli marketplace order unlink <uuid>
```

(sends a bodyless POST to `/api/marketplace-orders/{uuid}/unlink/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
