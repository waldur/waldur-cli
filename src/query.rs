//! Hand-written: `--query` client-side output projection via JMESPath
//! (AWS CLI's `--query`), applied to a `list` command's already-fetched
//! result before rendering. Distinct from server-side `--filter`: `--filter`
//! reduces what's fetched (fewer requests, less data over the wire);
//! `--query` reshapes/narrows what's already been fetched (e.g. `[].name`,
//! or `[?blocked==\`true\`]`), for scripting or trimming agent context.

use anyhow::{Context, Result};

pub fn apply(value: serde_json::Value, expression: &str) -> Result<serde_json::Value> {
    let compiled =
        jmespath::compile(expression).map_err(|e| anyhow::anyhow!("invalid --query expression: {e}"))?;
    let result = compiled
        .search(&value)
        .map_err(|e| anyhow::anyhow!("--query evaluation failed: {e}"))?;
    serde_json::to_value(&result).context("failed to convert --query result to JSON")
}
