# `waldur-cli openstack instance start`

Start openstack instances (vms).

## Usage

```bash
waldur-cli openstack instance start <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack instance start <uuid>
```

(sends a bodyless POST to `/api/openstack-instances/{uuid}/start/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
