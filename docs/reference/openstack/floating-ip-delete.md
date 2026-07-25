# `waldur-cli openstack floating-ip delete`

Delete openstack floating ips.

## Usage

```bash
waldur-cli openstack floating-ip delete <UUID>
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |

## Examples

```bash
waldur-cli openstack floating-ip delete <uuid>
```

Preview without deleting:

```bash
waldur-cli openstack floating-ip delete <uuid> --dry-run
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
