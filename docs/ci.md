# Continuous integration (R0.1–R0.3 — checks, Windows packages and regression baselines)

R0.4 adds a **separate, non-blocking**
[software GPU feasibility experiment](software-gpu-r0.4.md). Nothing in the
Tier 1 workflow described below installs or selects a software adapter.

R0.5 adds [native desktop verification](desktop-verification-r0.5.md) for an
already extracted package on either owner computer, without rebuilding. Its
CPU-only script tests run in the Format job under Windows PowerShell 5.1 and
PowerShell 7; the actual hardware smokes and manual checks remain Tier 3.

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
no self-hosted runners. All actions are pinned to full commit SHAs. Artifact
upload uses the runner's Actions service authorization; it needs no release or
repository write permission.

## Jobs

| Job | Command(s) | Timeout |
| --- | --- | --- |
| Format (cargo fmt) | `cargo fmt --all -- --check`, `./tools/test-windows-packaging.ps1` (no compilation) | 15 min |
| Clippy and tests (debug) | `cargo clippy --workspace --all-targets --locked -- -D warnings`, then `cargo test --workspace --locked --no-fail-fast` | 90 min |
| Deterministic regression (CPU only) | `./tools/verify-regression-baseline.ps1 -Comparison Exact`: immutable schema corpus, exact debug/Release production replay | 20 min |
| Release build and Windows package | `./tools/build-windows.ps1`, then `./tools/package-windows.ps1 -BuildReceipt ...`, then one artifact upload | 60 min |

The four jobs run in parallel, each on its own runner with its own disk.

The test step runs even when clippy fails, so one push reports both results.

## Automated portable Windows package (R0.2)

The release job produces one verified development package per completed workflow
commit, without invoking Cargo a second time during packaging. It does not create
a GitHub Release, installer, signature, or updater. Nothing compiled is committed
to git. Packaging checks are CPU-only: no window, graphics device, WARP or Mesa.

1. `tools/build-windows.ps1` runs **one**
   `cargo build --workspace --release --locked --target x86_64-pc-windows-msvc --message-format=json-render-diagnostics`.
   The native Windows target uses `-C target-feature=+crt-static`, so the MSVC C
   runtime is linked into the application. Target flags do not affect host
   build scripts/procedural macros. See the
   [Rust runtime linkage reference](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes)
   and [Cargo flag rules](https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags).
2. It records Cargo's exact application artifact and SHA-256 in a build receipt
   under the runner's temporary directory. It embeds `build-info.json` in the
   application via the build script, including a fresh build ID, full source
   commit/tree, application version, rustc verbose output, Cargo version, target,
   profile, flags, UTC build time and workflow/runner identifiers. `--build-info`
   prints this metadata without a GUI. Ordinary Cargo builds report
   `verified_build: false` and cannot be reused by the packager.
3. `package-windows.ps1 -BuildReceipt <path>` invokes **no Cargo command**. It
   requires a clean tracked checkout, no untracked build/package inputs, a
   matching commit/tree, an unchanged executable hash, matching embedded build
   metadata and Windows product/version resources. Source identity and hashes
   are checked again before completing the package. Unrelated untracked files,
   including `startup-log.txt`, are preserved. A stale/missing receipt, changed
   binary, different checkout or uncommitted source causes an actionable error.
   These checks protect against accidental stale/unrelated binaries; they are
   not signed supply-chain attestations.
4. It creates `QUANTUM Dynamics-<version>-<12-character-commit>-windows` under
   the runner's temporary `distribution` directory (local fallback:
   `target/distribution`), refusing an existing destination. It stays outside
   the Cargo target/cache directories. The package contains
   **QUANTUM Dynamics.exe**, its identical development compatibility copy
   **quantum-x.exe**, product identity, runtime assets, existing sample projects,
   project/embedded-asset license notices, README, build metadata and check reports.
   Windows runtime imports are inspected to reject external Visual C++ runtime
   dependencies. No installed Rust or Cargo is needed to run the distribution.
5. The existing icon checks (both PEs), `--version`, catalog/asset `--self-check`,
   development CLI help, Welcome/no-document startup, direct editor startup and
   both explicit project-file routes run against the packaged files. Asset
   overrides are cleared and checks run from the package directory, so a source
   checkout cannot supply missing assets.
6. `SHA256SUMS` lists every package file except itself, including the verifier,
   build metadata, reports and both executables. It uses lowercase SHA-256,
   forward-slash relative paths, ordinal sorting, two spaces between hash and
   path, and UTF-8 without BOM with LF endings. Given identical file bytes, the
   manifest is deterministic. Build time/ID, PE bytes and runner paths may differ
   between builds; this does **not** claim reproducible binary builds.
   `verify-package.ps1` verifies hashes, rejects missing/extra files, duplicate or
   unsafe paths and links, and requires deterministic ordering.
7. Only successful packaging reaches `actions/upload-artifact`. It creates one
   ZIP with the versioned folder inside, using **14-day retention**. Actions
   compresses directly; the job does not create another local ZIP or distribution.
   The run summary includes the download link, uncompressed byte count, manifest
   hash and archive digest. Artifact metadata shows the compressed size.

For PRs, `actions/checkout` tests GitHub's synthetic **merge commit**, not just
the branch tip. The package name and `source_commit` identify that actual checked
out merge SHA; `workflow_sha` records `GITHUB_SHA`. The PR head SHA can differ.
On `main`, the package identifies the pushed/merged commit. A newer PR push can
cancel an older run before it uploads; failed/cancelled packaging has no artifact.
Successful release packaging may upload while another parallel job fails: use a
package as a verified candidate only after **all four jobs are green**. Rerunning
the upload for the same workflow/name does not overwrite an existing artifact.

