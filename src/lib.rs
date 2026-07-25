//! Library half of the `waldur-cli` binary: every module except the CLI
//! entry point itself (`src/main.rs`'s `Cli`/`Commands` structs and
//! `main()`). Split out so integration tests in `tests/` can reach the
//! actual logic (`pagination`, `http`, `order`, `request`, `config`,
//! `filter`, `query`) instead of only being able to exercise it indirectly
//! by shelling out to the compiled binary.

pub mod cli;
pub mod commands;
pub mod config;
pub mod filter;
pub mod http;
pub mod order;
pub mod output;
pub mod pagination;
pub mod progress;
pub mod query;
pub mod request;
pub mod schema;
pub mod wait;
pub mod web;
