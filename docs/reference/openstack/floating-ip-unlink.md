# `waldur-cli openstack floating-ip unlink`

Unlink openstack floating ips.

## Usage

```bash
waldur-cli openstack floating-ip unlink <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack floating-ip unlink <uuid>
```

(sends a bodyless POST to `/api/openstack-floating-ips/{uuid}/unlink/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
