# `waldur-cli openstack instance unrescue`

Unrescue openstack instances (vms).

## Usage

```bash
waldur-cli openstack instance unrescue <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack instance unrescue <uuid>
```

(sends a bodyless POST to `/api/openstack-instances/{uuid}/unrescue/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
