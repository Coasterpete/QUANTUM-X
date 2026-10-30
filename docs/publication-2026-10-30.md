# Source publication audit — 2026-10-30

This is the verification record for publishing the accumulated Rust development
of **QUANTUM Dynamics** from the **QUANTUM-X** repository. It supersedes earlier
pass reports' local-only Git status statements. It is not an official public
Alpha release or a ride-safety assessment.

## Audit and publication scope

The starting checkout was `codex/public-identity-library` at `8691591`, matching
fetched `origin/main`; it contained substantial uncommitted work. The publication
branch is `codex/publish-quantum-dynamics-progress`. Existing work was retained,
including startup, owner-directed colorful Editor, catalog/library infrastructure,
original icon, packaging and the latest NURBS regions and graph interactions.
Physics/GLB foundations already on main remain integrated. No original C++
repository files were edited or built during this publication pass.

The README now distinguishes the application name, repository identity, working
features, experimental systems, Windows build/run requirements and source Alpha
status. Two unretouched, actual offscreen wgpu/egui captures were selected for
`docs/screenshots/`. The Editor capture is labeled as a synthetic numerical
fixture, not a completed coaster design. Reports that link into `target/` retain
historical local evidence; those generated files are not published.

Publication excludes Cargo targets, local distributions/captures, logs, caches,
private environment/key files, third-party icon research checkouts, Blender
recovery copies and working render output. These local files were not deleted.
The approved ICO, original Blender scene, master PNG, rendering script, curated
runtime SVG/font assets and their license notices remain source assets.
No project license declarations changed. New build-time `embed-resource` uses
MIT licensing; the direct `image` dependency reuses the existing decoder.

Changed-file path/content and credential-pattern checks found no credentials or
unexpected executable/archive/build artifacts in the publication candidates.
This is a source review and targeted scan, not a claim of a comprehensive
security audit.

## Corrections found by the verification sweep

Several Python verifiers still expected schema 5. They now validate schema 6
(the historical M6/M7 export verifiers also retain explicit schema-4/5 support).
All substantive geometry, load, asset, playback and image checks remain enabled.

The M6 diagnostic fixture had begun using local NURBS endpoint edits while its
independent verifier still specified full-span quintic profiles. Endpoint-jet
preservation does not imply a quintic interior. The fixture now explicitly
chooses its original analytic quintics and converts them exactly to NURBS.
Its real-widget test makes that same explicit choice through the Editor and
still requires identical generated geometry. No user project is reshaped.
The independent verifier retains its original 901-sample, 10–11 m peak,
`1e-12` profile, `2e-5 g` normal and `2e-6 g` lateral error checks.

Older M7 scenario/live-cancel code wrote scalar endpoint mirrors directly.
Those operations now use the shared `region_edit::endpoint` command, preserving
valid NURBS control nets and continuity contracts. This affects development
verification scenarios, not the production editing workflow or physics model.
The M7 verifier now pins 901 stations before a local shape edit and 904 in its
preview/committed captures: real internal NURBS span changes add integration
stations. It still checks exact preview/commit agreement, meshes, images,
history, cancellation and reopen; no error tolerance was increased.

## Verification

Executed on native Windows with Rust 1.99.1 / `x86_64-pc-windows-msvc` and an
NVIDIA GeForce RTX 4070 using Vulkan. All checks below passed on the corrected
source. Initial failed verifier runs were investigated as described above;
they are not counted as successful verification.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 265 passed; zero failed or ignored |
| `cargo build --workspace --release --locked` | Passed |
| `tools/package-windows.ps1` with fresh output destinations | Passed: public/development PE, product metadata, all seven icon frames, native decoder, catalogs, samples, Welcome and project-file startup |
| `physics-diagnostic` | Passed |
| `editor-workflow-scenario` / `verify-editor-workflow.py` | Passed: eight captures, properties history/round trips, unchanged geometry/construction/trains |
| `authoring-scenario` / `verify-m6-exports.py` | Passed: analytic quintic profile and load checks, eight captures |
| `track-scenario` / `verify-m7-exports.py` | Passed: 14 fixtures, 40 captures, meshes, placement frames and live edit/history/cancel/reopen |
| `train-asset-scenario` / `verify-m81-exports.py` | Passed: GLB bytes/hierarchy, six component transforms, pivots, joints, Save As and actual rendered body/bogie pixels |
| `train-playback-scenario` / `verify-m82-exports.py` | Passed: conventional, below-track and rotating-seat fixtures; physical snapshots, joints, motors and captures |
| `ui-reference-scenario` / `verify-ui-reference.py` | Passed: 26 actual captures, palette, bounded physical viewports and 100/150/200% UI scaling |
| `transition-scenario` / `verify-transition-reference.py` | Passed: 10,001 independent saved-NURBS evaluations, shared C2 edit and independently integrated geometry |
| `examples` / `verify-authoring-exports.py` | Passed: Bezier/B-spline/NURBS/Catmull-Rom, frames and analytic references |
| `heartline-demo` / `verify-heartline-exports.py` | Passed: analytic rider-reference circle, curvature, speed, load and length |
| `git diff --check` and staged path/credential-pattern review | Passed |

