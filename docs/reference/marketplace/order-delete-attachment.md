# `waldur-cli marketplace order delete-attachment`

Delete attachment marketplace orders (check status of a submitted provision/terminate).

## Usage

```bash
waldur-cli marketplace order delete-attachment <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli marketplace order delete-attachment <uuid>
```

(sends a bodyless POST to `/api/marketplace-orders/{uuid}/delete_attachment/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
