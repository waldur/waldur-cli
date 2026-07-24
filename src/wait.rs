//! Hand-written: generic client-side polling for the `wait` verb every
//! get-able resource gets. Waldur's API has no server-side watch/push
//! mechanism (confirmed nothing in the schema resembles one), so this is a
//! poll-on-an-interval loop -- the same shape as `order.rs`'s marketplace
//! order polling, but evaluating an arbitrary `--jmespath` condition against
//! whatever's fetched, rather than a fixed state enum. Mirrors AWS's named
//! waiters / Azure's `az resource wait --custom` / kubectl's
//! `--for=jsonpath=`, generalized via the JMESPath engine already embedded
//! for `--jmespath`.
//!
//! Not unified with `order.rs::poll_order` into one shared primitive: the
//! two have genuinely different exit semantics (a fixed success/failure
//! state enum plus a follow-up resource fetch, vs. an arbitrary boolean
//! condition with no known failure vocabulary), and async closures make a
//! generic shared loop awkward for what's ultimately one similar-shaped
//! ~30-line loop, not enough duplication to justify the abstraction.

use anyhow::{bail, Result};
use std::time::{Duration, Instant};

use crate::output::OutputFormat;

/// How often to animate the spinner (independent of how often we actually
/// poll) so it stays lively during the gaps between real requests.
const SPINNER_TICK: Duration = Duration::from_millis(100);

/// Polls `path` every `interval_secs` seconds, evaluating `jmespath_expr`
/// against the fetched object each time, until the result is "met" --
/// anything other than `false`/`null`, so both a boolean condition
/// (`state=='OK'`) and a simple presence check (`resource_uuid`) work
/// naturally -- or `timeout_secs` elapses. Prints the final object (using
/// the caller's own resource columns for table/tsv) on success.
#[allow(clippy::too_many_arguments)]
pub async fn wait_for(
    base_url: &str,
    token: Option<&str>,
    path: &str,
    jmespath_expr: &str,
    timeout_secs: u64,
    interval_secs: u64,
    columns: &[&str],
    format: OutputFormat,
) -> Result<()> {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(timeout_secs);
    // A 0 (or absurdly small) --interval would otherwise hammer the API once
    // per spinner tick instead of once per poll.
    let interval = Duration::from_secs(interval_secs.max(1));
    let mut spinner = crate::progress::Spinner::new("Waiting");

    loop {
        let value = crate::http::call_one(base_url, token, reqwest::Method::GET, path, None).await?;
        let condition = crate::query::apply(value.clone(), jmespath_expr)?;

        if condition_met(&condition) {
            spinner.clear();
            return crate::output::print_result(&value, columns, format);
        }

        if Instant::now() >= deadline {
            spinner.clear();
            bail!(
                "timed out after {timeout_secs}s waiting for `{jmespath_expr}` on {path} \
                 (last value: {condition}) -- it may still become true; check again, or retry \
                 with a longer --timeout"
            );
        }

        let next_poll = (Instant::now() + interval).min(deadline);
        let condition_label = condition.to_string();
        while Instant::now() < next_poll {
            spinner.tick(start.elapsed(), &condition_label);
            tokio::time::sleep(SPINNER_TICK).await;
        }
    }
}

fn condition_met(value: &serde_json::Value) -> bool {
    !matches!(value, serde_json::Value::Null | serde_json::Value::Bool(false))
}