### Download, extract and verify (either Windows computer)

1. Sign into GitHub. Open [CI workflow runs](https://github.com/Coasterpete/QUANTUM-X/actions/workflows/ci.yml),
   choose the intended PR/commit and confirm all jobs succeeded.
2. Under **Artifacts**, download `QUANTUM Dynamics-<version>-<commit>-windows`.
   Extract the **entire ZIP** to a writable local folder; do not launch from the
   ZIP or copy only an executable. Keep `assets` beside both executables.
   Windows x64 with a compatible graphics driver is required.
3. Open PowerShell in the extracted versioned folder. Before first launch:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-package.ps1
   Get-Content .\build-info.json
   & '.\QUANTUM Dynamics.exe' --build-info
   & '.\QUANTUM Dynamics.exe' --version
   & '.\QUANTUM Dynamics.exe' --self-check
   & '.\QUANTUM Dynamics.exe' --startup-check
   ```

   Expect `verified: True`, matching source/build metadata, a successful catalog
   report and `page: welcome`, `document_open: false`. These checks need only
   built-in Windows PowerShell 5.1+ and the package. The per-command execution
   policy bypass does not change machine policy; organizational restrictions may
   still prevent scripts or unsigned applications. Hashes detect corruption,
   not publisher authenticity. Compare the manifest hash with the run summary.
   Verify immediately after extraction; adding files or saving projects **inside**
   this distribution will subsequently be reported as extra/changed contents.
4. Double-click `QUANTUM Dynamics.exe`, create/open a project, then save and
   reopen it in a separate projects folder. The development compatibility name
   `quantum-x.exe` remains available. This unsigned development build still has
   the existing console window; Windows/SmartScreen may warn. GUI/GPU startup,
   visual quality and hardware performance require owner checks on each machine
   and are **not** verified by R0.2.

The optional GitHub CLI download is:

```powershell
gh run download RUN_ID --repo Coasterpete/QUANTUM-X --name 'ARTIFACT_NAME' --dir 'C:\Downloads\QuantumBuild'
```

This downloads and extracts directly. GitHub CLI is optional for downloading;
it is not an application runtime dependency.

### Artifact access and expiry — public repository

Workflow artifacts are **not private owner-only downloads**. GitHub requires
sign-in and repository read access to download them. Since QUANTUM-X is public,
other signed-in GitHub users have that read access and can download available
artifacts. They are not restricted to collaborators or the two owner computers.
See [GitHub's artifact download rules](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts).
Do not put credentials, private projects, personal data or local logs in a
package. Only the script's explicit runtime inputs are copied, never `target/`,
user profiles or `startup-log.txt` wholesale.

Downloads expire after 14 days (or earlier if the artifact/run is deleted).
An expired run has no downloadable package; use a newer green run or rerun the
workflow. Retention is time-based, not a promise to keep the latest N builds.
These temporary workflow artifacts are not permanent GitHub Release assets.

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
- R0.3 immutable fixture directories use explicit `text eol=lf` so their SHA-256
  manifests cover identical bytes on every checkout.

## Caching and disk

**Cache.** `Swatinem/rust-cache` caches the Cargo registry and dependency build
outputs.

- There is one cache for debug and one for release.
- Canonical release builds use a single explicit x64 target with static CRT;
  the build receipt and distribution are not Cargo dependency cache inputs.
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
- **The existing Python numerical export verifiers.** These remain later R0
  steps. R0.3 adds a read-only Python fixture integrity check, golden replay
  fingerprints and schemas 1–7 in the bounded CPU regression job. See
  [the fixture coverage and comparison contract](deterministic-regression-r03.md).

Making CI a required check is a repository setting. The owner turns it on in
branch protection once the workflow has a reliable green record.

## Reproduce locally (laptop)

Run from the workspace root. Rustup selects the pinned toolchain
automatically.

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --no-fail-fast
# Optional full local release build, not needed to download/use a CI package:
cargo build --workspace --release --locked
```

To match CI exactly, set the following first:

```powershell
$env:CARGO_INCREMENTAL = '0'
$env:CARGO_PROFILE_DEV_DEBUG = 'line-tables-only'
```

This rebuilds the debug profile once.

Prefer the laptop for source development and Actions for canonical packages;
both owner computers can compile, but the desktop currently has limited storage.
Do not create duplicate target directories or additional distributions just to
validate packaging. Lightweight local checks are:

```powershell
cargo fmt --all -- --check
./tools/test-windows-packaging.ps1
git diff --check
```

For an exceptional local package, first commit the source, then run
`./tools/package-windows.ps1`. This runs the single-build producer and packages
its receipt. Or run `./tools/build-windows.ps1` once and pass its receipt to
`./tools/package-windows.ps1 -BuildReceipt target/windows-release-build.json`.
The static-CRT release target may require fresh dependency compilation locally;
do not do this routinely on the storage-limited desktop. The scripts refuse to
overwrite distributions and do not remove existing builds, files or directories.

## Test isolation

Tests must not depend on the developer's own application data.

- The startup tests use default workbench preferences, as the offscreen
  diagnostics do.
- Recent-project history is kept in a per-test temporary folder.
- A real `%LOCALAPPDATA%\QUANTUM-X` profile therefore no longer changes their
  starting state.
