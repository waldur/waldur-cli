# `waldur-cli auth personal-access-token rotate`

Rotate personal access tokens (named, scoped, time-limited api credentials).

## Usage

```bash
waldur-cli auth personal-access-token rotate <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli auth personal-access-token rotate <uuid>
```

(sends a bodyless POST to `/api/personal-access-tokens/{uuid}/rotate/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
