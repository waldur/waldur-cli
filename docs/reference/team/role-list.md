# `waldur-cli team role list`

List roles.

## Usage

```bash
waldur-cli team role list [OPTIONS]
```

| Flag | Type | Description |
| --- | --- | --- |
| `--filter KEY=VALUE` | repeatable | Server-side filter. Valid keys: `description` (string), `is_active` (boolean), `name` (string). |
| `--fields FIELDS` | string | Fetch only these fields from the server (comma-separated). Valid: content_type, description, description_ar, description_cs, description_da, description_de, description_en, description_es, description_et, description_fr, description_it, description_lt, description_lv, description_nb, description_ru, description_sv, is_active, is_system_role, name, permissions, users_count, uuid. |
| `--jmespath EXPR` | string | Reshape the already-fetched result client-side (https://jmespath.org). |
| `--limit N` | integer | Stop after this many items. |
| `--format FORMAT` | string | table, json, tsv, toon, or ndjson. |

## Examples

```bash
waldur-cli team role list --filter is_active=true --fields uuid,name --format json
```

Project just the columns you need, client-side:

```bash
waldur-cli team role list --jmespath '[].[uuid, name]'
```

## Global options

Every command also accepts `--api-url`, `--token`, `--profile`, `--format`, and `--debug`; mutating commands additionally accept `--dry-run`. See [Getting started](../../1-getting-started.md) for what each does.
