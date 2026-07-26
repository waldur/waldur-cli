//! Hand-written: resolves the UUID(s) a batch-capable command (`delete`, or
//! a bodyless action like `stop`) should operate on. Waldur's API has no
//! bulk endpoints, so a batch is just this CLI looping over single-item
//! calls -- but reading the UUIDs from stdin when none are given as
//! arguments is what makes `waldur-cli ... list --format ndjson | ... |
//! waldur-cli ... delete` compose without an intermediate `jq -r .uuid`:
//! each stdin line may be a bare UUID or a JSON object with a `uuid` field,
//! so piping `list`'s own ndjson output straight in works unmodified.

use anyhow::{Context, Result};
use std::io::{BufRead, IsTerminal};

/// Returns `explicit` unchanged if it's non-empty (UUIDs given as
/// arguments); otherwise reads them from stdin. Errors if stdin is a
/// terminal and no arguments were given -- a bare `delete` with nothing to
/// operate on and nothing piped in is almost certainly a forgotten
/// argument, not an intentional "read forever" wait.
pub fn resolve_uuids(explicit: Vec<String>) -> Result<Vec<String>> {
    if !explicit.is_empty() {
        return Ok(explicit);
    }
    if std::io::stdin().is_terminal() {
        anyhow::bail!(
            "no UUIDs given, and stdin is a terminal -- pass UUID(s) as arguments, or pipe \
             them in one per line (a bare UUID, or a JSON object with a `uuid` field, e.g. \
             `list --format ndjson`'s own output)"
        );
    }
    parse_uuid_lines(std::io::stdin().lock())
}

/// The parsing logic behind `resolve_uuids`, split out so tests can feed it
/// a `Cursor` instead of going through a real (and untestably-a-terminal-or-
/// not) stdin.
fn parse_uuid_lines(reader: impl BufRead) -> Result<Vec<String>> {
    let mut uuids = Vec::new();
    for line in reader.lines() {
        let line = line.context("reading a UUID from stdin")?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let uuid = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(serde_json::Value::Object(obj)) => obj
                .get("uuid")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .with_context(|| format!("stdin line has no string `uuid` field: {line}"))?,
            _ => line.to_string(),
        };
        uuids.push(uuid);
    }
    Ok(uuids)
}

/// Reports one batch item's failure to stderr and keeps going -- a single
/// bad UUID in a batch of 50 shouldn't abort the other 49. The generated
/// caller tracks failures and exits non-zero once the batch is done if any
/// of these were printed.
pub fn report_error(uuid: &str, err: &anyhow::Error) {
    eprintln!("error: {uuid}: {err:#}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn resolve_uuids_returns_explicit_args_unchanged_without_touching_stdin() {
        let result = resolve_uuids(vec!["a".to_string(), "b".to_string()]).unwrap();
        assert_eq!(result, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parse_uuid_lines_reads_bare_uuids_one_per_line() {
        let input = Cursor::new(b"uuid-1\nuuid-2\nuuid-3\n".to_vec());
        let uuids = parse_uuid_lines(input).unwrap();
        assert_eq!(uuids, vec!["uuid-1".to_string(), "uuid-2".to_string(), "uuid-3".to_string()]);
    }

    #[test]
    fn parse_uuid_lines_skips_blank_lines() {
        let input = Cursor::new(b"uuid-1\n\n\nuuid-2\n".to_vec());
        let uuids = parse_uuid_lines(input).unwrap();
        assert_eq!(uuids, vec!["uuid-1".to_string(), "uuid-2".to_string()]);
    }

    #[test]
    fn parse_uuid_lines_extracts_uuid_field_from_ndjson_objects() {
        let input = Cursor::new(
            b"{\"uuid\": \"abc-1\", \"name\": \"thing one\"}\n{\"uuid\": \"abc-2\", \"name\": \"thing two\"}\n"
                .to_vec(),
        );
        let uuids = parse_uuid_lines(input).unwrap();
        assert_eq!(uuids, vec!["abc-1".to_string(), "abc-2".to_string()]);
    }

    #[test]
    fn parse_uuid_lines_supports_a_mix_of_bare_uuids_and_ndjson_objects() {
        let input = Cursor::new(b"bare-uuid\n{\"uuid\": \"json-uuid\"}\n".to_vec());
        let uuids = parse_uuid_lines(input).unwrap();
        assert_eq!(uuids, vec!["bare-uuid".to_string(), "json-uuid".to_string()]);
    }

    #[test]
    fn parse_uuid_lines_errors_clearly_on_an_object_with_no_uuid_field() {
        let input = Cursor::new(b"{\"name\": \"no uuid here\"}\n".to_vec());
        let err = parse_uuid_lines(input).unwrap_err();
        assert!(err.to_string().contains("no string `uuid` field"));
    }

    #[test]
    fn parse_uuid_lines_treats_a_json_array_line_as_an_opaque_literal() {
        // Not an object, so it falls through to "use the line verbatim" --
        // this will fail downstream as an invalid UUID, but that's a clearer
        // failure than silently guessing at array semantics here.
        let input = Cursor::new(b"[1, 2, 3]\n".to_vec());
        let uuids = parse_uuid_lines(input).unwrap();
        assert_eq!(uuids, vec!["[1, 2, 3]".to_string()]);
    }
}
