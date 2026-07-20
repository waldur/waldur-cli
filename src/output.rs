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
}

/// Print a single object or a list of objects, either as a table (using
/// `columns` to pick and order fields) or as pretty JSON.
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
    }
    Ok(())
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
