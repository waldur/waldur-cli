# `waldur-cli openstack floating-ip wait`

Poll openstack floating ips until a `--jmespath` condition against it stops evaluating to `false`/`null`, or error on timeout. Client-side polling -- Waldur's API has no server-side watch/push mechanism.

## Usage

```bash
waldur-cli openstack floating-ip wait <UUID> --jmespath EXPR [OPTIONS]
```

| Flag | Type | Description |
| --- | --- | --- |
| `<UUID>` | positional, required | uuid of the resource. |
| `--jmespath EXPR` | string, required | Condition to poll for, evaluated against the fetched object on every poll. |
| `--timeout N` | integer | Seconds to wait for the condition before giving up (default 600). |
| `--interval N` | integer | Seconds between polls (default 3). |

## Examples

```bash
waldur-cli openstack floating-ip wait <uuid> --jmespath "state=='OK'"
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
