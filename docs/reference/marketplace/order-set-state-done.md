# `waldur-cli marketplace order set-state-done`

Set state done marketplace orders (check status of a submitted provision/terminate).

## Usage

```bash
waldur-cli marketplace order set-state-done <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli marketplace order set-state-done <uuid>
```

(sends a bodyless POST to `/api/marketplace-orders/{uuid}/set_state_done/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
