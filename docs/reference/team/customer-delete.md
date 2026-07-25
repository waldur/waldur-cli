# `waldur-cli team customer delete`

Delete customers (organizations).

## Usage

```bash
waldur-cli team customer delete <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli team customer delete <uuid>
```

Preview without deleting:

```bash
waldur-cli team customer delete <uuid> --dry-run
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
