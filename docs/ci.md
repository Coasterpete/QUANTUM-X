# Continuous integration (R0.1 — CPU-only baseline)

The workflow is in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml).
It implements Tier 1 of the approved roadmap
([production-readiness-v1.md](roadmaps/production-readiness-v1.md) §7.1).

All jobs run on the standard GitHub-hosted `windows-2025` runner, which has no
GPU. No check creates a window or graphics device, and none needs a software
adapter.

## When it runs

- **Pull requests that target `main`.** A newer push to the same pull request
  cancels the older run.
- **Pushes to `main`,** including merge commits. These runs are never
  cancelled.

Permissions are read-only (`contents: read`). The workflow uses no secrets and
no self-hosted runners. Both actions are pinned to full commit SHAs.

## Jobs

| Job | Command(s) | Timeout |
| --- | --- | --- |
| Format (cargo fmt) | `cargo fmt --all -- --check` | 15 min |
| Clippy and tests (debug) | `cargo clippy --workspace --all-targets --locked -- -D warnings`, then `cargo test --workspace --locked --no-fail-fast` | 90 min |
| Release build | `cargo build --workspace --release --locked` | 60 min |

The three jobs run in parallel, each on its own runner with its own disk.

The test step runs even when clippy fails, so one push reports both results.

## Toolchain

[`rust-toolchain.toml`](../rust-toolchain.toml) pins **Rust 1.100.0** with
`rustfmt` and `clippy`. Rustup applies the same pin locally, so the laptop and
CI use the same compiler. Change the pin only in its own pull request, after
the full suite passes on the new version.

## Line endings

[`.gitattributes`](../.gitattributes) sets `* text=auto` and marks binary asset
types.

- **Repository:** text files are stored with LF, as they already were. Adding
  the file changed no stored bytes.
- **Working copies:** checkout conversion follows each clone's `core.autocrlf`.
  - CI sets `core.autocrlf=true` before checkout. Its working copy therefore
    matches the owner's Windows laptop, which also uses CRLF.
  - Tests are verified only under that configuration. An LF working copy has
    not been verified.
- **Exception:** `docs/research/m4i-claude/` keeps its own `-text` rule, which
  preserves archival bytes.

## Caching and disk

**Cache.** `Swatinem/rust-cache` caches the Cargo registry and dependency build
outputs.

- There is one cache for debug and one for release.
- Only runs on `main` save the cache. Pull requests only restore it. This
  caps cache churn within the repository's cache quota.

**Disk and build settings.**

- `CARGO_INCREMENTAL=0` avoids incremental artifacts.
- `CARGO_PROFILE_DEV_DEBUG=line-tables-only` shrinks debug info, while keeping
  file and line backtraces. Optimization and test behavior are unchanged.
- Each job logs free disk space so runner limits can be watched.

## What CI does **not** cover

- **Anything GPU-related.**
  - GPU smoke tests (`--gui-smoke-test`), offscreen render scenarios and
    rendered-image verifiers.
  - Tests marked `#[ignore]`, such as `gui_smoke` and the long release-mode
    physics controls.
  - Environment-gated render tests.

  These stay Tier 3 local or owner-hardware checks. A software-GPU trial
  (WARP/Mesa) is a later, non-blocking R0 step.
- **Timing and performance gates** (`simulator-timing`, step budgets, frame
  rate). Shared runners are not timing-stable.
- **The Python numerical verifiers and packaging** (`package-windows.ps1`).
  These are later R0 steps.

Making CI a required check is a repository setting. The owner turns it on in
branch protection once the workflow has a reliable green record.

## Reproduce locally (laptop)

Run from the workspace root. Rustup selects the pinned toolchain
automatically.

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --no-fail-fast
cargo build --workspace --release --locked
```

To match CI exactly, set the following first:

```powershell
$env:CARGO_INCREMENTAL = '0'
$env:CARGO_PROFILE_DEV_DEBUG = 'line-tables-only'
```

This rebuilds the debug profile once.

## Test isolation

Tests must not depend on the developer's own application data.

- The startup tests use default workbench preferences, as the offscreen
  diagnostics do.
- Recent-project history is kept in a per-test temporary folder.
- A real `%LOCALAPPDATA%\QUANTUM-X` profile therefore no longer changes their
  starting state.
