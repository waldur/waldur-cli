# `waldur-cli marketplace resource unlink`

Unlink marketplace resources (provision/terminate any offering).

## Usage

```bash
waldur-cli marketplace resource unlink <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli marketplace resource unlink <uuid>
```

(sends a bodyless POST to `/api/marketplace-resources/{uuid}/unlink/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
