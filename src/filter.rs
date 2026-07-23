//! Hand-written: parses repeated `--filter KEY=VALUE` flags into query
//! params for `list` commands (AWS `--filters`/kubectl `--field-selector`
//! style), replacing a dedicated flag per query field. `spec` (embedded by
//! the generator per resource, from the OpenAPI schema) gives the valid keys
//! and their types, so a bad key or a wrongly-typed value (e.g. `archived=
//! maybe`) is rejected locally instead of round-tripping to the server for a
//! 400.

use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug)]
pub enum FilterKind {
    Str,
    Bool,
    I64,
}

/// Parses and validates `--filter` values against `spec`. `raw` is the
/// flag's raw `KEY=VALUE` strings, one per `--filter` occurrence.
pub fn parse_filters(raw: &[String], spec: &[(&str, FilterKind)]) -> Result<Vec<(String, String)>> {
    let mut params = Vec::with_capacity(raw.len());
    for entry in raw {
        let Some((key, value)) = entry.split_once('=') else {
            bail!("invalid --filter `{entry}` -- expected KEY=VALUE");
        };
        let Some((_, kind)) = spec.iter().find(|(name, _)| *name == key) else {
            let valid: Vec<&str> = spec.iter().map(|(name, _)| *name).collect();
            bail!("unknown filter key `{key}` -- valid keys: {}", valid.join(", "));
        };
        match kind {
            FilterKind::Bool => {
                if value != "true" && value != "false" {
                    bail!("invalid --filter `{key}={value}` -- expected true or false");
                }
            }
            FilterKind::I64 => {
                if value.parse::<i64>().is_err() {
                    bail!("invalid --filter `{key}={value}` -- expected an integer");
                }
            }
            FilterKind::Str => {}
        }
        params.push((key.to_string(), value.to_string()));
    }
    Ok(params)
}
