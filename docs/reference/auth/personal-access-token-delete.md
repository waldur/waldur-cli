# `waldur-cli auth personal-access-token delete`

Delete personal access tokens (named, scoped, time-limited api credentials).

## Usage

```bash
waldur-cli auth personal-access-token delete <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli auth personal-access-token delete <uuid>
```

Preview without deleting:

```bash
waldur-cli auth personal-access-token delete <uuid> --dry-run
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