The six packaged workflow scenarios and their Python verifiers were executed
outside the repository by `tools/verify-owner-ui.ps1` and the same individual
commands while correcting failures. Generated logs/distributions stay under
`target/publication-2026-10-30/` and ignored `target/publication-*` paths.
The README lists reproducible commands; scenario outputs must use fresh paths.

A fresh `git checkout-index --all --prefix=...` export of the staged source was
also built with `cargo build --workspace --release --locked --manifest-path
target/publication-2026-10-30/source-check/Cargo.toml --target-dir target`.
Packaging from that source export passed with an external `CARGO_TARGET_DIR`.
This verifies that excluded local checkouts, recovery files and render outputs
are not build or packaging dependencies. Dependency/compiler caches were reused;
this is not an offline build or a bit-identical binary reproducibility claim.

Representative independent numerical results: scalar split value/slope/second
errors `1.735e-17 / 2.168e-18 / 9.758e-19` in their physical units; analytic
positional-reference total-length error `1.228e-11 m`; heartline-circle length
error `1.112e-10 m`. Bounds and full checks remain in the verifier sources.

## Compatibility and limitations

Schema-1 through schema-5 inputs load through explicit migration to schema 6.
Existing spline representations, project IDs, train/asset definitions and
unconstrained legacy boundaries are preserved. Unknown data is rejected rather
than silently lost. Saving schema-6 NURBS regions requires a current application;
older builds cannot read them. Keep copies of older files when migrating.

The latest direct region split, shared seam, local shape control, Shift drag,
precision editing, draft/history and save/reopen improvements remain present.
The transition verifier independently evaluates 10,001 saved-NURBS samples and
checks numerical geometry evidence. This does not certify subjective ride
quality, mechanical smoothness or a safe ride.

No native desktop was controlled. Explorer/taskbar presentation, native dialogs,
mouse feel and end-to-end owner acceptance remain **not manually verified**.
Use the manual checklists in [owner UI](ux/owner-mockups.md),
[transition interactions](transition-cpp-reference-report.md),
[startup](welcome-startup.md) and [Windows icon](windows-icon.md).
A compact owner check is: launch the packaged public executable; New/Open a
project; split/edit a transition and use Shift/Esc; test Apply/Cancel, undo/redo,
Save As and reopen; inspect Track/Train and high-DPI/floating panes.

All production catalog styles are Planned. Development train GLBs are fixtures.
Support authoring, wheel/rail contact and full operational train simulation are
incomplete. Playback remains experimental with idealized captive guides at
1/240 s. Offscreen captures are not native input tests or FPS benchmarks.

## UX and distribution status

This pass preserves the owner-approved Welcome/Editor arrangement, original
Windows icon, project lifecycle, Coaster Properties and transition interactions.
It introduces no new product UX or developer-only interactive UI. Existing
optional development train/library controls remain labeled as such. Final
interaction feel and unfinished support/catalog/operational workflows remain
subject to human design and testing.

The Windows packaging process produces a verified, unsigned local portable
folder. This source update does not upload it, create a GitHub Release or announce
an official downloadable Alpha. No downloadable GitHub Releases existed when
checked during this audit. The pull request and Git history provide the final
published commit and merge references.
