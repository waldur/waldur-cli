# Development

This page is for contributors to `waldur-cli` itself, not for using the CLI -- see the
[Manual](../README.md#manual) for that.

## Two-repo architecture

`waldur-cli`'s command surface isn't hand-written. It's generated from Waldur's OpenAPI
schema by a separate repo,
[waldur-cli-generator](https://code.opennodecloud.com/waldur/waldur-cli-generator), which
parses the schema directly (paths, params, request/response shapes, validation rules) and
emits this repo's `src/commands/`, `src/cli.rs`, and `src/schema.rs` wholesale. The schema is
the single source of truth end to end -- a field nobody reads drifting out of date, or a
stale request-body type, can't silently break a command, because there's no intermediate
hand-maintained layer for it to drift against.

**Generated, don't edit by hand:**

- `src/commands/` -- every resource's `Args` structs, `Command` enum, and `run()` dispatch
- `src/cli.rs` -- the top-level `openstack`/`team`/`marketplace` group wiring
- `src/schema.rs` -- the CLI's command surface as JSON, embedded for `waldur-cli schema`
  (a machine-readable tool spec for LLM agents)

**Hand-written and permanent** -- everything else in `src/`: `lib.rs`, `main.rs`, `config.rs`,
`output.rs`, `pagination.rs`, `http.rs`, `web.rs`, `request.rs`, `filter.rs`, `query.rs`,
`order.rs`, `wait.rs`, `progress.rs`.

What ends up in the generated surface (which resources, which verbs, which custom actions,
which HomePort routes) is controlled by
[`commands.toml`](https://code.opennodecloud.com/waldur/waldur-cli-generator/-/blob/main/commands.toml)
in the generator repo -- see that repo's README for the manifest format and how to add a
resource or verb.

## Building and testing

```bash
cargo build --locked
cargo test --locked
cargo clippy --all-targets
```

The crate is split into a library (everything except the `Cli`/`main()` entry point in
`main.rs`) and a thin binary, so `tests/*.rs` can exercise the actual logic directly instead
of only being able to shell out to the compiled binary. Networked code (`pagination`, `http`,
`order`, `wait`, `web`) is tested against an in-process HTTP mock
([`wiremock`](https://docs.rs/wiremock)) rather than a live Waldur instance -- see `tests/*.rs`
for examples. `config.rs` tests use [`serial_test`](https://docs.rs/serial_test) plus a
per-test `tempfile::TempDir`/`XDG_CONFIG_HOME` override, since env vars and the config file
path are process-global state that would otherwise race across parallel tests.

GitLab CI (`.gitlab-ci.yml`) runs all three of the commands above on every merge request and
on `main`.

## Regenerating the command surface locally

From a checkout of `waldur-cli-generator`, sitting as a sibling directory to this repo:

```bash
cargo run -- waldur-openapi-schema.yaml ../waldur-cli
```

This overwrites `src/commands/`, `src/cli.rs`, and `src/schema.rs` in place. Review the diff,
then `cargo build --locked && cargo test --locked && cargo clippy --all-targets` here as
usual before committing -- regeneration itself doesn't run those checks.

In CI, this happens automatically: `waldur-cli-generator`'s pipeline fetches the latest schema
from `waldur-mastermind`, regenerates, and pushes the result to this repo's `main` directly
(a "chore: regenerate CLI command surface from OpenAPI schema" commit) whenever the schema
changes -- see that repo's `.gitlab-ci.yml` (`Generate CLI` job).

## Cutting a release

Three files change together (`chore: release vX.Y.Z`):

1. `Cargo.toml` -- bump `version`
2. `Cargo.lock` -- run a plain `cargo build` (not `--locked`) to pick up the new version
3. `src/schema.rs` -- its embedded `"version"` field. Easiest way: regenerate via
   `waldur-cli-generator` again (it reads the version straight out of this repo's
   `Cargo.toml`) -- the diff should be exactly that one field if nothing else changed.

Then tag and push both the commit and the tag to **both** remotes:

```bash
git tag vX.Y.Z
git push origin main && git push origin vX.Y.Z
git push github main && git push github vX.Y.Z
```

### Why two remotes

- `origin` (`code.opennodecloud.com`) -- the internal GitLab, used for MR checks and as the
  push target for `waldur-cli-generator`'s automated regeneration commits.
- `github` (`github.com/waldur/waldur-cli`) -- public, and the only remote that matters for
  releases: pushing a `vX.Y.Z` tag there triggers `.github/workflows/release.yml`
  ([cargo-dist](https://github.com/axodotdev/cargo-dist)-generated), which builds binaries for
  every target in `dist-workspace.toml` and publishes them as a GitHub Release. `origin` has
  no equivalent release pipeline -- pushing the tag there alone does nothing.

`dist-workspace.toml` pins `ci = "github"` and `hosting = "github"`; migrating release
distribution to GitLab isn't a small config flip, it's picking a different release tool
entirely, so this dual-remote setup is intentional rather than a migration in progress.

## `waldur-cli update`

The `update` command (`src/main.rs`'s `run_update`) uses the
[`self_update`](https://docs.rs/self_update) crate against GitHub Releases. One non-obvious
config: `self_update`'s `bin_name()` alone assumes the binary sits at the archive root, but
cargo-dist's tarballs nest it under a `waldur-cli-{target}/` directory -- so
`bin_path_in_archive("waldur-cli-{{ target }}/{{ bin }}")` is required, or extraction fails
with `Could not find the required path in the archive` on every platform, silently (the
command only fails when a user actually runs `update`, not at build or test time). If you
touch `run_update`, verify against a real published release tarball rather than trusting a
mock -- that's what caught this bug in the first place.
