//! Hand-written: backs the `api` escape hatch -- a raw call to any endpoint,
//! for one this CLI hasn't wired up as a typed command yet, or quick
//! one-off debugging. Reuses `http::call_one` (retries, timeout, `--debug`
//! tracing), so it goes through the same transport every generated command
//! does; the only genuinely new logic here is method/path normalization and
//! making the request body optional (create/update's own `--request` is
//! required via a clap `ArgGroup`, but GET/DELETE typically send none).

use crate::output::{self, OutputFormat};
use anyhow::{Context, Result};

/// Normalizes a user-supplied HTTP method string (case-insensitive) into a
/// `reqwest::Method`, erroring clearly on something that isn't a legal HTTP
/// token at all (a stray space, ...) -- not on a method the *server* doesn't
/// recognize (a nonstandard verb some APIs do support), which is a legal
/// request to attempt and should fail server-side instead.
pub fn parse_method(raw: &str) -> Result<reqwest::Method> {
    reqwest::Method::from_bytes(raw.to_uppercase().as_bytes())
        .with_context(|| format!("`{raw}` is not a valid HTTP method"))
}

/// A path relative to the API base: adds a leading slash if missing, so
/// both `/api/customers/` and `api/customers/` work.
pub fn normalize_path(raw: &str) -> String {
    if raw.starts_with('/') {
        raw.to_string()
    } else {
        format!("/{raw}")
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    base_url: &str,
    token: Option<&str>,
    method: &str,
    path: &str,
    request: Option<&str>,
    request_file: Option<&std::path::Path>,
    jmespath: Option<&str>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let method = parse_method(method)?;
    let path = normalize_path(path);
    // Unlike create/update's `--request`, a body here is optional (GET/
    // DELETE typically send none) -- `request::load_body` itself requires
    // exactly one of inline/file, so only call it once at least one was
    // given.
    let body = match (request, request_file) {
        (None, None) => None,
        _ => Some(crate::request::load_body(request, request_file)?),
    };

    if dry_run {
        return output::print_dry_run(method.as_str(), &path, body.as_deref(), format);
    }

    let result = crate::http::call_one(base_url, token, method, &path, body.as_deref()).await?;
    let result = match jmespath {
        Some(expr) => crate::query::apply(result, expr)?,
        None => result,
    };
    print_raw_json(&result, format)
}

/// Prints an arbitrary, schema-less API response. Unlike every generated
/// command, `api` has no known column set to render a table/tsv from, so
/// those fall back to pretty JSON -- the same fallback the `schema` command
/// uses for its own schema-shaped (not row-shaped) output.
fn print_raw_json(value: &serde_json::Value, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Toon => println!("{}", serde_toon::to_string(value)?),
        OutputFormat::Ndjson => match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    if !output::print_ndjson_line(item)? {
                        break; // downstream reader (e.g. `| head`) hung up
                    }
                }
            }
            other => {
                output::print_ndjson_line(other)?;
            }
        },
        OutputFormat::Json | OutputFormat::Table | OutputFormat::Tsv => {
            println!("{}", serde_json::to_string_pretty(value)?)
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_method_is_case_insensitive() {
        assert_eq!(parse_method("get").unwrap(), reqwest::Method::GET);
        assert_eq!(parse_method("Post").unwrap(), reqwest::Method::POST);
        assert_eq!(parse_method("DELETE").unwrap(), reqwest::Method::DELETE);
    }

    #[test]
    fn parse_method_rejects_a_malformed_token() {
        let err = parse_method("bad method").unwrap_err();
        assert!(err.to_string().contains("not a valid HTTP method"));
    }

    #[test]
    fn parse_method_accepts_a_nonstandard_verb() {
        // Not every method the *server* rejects should be rejected
        // client-side -- extension methods are legal HTTP tokens, so this
        // should fail server-side (a 501, say), not here.
        assert!(parse_method("PURGE").is_ok());
    }

    #[test]
    fn normalize_path_adds_a_leading_slash_when_missing() {
        assert_eq!(normalize_path("api/customers/"), "/api/customers/");
    }

    #[test]
    fn normalize_path_leaves_an_existing_leading_slash_alone() {
        assert_eq!(normalize_path("/api/customers/"), "/api/customers/");
    }
}
