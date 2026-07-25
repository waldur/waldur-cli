# `waldur-cli marketplace order set-state-executing`

Set state executing marketplace orders (check status of a submitted provision/terminate).

## Usage

```bash
waldur-cli marketplace order set-state-executing <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli marketplace order set-state-executing <uuid>
```

(sends a bodyless POST to `/api/marketplace-orders/{uuid}/set_state_executing/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
