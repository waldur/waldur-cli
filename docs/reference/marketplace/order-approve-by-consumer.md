# `waldur-cli marketplace order approve-by-consumer`

Approve by consumer marketplace orders (check status of a submitted provision/terminate).

## Usage

```bash
waldur-cli marketplace order approve-by-consumer <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli marketplace order approve-by-consumer <uuid>
```

(sends a bodyless POST to `/api/marketplace-orders/{uuid}/approve_by_consumer/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
