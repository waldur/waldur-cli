# `waldur-cli openstack volume detach`

Detach openstack volumes.

## Usage

```bash
waldur-cli openstack volume detach <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack volume detach <uuid>
```

(sends a bodyless POST to `/api/openstack-volumes/{uuid}/detach/`)

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
