//! Hand-written: request-body input handling for `create`/`update`, in the
//! style of AWS's `--generate-cli-skeleton` / `--cli-input-json`.
//!
//! `create`/`update` take the request body as raw JSON rather than a flag per
//! field (request schemas are large -- e.g. 44 fields for a customer). To
//! keep that discoverable, `--generate-skeleton` prints a fillable template
//! (built from the OpenAPI schema at generation time and embedded as a
//! `const` in each generated command), and `--request-file` reads a filled-in
//! template back from a JSON or YAML file.

use anyhow::{bail, Context, Result};
use std::path::Path;

/// Output format for `--generate-skeleton`.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum SkeletonFormat {
    Json,
    Yaml,
}

/// Prints a request-body template (`template` is already-pretty JSON embedded
/// by the generator) in the requested format, for `--generate-skeleton`.
pub fn print_skeleton(template: &str, format: SkeletonFormat) -> Result<()> {
    match format {
        SkeletonFormat::Json => println!("{template}"),
        SkeletonFormat::Yaml => {
            let value: serde_json::Value = serde_json::from_str(template)
                .context("internal error: embedded skeleton is not valid JSON")?;
            // serde_yaml::to_string already ends with a newline.
            print!(
                "{}",
                serde_yaml::to_string(&value).context("serializing skeleton to YAML")?
            );
        }
    }
    Ok(())
}

/// Resolves the request body to the JSON string to send, from either inline
/// JSON (`--request`) or a JSON/YAML file (`--request-file`). Exactly one is
/// expected -- clap's arg group enforces that -- and YAML is converted to
/// JSON since the wire body is always `application/json`.
///
/// `null`-valued object keys are dropped at every depth: a
/// `--generate-skeleton` template defaults every optional field to `null`, and
/// Waldur's API rejects an explicit `null` for a non-nullable optional field
/// ("This field may not be null") while happily accepting the field being
/// *omitted*. Recursive (not just top-level) so a `provision` order's nested
/// `attributes` object round-trips the same way. So a `null` in the body reads
/// as "leave this unset" -- fill in the fields you want and send the template
/// through as-is. (Null *array elements* are left alone -- removing them would
/// change list contents rather than just omit an unset field.)
pub fn load_body(inline: Option<&str>, file: Option<&Path>) -> Result<String> {
    let raw = match (inline, file) {
        (Some(json), None) => json.to_string(),
        (None, Some(path)) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading request body file {}", path.display()))?;
            // Parse as YAML (a superset of JSON, so this accepts either) into
            // a serde_yaml::Value, then re-serialize as JSON -- going straight
            // to serde_json::Value would hit serde_yaml's incompatibility with
            // serde_json's preserve_order Map (enabled transitively here).
            let value: serde_yaml::Value = serde_yaml::from_str(&text)
                .with_context(|| format!("parsing {} as JSON or YAML", path.display()))?;
            serde_json::to_string(&value).context("re-serializing request body to JSON")?
        }
        _ => bail!("provide exactly one of --request or --request-file"),
    };

    let mut value: serde_json::Value =
        serde_json::from_str(&raw).context("request body is not valid JSON")?;
    strip_null_keys(&mut value);
    serde_json::to_string(&value).context("re-serializing request body to JSON")
}

/// Recursively removes `null`-valued keys from every object, descending into
/// nested objects and array elements. Null array *elements* are preserved.
fn strip_null_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for v in map.values_mut() {
                strip_null_keys(v);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                strip_null_keys(v);
            }
        }
        _ => {}
    }
}
