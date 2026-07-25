# `waldur-cli auth ssh-key delete`

Delete ssh public keys.

## Usage

```bash
waldur-cli auth ssh-key delete <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli auth ssh-key delete <uuid>
```

Preview without deleting:

```bash
waldur-cli auth ssh-key delete <uuid> --dry-run
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
