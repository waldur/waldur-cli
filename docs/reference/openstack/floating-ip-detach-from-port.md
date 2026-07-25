# `waldur-cli openstack floating-ip detach-from-port`

Detach from port openstack floating ips.

## Usage

```bash
waldur-cli openstack floating-ip detach-from-port <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack floating-ip detach-from-port <uuid>
```

(sends a bodyless POST to `/api/openstack-floating-ips/{uuid}/detach_from_port/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
