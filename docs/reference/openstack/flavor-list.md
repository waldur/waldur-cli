# `waldur-cli openstack flavor list`

List openstack flavors (vm sizes).

## Usage

```bash
waldur-cli openstack flavor list [OPTIONS]
```

| Flag | Type | Description |
| --- | --- | --- |
| `--filter KEY=VALUE` | repeatable | Server-side filter. Valid keys: `cores` (integer), `cores__gte` (integer), `cores__lte` (integer), `disk` (integer), `disk__gte` (integer), `disk__lte` (integer), `name` (string), `name_exact` (string), `name_iregex` (string), `offering_uuid` (string), `ram` (integer), `ram__gte` (integer), `ram__lte` (integer), `settings` (string), `settings_uuid` (string), `tenant` (string), `tenant_uuid` (string). |
| `--fields FIELDS` | string | Fetch only these fields from the server (comma-separated). Valid: backend_id, cores, disk, display_name, name, ram, settings, url, uuid. |
| `--order FIELDS` | string | Sort server-side (comma-separated, `-` prefix for descending). Valid: -cores, -disk, -ram, cores, disk, ram. |
| `--jmespath EXPR` | string | Reshape the already-fetched result client-side (https://jmespath.org). |
| `--limit N` | integer | Stop after this many items. |
| `--format FORMAT` | string | table, json, tsv, toon, or ndjson. |

## Examples

```bash
waldur-cli openstack flavor list --filter name_exact=example --fields uuid,name --format json
```

Project just the columns you need, client-side:

```bash
waldur-cli openstack flavor list --jmespath '[].[uuid, name]'
```

Smallest/first result matching a filter, sorted server-side:

```bash
waldur-cli openstack flavor list --order cores --limit 1
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
