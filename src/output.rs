//! Shared rendering used by every generated command in `src/commands/`.

use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Table};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table (default).
    Table,
    /// Machine-readable JSON, for scripted/agent use.
    Json,
    /// Tab-separated values, one row per line, no header -- for shell loops
    /// (`while IFS=$'\t' read -r ...`) and `cut`/`awk` pipelines.
    Tsv,
    /// TOON (Token-Oriented Object Notation, https://toonformat.dev):
    /// full/lossless like json, but far fewer tokens for uniform arrays of
    /// objects -- field names are declared once in a header instead of
    /// repeated per row. Unlike table/tsv this is NOT limited to `columns`;
    /// it serializes the complete result.
    Toon,
    /// Newline-delimited JSON: one compact JSON object per line, like json
    /// but streamed. On `list`, results are printed as each page arrives
    /// instead of buffering the complete result set first -- lower memory,
    /// and a consumer (`jq`, a pipe, an agent) can start processing before
    /// the fetch finishes. Falls back to buffer-then-print (still one
    /// object per line) when combined with `--jmespath`, which needs the
    /// complete result to reshape.
    Ndjson,
}

/// Print a single object or a list of objects, either as a table (using
/// `columns` to pick and order fields), pretty JSON, or TOON.
///
/// Works generically on anything `Serialize`: rs-client's list methods
/// return `Vec<T>` (aliased), retrieve/create/update return a bare `T`. We
/// convert to `serde_json::Value` first and branch on whether it's an array,
/// rather than requiring callers to know which shape their particular
/// method returns.
pub fn print_result<T: Serialize>(value: &T, columns: &[&str], format: OutputFormat) -> Result<()> {
    let json = serde_json::to_value(value)?;
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&json)?),
        OutputFormat::Table => print_table(&json, columns),
        OutputFormat::Tsv => print_tsv(&json, columns),
        // Full result, not just `columns` -- table/tsv are deliberately
        // curated for human/shell scanning, but toon (like json) is meant
        // to be a complete, lossless representation an agent can rely on.
        OutputFormat::Toon => println!("{}", serde_toon::to_string(&json)?),
        // Reached for get/create/update/delete (a single object -- one
        // line), and for `list --jmespath` (the streaming fast path in
        // codegen only applies without --jmespath, since a JMESPath
        // expression can reshape/aggregate across the whole array and so
        // needs it fully fetched first). Either way, `list`'s own
        // buffer-then-stream call site never reaches this arm.
        OutputFormat::Ndjson => match &json {
            serde_json::Value::Array(items) => {
                for item in items {
                    if !print_ndjson_line(item)? {
                        break; // downstream reader (e.g. `| head`) hung up
                    }
                }
            }
            other => {
                print_ndjson_line(other)?;
            }
        },
    }
    Ok(())
}

/// Prints one compact-JSON NDJSON line to stdout. Returns `Ok(false)`
/// (not an error) if the downstream reader has closed the pipe (e.g.
/// `| head`), so callers -- especially the page-at-a-time streaming loop
/// in `pagination::fetch_all_streaming` -- know to stop producing more
/// output (and, for streaming, stop fetching further pages nobody will
/// read) instead of panicking on the next `println!`.
pub(crate) fn print_ndjson_line(value: &serde_json::Value) -> Result<bool> {
    use std::io::Write;
    let line = serde_json::to_string(value)?;
    match writeln!(std::io::stdout(), "{line}") {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Prints the request a mutating command *would* send under `--dry-run`,
/// instead of sending it. Respects `--format` so an agent parsing structured
/// output gets a structured preview: json/toon emit a
/// `{dry_run, method, path, body}` object; table/tsv print a human line (plus
/// the pretty body under table). Goes to stdout -- a dry run is a successful,
/// non-destructive outcome.
pub fn print_dry_run(
    method: &str,
    path: &str,
    body: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    // Parse the body so structured output nests it as JSON rather than a
    // string; fall back to null if it's absent (delete) or somehow not JSON.
    let body_value = match body {
        Some(b) => serde_json::from_str(b).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    match format {
        OutputFormat::Json => {
            let obj = serde_json::json!({
                "dry_run": true, "method": method, "path": path, "body": body_value,
            });
            println!("{}", serde_json::to_string_pretty(&obj)?);
        }
        OutputFormat::Toon => {
            let obj = serde_json::json!({
                "dry_run": true, "method": method, "path": path, "body": body_value,
            });
            println!("{}", serde_toon::to_string(&obj)?);
        }
        OutputFormat::Ndjson => {
            let obj = serde_json::json!({
                "dry_run": true, "method": method, "path": path, "body": body_value,
            });
            print_ndjson_line(&obj)?;
        }
        OutputFormat::Table => {
            println!("DRY RUN -- would send: {method} {path}");
            if !body_value.is_null() {
                println!("{}", serde_json::to_string_pretty(&body_value)?);
            }
        }
        OutputFormat::Tsv => println!("dry_run\t{method}\t{path}"),
    }
    Ok(())
}

fn print_tsv(json: &serde_json::Value, columns: &[&str]) {
    let rows: Vec<&serde_json::Value> = match json {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Null => Vec::new(),
        other => vec![other],
    };

    for row in &rows {
        let cells: Vec<String> = columns
            .iter()
            .map(|col| tsv_escape(&cell_text(row, col)))
            .collect();
        println!("{}", cells.join("\t"));
    }
}

/// A tab or newline inside a cell would corrupt TSV's one-row-per-line,
/// tab-delimited structure -- there's no standard escaping/quoting
/// mechanism the way CSV has, so replace with spaces rather than emit
/// output a naive line-based parser would misread.
fn tsv_escape(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn print_table(json: &serde_json::Value, columns: &[&str]) {
    let rows: Vec<&serde_json::Value> = match json {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Null => Vec::new(),
        other => vec![other],
    };

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(columns);

    for row in &rows {
        let cells: Vec<String> = columns.iter().map(|col| cell_text(row, col)).collect();
        table.add_row(cells);
    }

    println!("{table}");
    if rows.is_empty() {
        println!("(no results)");
    }
}

fn cell_text(row: &serde_json::Value, column: &str) -> String {
    match row.get(column) {
        None | Some(serde_json::Value::Null) => "-".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}
