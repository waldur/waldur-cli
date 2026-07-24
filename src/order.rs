//! Hand-written: Waldur marketplace-order provisioning, the async flow that
//! OpenStack tenant/instance/volume creation and deletion go through instead
//! of a direct REST create/delete.
//!
//! Provision: POST the order to `/api/marketplace-orders/`, then poll it to a
//! terminal state and fetch the resulting resource. Terminate: POST to the
//! marketplace resource's `terminate/` action (which itself returns an order)
//! and poll that. Both reuse `http::call_one`, so `--debug` request tracing
//! covers them like every other verb.

use anyhow::{bail, Context, Result};
use std::time::{Duration, Instant};

use crate::output::OutputFormat;

/// How often to re-poll an in-flight order.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Columns for printing an order object under `--no-wait` (the order itself,
/// not yet a provisioned resource).
const ORDER_COLUMNS: &[&str] = &["uuid", "state", "resource_uuid", "error_message"];

/// Generic columns for a provisioned/looked-up resource, whose concrete type
/// varies by offering -- json/toon still show the complete object.
const RESOURCE_COLUMNS: &[&str] = &["uuid", "name", "state"];

/// Submits a marketplace order and, unless `wait` is false, polls it to
/// completion and prints the provisioned resource. `project` is the ambient
/// `--project` scope (a UUID): if the order body doesn't already name a
/// project, it's filled in from this, since every order requires one.
#[allow(clippy::too_many_arguments)]
pub async fn provision(
    base_url: &str,
    token: Option<&str>,
    body: &str,
    project: Option<&str>,
    dry_run: bool,
    wait: bool,
    timeout_secs: u64,
    format: OutputFormat,
) -> Result<()> {
    let body = apply_project(body, base_url, project)?;
    if dry_run {
        return crate::output::print_dry_run("POST", "/api/marketplace-orders/", Some(&body), format);
    }
    let order = crate::http::call_one(
        base_url,
        token,
        reqwest::Method::POST,
        "/api/marketplace-orders/",
        Some(&body),
    )
    .await?;
    let order_uuid = order_uuid(&order)?;

    if !wait {
        crate::output::print_result(&order, ORDER_COLUMNS, format)?;
        return Ok(());
    }

    poll_order(base_url, token, &order_uuid, timeout_secs).await?;
    // Fetch the actual provisioned resource the order created.
    let resource = crate::http::call_one(
        base_url,
        token,
        reqwest::Method::GET,
        &format!("/api/marketplace-orders/{order_uuid}/resource/"),
        None,
    )
    .await?;
    crate::output::print_result(&resource, RESOURCE_COLUMNS, format)?;
    Ok(())
}

/// Terminates a marketplace resource (by its `marketplace_resource_uuid`) and,
/// unless `wait` is false, polls the resulting termination order to completion.
#[allow(clippy::too_many_arguments)]
pub async fn terminate(
    base_url: &str,
    token: Option<&str>,
    resource_uuid: &str,
    attributes: Option<&str>,
    dry_run: bool,
    wait: bool,
    timeout_secs: u64,
    format: OutputFormat,
) -> Result<()> {
    // The terminate action accepts an optional ResourceTerminateRequest body
    // ({attributes: {...}}). When the caller passes raw termination attributes,
    // wrap them; otherwise send an empty body.
    let body = match attributes {
        Some(attrs) => {
            let value: serde_json::Value =
                serde_json::from_str(attrs).context("--request is not valid JSON")?;
            serde_json::json!({ "attributes": value }).to_string()
        }
        None => "{}".to_string(),
    };

    let path = format!("/api/marketplace-resources/{resource_uuid}/terminate/");
    if dry_run {
        return crate::output::print_dry_run("POST", &path, Some(&body), format);
    }

    let response = crate::http::call_one(
        base_url,
        token,
        reqwest::Method::POST,
        &path,
        Some(&body),
    )
    .await?;
    // Terminate returns {"order_uuid": "..."}.
    let order_uuid = response
        .get("order_uuid")
        .and_then(|v| v.as_str())
        .context("terminate response did not include an order_uuid")?
        .to_string();

    if !wait {
        crate::output::print_result(&response, &["order_uuid"], format)?;
        return Ok(());
    }

    poll_order(base_url, token, &order_uuid, timeout_secs).await?;
    match format {
        OutputFormat::Json => println!("{}", serde_json::json!({"terminated": true, "uuid": resource_uuid})),
        OutputFormat::Table => println!("Terminated {resource_uuid}"),
        OutputFormat::Tsv => println!("true\t{resource_uuid}"),
        OutputFormat::Toon => println!(
            "{}",
            serde_toon::to_string(&serde_json::json!({"terminated": true, "uuid": resource_uuid}))?
        ),
    }
    Ok(())
}

/// Fills a `project` URL into the order body from the ambient `--project`
/// scope, unless the body already names a project (an explicit one wins).
/// The order API takes a project URL; we build it from the UUID + base URL,
/// consistent with how the rest of the CLI takes raw UUIDs/URLs.
fn apply_project(body: &str, base_url: &str, project: Option<&str>) -> Result<String> {
    let Some(uuid) = project else {
        return Ok(body.to_string());
    };
    let mut value: serde_json::Value =
        serde_json::from_str(body).context("request body is not valid JSON")?;
    if let Some(object) = value.as_object_mut() {
        if !object.contains_key("project") {
            object.insert(
                "project".to_string(),
                serde_json::Value::String(format!("{base_url}/api/projects/{uuid}/")),
            );
        }
    }
    serde_json::to_string(&value).context("re-serializing request body to JSON")
}

fn order_uuid(order: &serde_json::Value) -> Result<String> {
    Ok(order
        .get("uuid")
        .and_then(|v| v.as_str())
        .context("order response did not include a uuid")?
        .to_string())
}

/// Polls an order until it reaches a terminal state, returning the final
/// order object on success or erroring on a failure state / timeout.
async fn poll_order(
    base_url: &str,
    token: Option<&str>,
    order_uuid: &str,
    timeout_secs: u64,
) -> Result<serde_json::Value> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let path = format!("/api/marketplace-orders/{order_uuid}/");
    loop {
        let order =
            crate::http::call_one(base_url, token, reqwest::Method::GET, &path, None).await?;
        let state = order.get("state").and_then(|v| v.as_str()).unwrap_or("");
        match state {
            // Terminal success.
            "done" => return Ok(order),
            // Terminal failures (Waldur's OrderState enum).
            "erred" | "canceled" | "rejected" => {
                let msg = order
                    .get("error_message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                bail!("marketplace order {order_uuid} {state}: {msg}");
            }
            // Still pending-*/executing -- keep waiting.
            _ => {}
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out after {timeout_secs}s waiting for marketplace order {order_uuid} \
                 (last state: {state:?}) -- it may still complete; check with \
                 `waldur-cli` against /api/marketplace-orders/, or retry with a longer --timeout"
            );
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
