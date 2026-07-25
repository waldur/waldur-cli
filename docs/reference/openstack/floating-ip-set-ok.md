# `waldur-cli openstack floating-ip set-ok`

Set ok openstack floating ips.

## Usage

```bash
waldur-cli openstack floating-ip set-ok <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack floating-ip set-ok <uuid>
```

(sends a bodyless POST to `/api/openstack-floating-ips/{uuid}/set_ok/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
