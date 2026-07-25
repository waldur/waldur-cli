# `waldur-cli openstack instance stop`

Stop openstack instances (vms).

## Usage

```bash
waldur-cli openstack instance stop <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack instance stop <uuid>
```

(sends a bodyless POST to `/api/openstack-instances/{uuid}/stop/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
