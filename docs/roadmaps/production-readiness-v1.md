# QUANTUM Dynamics — Production Readiness Audit and Version 1.0 Roadmap

| | |
| --- | --- |
| Product (public) | **QUANTUM Dynamics** |
| Repository (internal) | **QUANTUM-X** |
| Audited checkpoint | `main` at `25ae183` (merge of PR #15: M4K Simulator Mode + M4K.1 Windows timing). Source tree identical to `67481a0`. |
| Audit date | 2026-11-14 |
| Audit type | Source, documentation, packaging script and test-suite inspection. **No roadmap item was implemented and no code was changed.** |
| Status of this document | Agent recommendation for owner review. The milestones, priorities and gates below are proposals. They are **not** approved product decisions. Any UX they name still needs owner design (AGENTS.md §3). |

---

## 1. Executive summary

QUANTUM Dynamics has a strong **numerical and authoring core** and a weak
**product shell**.

**The core is close to 1.0 quality.** It has:

- graph/timeline force-vector and geometric authoring with scalar NURBS regions;
- f64 geometry with independent verifiers;
- transactional undo/redo;
- validated, migrated and atomically written projects;
- a deterministic fixed 1/240 s rigid-body playback with measured, honest
  limitations;
- a native Simulator Mode that preserves the Editor session.

That core is supported by about 530 automated tests and many independent Python
verifiers.

**The product is not yet a dependable desktop application.** The gaps are not
in mathematics. They are in what happens around it:

- **Data safety.** A GPU error, device loss or panic ends the process. Unsaved
  work is lost. There is no autosave, recovery file or crash report.
- **Windows shell.** `QUANTUM Dynamics.exe` is a console-subsystem binary, so
  double-clicking it also opens a console window. Startup errors are written
  only to stderr. Behavior depends on the executable's file name. The shipped
  binary contains roughly 30 development CLI scenarios.
- **Ride operation.** Launches and brakes are the only devices. There are no
  lifts, stations, block sections, dispatch or multiple trains. A train on
  level track with no initial speed stays still, which is correct physics but
  a dead end for users. You can only get a train from a control labeled
  "Gray-box train · development placeholder".
- **Simulator window.** The Simulator replaces the Editor in the same window.
  The intended separate native window is not started. `qx-rendering` supports
  only one window.
- **Physics performance headroom.** One 3-car train (11 bodies) costs 1.3–2.3 ms
  of the 4.17 ms step budget on the laptop. A 16-car train averages 7.2 ms,
  which is over budget. The UI lets a user request up to 256 cars.
- **Distribution.** The package is an unsigned portable folder: about 173 MB,
  with the same 22 MB executable included twice. There is no installer, file
  association or versioned upgrade path. There is **no CI**: every check so far
  ran by hand on one machine.
- **Content.** 1 of 28 catalog styles is Available (Box Spined-100). No
  production train exists.

**Realistic distance to 1.0.** There are about ten milestones of work. Four of
them are non-negotiable reliability work that adds no new features: CI and
verification, the Windows shell, data safety, and packaging. Three depend on
owner UX design before they can finish: the Simulator window, operations UI,
and train setup. The largest technical risk is physics cost for longer and
multiple trains. The largest product risk is shipping simulation without
lifts, stations and blocks.

**Recommended 1.0 definition.** A person can install QUANTUM Dynamics on a
clean Windows 10/11 PC and author a complete single-coaster circuit. They can
fit at least one production train and simulate a full ride cycle: station,
dispatch, lift, layout, brakes and return. They can do this in a separate
Simulator window while the Editor stays open. They can save, crash, recover,
reopen and upgrade without losing work. The physics stays deterministic and
honestly labeled as an idealized model. Multi-train block operation is the
stretch goal for 1.0, gated on physics performance. Weather, audio,
wheel/rail contact, multiple coasters and the full catalog come after 1.0.

---

## 2. Capability inventory

Classification:

- **Implemented**: wired end to end, persisted where applicable, and covered by
  automated tests.
- **Experimental**: real code that is labeled development or opt-in, or has
  documented model limits.
- **Planned**: has a catalog entry, document or proposal, but no working
  feature.
- **Missing**: no design and no code.

### 2.1 Editor and authoring

| Capability | Status | Evidence / notes |
| --- | --- | --- |
| Welcome, New/Open, Recent with moved-file recovery, templates, Library | Implemented | `authoring/startup*.rs`, `recent.rs`, `project_library.rs`; 12+ lifecycle tests |
| Ordered force-vector, geometric and rider-local sections; distance/time domains | Implemented | `qx-geometry::{solver,transition,profile*}` |
| Scalar NURBS regions, split/merge, C0/C1/C2/Free seams, graph drag, numeric entry | Implemented | `graph_*`, `region_edit.rs`; 10,001-sample independent verifier |
| Bézier / B-spline / NURBS / Catmull–Rom complementary curves | Implemented | `qx-geometry::curve`; analytic references |
| Heartline / rider-reference geometry | Implemented | `heartline.rs`; analytic circle verifier |
| Transactional Apply/Cancel drafts and undo/redo | Implemented | `qx-document::command`, `draft_workflow.rs` |
| Coaster Properties (family / numbered style) | Implemented | Owner spec `docs/ux/coaster-properties.md` |
| Layout classification, bounded endpoint completion (G1) | Implemented | Does not guarantee G2 or obstacle clearance |
| Engineering plots (design-speed loads, curvature, bank…) | Implemented | Design loads are distinct from simulated loads |
| Supports: editable nodes/members, timber bents/lattice | Implemented (basic) | No structural analysis, terrain or footing hardware |
| Multiple coasters per project | Planned | `docs/multiple-coaster-roadmap.md`; owner deferred until after Alpha 1/2 |
| Terrain, scenery, obstacle clearance | Missing | — |
| In-app help / user manual / onboarding | Missing | Docs are developer pass reports |

### 2.2 Simulator, physics and trains

| Capability | Status | Evidence / notes |
| --- | --- | --- |
| Fixed 1/240 s rigid-body world, joints, hinges, limits, bounded motors | Experimental (well-tested) | `qx-physics::solver`; 4 substeps × 32 iterations, always run |
| Articulated consist on ideal bilateral (captive) track guides | Experimental | M4A–M4J; no unilateral wheel/rail contact, friction or impact |
| Bit-identical replay of the same inputs on one machine | Implemented | M4K.1: 18/18 runs matched |
| Gravity, rolling resistance, CdA drag, initial velocity | Implemented | Defaults are 0 resistance |
| Launch (constant / keyed / curve, force cap) and Brake devices | Implemented | Ideal distributed longitudinal actuators |
| Lift / chain / cable, tire drives, station, transfer, block system, dispatch | **Missing** | `DeviceKind` = {Launch, Brake} only |
| Multiple trains on one track | **Missing** | Playback resolves one associated train |
| M4J compatible-lead velocity initializer | Experimental, opt-in by environment variable | Fails on 16-car R20 curve; not the default |
| Simulator Mode (same window, preserves Editor session) | Experimental (development UI) | `simulator*.rs`; 5 real-UI tests + native smoke |
| **Separate native Simulator window** | **Missing** | Single-window `qx-rendering::Workspace` |
| Windows high-resolution worker pacing | Implemented | M4K.1; 0 catch-up events, ≥ 99.7 % fresh frames on laptop |
| Simulation speed control, single-step in the Simulator | Missing | 1× only |
| Telemetry: speed, distance, mass-weighted specific force, device force | Implemented | Not a seat accelerometer |
| Per-seat rider loads and seat camera catalog | Missing | Cameras use fixed development offsets |
| Train definitions, static GLB import, mounting, kinematic inspection | Implemented | GLBs shipped are development fixtures |
| Production train model (any) | **Missing** | 0 of 14 train families Available |
| Collision detection, wind, temperature, weather | Missing | Long-term, after 1.0 |
| Audio | Missing | `qx-audio` is a 14-line placeholder |

### 2.3 Graphics and assets

| Capability | Status | Evidence / notes |
| --- | --- | --- |
| wgpu DX12/Vulkan viewport, egui shell, docking, UI scaling | Implemented | `qx-rendering` |
| PBR track materials, environment lighting, Box Spined-100 GLB construction | Implemented | 105 MB of uncompressed PNG textures in git |
| Surface Lost/Outdated recovery | Implemented | `gpu.rs` recreates the surface |
| Device-lost recovery | **Missing** | Callback reports "restart inspector"; the event loop exits |
| Anti-aliasing, shadows, ground plane, sky | Missing | `sample_count: 1` everywhere |
| Adapter choice | Defect risk | `PowerPreference::LowPower` picks the integrated GPU on hybrid laptops; no setting or override |
| Measured FPS | **Not measured** | No frame-rate gate has ever run on a visible window |
| Texture compression, asset size budget | Missing | Package assets are about 129 MB |
| Catalog: 6 track families / 17 numbered styles; 14 train families / 11 styles | 1 Available, 27 Planned | `assets/catalog/styles.json` |
| Screen-reader support (AccessKit) | Missing | `egui-winit` built without its accessibility feature |

### 2.4 Persistence, distribution and verification

| Capability | Status | Evidence / notes |
| --- | --- | --- |
| Versioned `.qx.json`, schema 7, explicit migration from schemas 1–6 | Implemented | `persistence.rs`; `deny_unknown_fields` throughout |
| Atomic save (temp file, fsync, rename) | Implemented | |
| Size limits on untrusted input (16 MiB project, bounded library scans) | Implemented | |
| Autosave / crash-recovery journal | **Missing** | |
| Backup before migrating an older file | Missing | Docs tell users to "save a copy" by hand |
| "Saved by application version" metadata; newer-file message | Missing | An older app returns a generic "unsupported schema" error |
| Portable packaging script with self-checks | Implemented | `tools/package-windows.ps1` |
| Installer, uninstall, Start menu, file association, code signing, updater | Missing | `.qx.json` cannot be associated: Windows associates only the last extension (`.json`) |
| Crash reporting, log files | Missing | Exit record is on stderr only |
| CI (any) | **Missing** | No `.github/`; all verification is manual |
| Hidden-window native GUI smoke (welcome / edit / simulator / timing) | Implemented | Requires a GPU; `#[ignore]` in default suite |
| Cross-machine determinism check | Missing | Replay checked on the laptop only |

---

## 3. Findings: defects, limitations, risks

Severity: **S1** blocks 1.0 · **S2** must be fixed or explicitly accepted for 1.0 · **S3** polish / later.

### 3.1 Defects (verified in source or by evidence in the repository)

| # | Sev | Finding | Evidence |
| --- | --- | --- | --- |
| D1 | S1 | Unsaved work is lost on any rendering failure, GPU device loss or panic. The event loop exits with `RenderingFailed`. There is no autosave, no emergency save and no panic hook. | `workspace.rs` `finish()`, `gpu.rs` device-lost callback; no `set_hook` anywhere |
| D2 | S1 | The public executable is a console-subsystem program. Double-clicking `QUANTUM Dynamics.exe` also opens a console window. Fatal startup errors appear only there and disappear when the process exits. | No `windows_subsystem` attribute; `main()` prints to stderr |
| D3 | S2 | Startup route depends on the executable's file name. `quantum-x.exe` with no arguments prints CLI help. `cargo run -p quantum-x` therefore never opens the app (seen in the owner's local `startup-log.txt`). Two manual checklists say a no-argument launch opens the app, which is wrong: M4K step 1 (`cargo run --release -p quantum-x`) and M4K.1 step 6 (`.\target\release\quantum-x.exe`). | `startup::launch`; `docs/simulator-mode-m4k.md` §5.3; `docs/investigations/m4k1-windows-timing-correction.md` §9 |
| D4 | S2 | A user-visible GPU error says "restart inspector" (an old internal name). | `gpu.rs` |
| D5 | S2 | The seating error does not explain itself. A 3-car train on the default 10 m track fails with `distance query outside generated track`. | M4K investigation §6.4, still unchanged |
| D6 | S2 | The car count accepts 1–256. Only 1- and 3-car curved trains (and straight 16-car) are qualified, and 16 cars already exceed the real-time budget. | `layout_ui.rs:568`; M4J report |
| D7 | S2 | The internal name appears in user data paths: `%LOCALAPPDATA%\QUANTUM-X\recent-v1.json` and the workbench preferences path. | `recent.rs`, `workbench.rs` |
| D8 | S3 | The packaging script copies the same 22 MB executable twice and ships `assets/development`. A single package is about 173 MB. | `package-windows.ps1` |

### 3.2 Architecture limitations

- **Simulation ownership.** The single `train_playback::State` belongs to the
  Editor `Session`. The M4K proposal's own first step, extracting a
  mode-independent `SimulationController`, was deferred. This blocks the
  separate window and multi-train operations.
- **Single-window renderer.** The `Workspace` `ApplicationHandler` holds one
  `Window`, one surface and one egui context. Camera banks are integer indices
  (`profile == 1`).
- **App crate size.** `apps/quantum-x/src/authoring` is a flat module of about
  90 files and 33k lines. Production UI, sessions, roughly 15 acceptance
  scenario harnesses and tests are mixed together. All scenario commands ship
  in the public binary.
- **Schema churn.** The format went through seven schema versions in about
  four weeks. There is no stated 1.x compatibility policy yet.
- **Blocking project open.** Opening runs validation and generation on the UI
  thread. Large projects will freeze the window, and nothing has measured how
  long.
- **Fixed solver cost.** 128 solver sweeps run every tick, whether or not the
  solution has converged. Cost scales with body count and does not depend on
  motion.
- **Large binary assets in git.** 105 MB of PNG textures are stored directly
  in git, with no LFS and no compressed GPU format.

### 3.3 Performance risks

| Risk | Data | Exposure |
| --- | --- | --- |
| Physics budget | 3-car: mean 1.3–2.3 ms, worst 5.5 ms; 16-car straight: 7.2 ms mean (budget 4.167 ms) | Longer trains, multiple trains, laptop on battery |
| Energy drift | R8 fixture −15 J over 2 s; integrator "still drifts" (M4H) | Full-ride (60–180 s) circuits not yet characterized |
| Rendering | Frame CPU p50 1.44 ms in hidden profile; **no visible-window FPS ever measured** | 4K, 240 Hz, iGPU, large support structures |
| Worker CPU | 28–60 % of one core while playing | Battery life; multi-train scaling |
| Open/generation | Synchronous at open | Large projects |

### 3.4 User-experience problems

1. You cannot simulate a ride without using development tools. A train exists
   only through the gray-box development control. Simulator entry is under
   *Window / Layout › Simulator Mode (development)…*.
2. A lift, station or dispatch button does not exist. The common first attempt,
   a level track with no initial speed, does nothing. That is correct physics
   but a dead end for users.
3. A console window appears behind the app (D2). Errors are invisible to
   non-developers.
4. The project extension `.qx.json` cannot carry a Windows file association
   or a custom icon.
5. Only one window: users cannot watch the ride while editing the layout.
6. Telemetry g-values are mass-weighted train averages. Users will read them
   as rider loads unless the presentation makes the difference clear.
7. There is no user documentation. More than 60 documents are
   pass-by-pass engineering reports.
8. There is no screen-reader support. Keyboard focus and high-DPI behavior
   have not been verified by hand.

---

## 4. Version 1.0 scope decision

### 4.1 Essential for 1.0 (P0)

1. **Reliability shell:** a GUI-subsystem public binary, crash and log
   capture, a startup-failure dialog, and protection of the document on GPU
   loss.
2. **Data safety:** autosave and recovery, backup before migration,
   version-stamped files, and a frozen 1.x compatibility policy.
3. **Verification infrastructure:** CI and a repeatable verification step
   from laptop to desktop.
4. **Simulator as a product surface:**
   - an owner-designed Simulator, in a separate window if the owner confirms
     that direction;
   - an extracted simulation controller;
   - speed control and single-step;
   - an honest real-time-rate indicator.
5. **Single-train operational ride cycle:**
   - station stop, hold and dispatch;
   - lift (chain or cable with anti-rollback);
   - tire or friction drives;
   - trim and block brakes;
   - all under a deterministic operations controller.
6. **Physics qualification** for the shipped train configurations, with
   real-time and full-ride energy gates.
7. **Minimum production content:**
   - at least one Available track style (Box Spined-100 with final dimensions
     approved by the owner);
   - at least one original production train with qualified mechanics;
   - a seat or camera definition for that train;
   - a train setup workflow outside development tools.
8. **Graphics baseline:**
   - anti-aliasing;
   - a ground reference;
   - a correct adapter choice on hybrid laptops;
   - measured frame rate on both owner machines.
9. **Installable distribution:**
   - an installer and uninstaller;
   - a file association (needs an extension decision);
   - versioning;
   - a decision on code signing;
   - an automated, private release-candidate pipeline.
10. **User-facing documentation**: a getting-started guide, a shortcut
    reference and a model-limitations statement.

### 4.2 Target for 1.0, gated (P1)

- Multi-train block operation (2–3 trains) **if** R4 performance gates pass;
  otherwise it moves to 1.1.
- Shadows and a sky model.
- AccessKit screen-reader wiring.
- Asynchronous project open with progress.

### 4.3 After 1.0 (P2+)

- Unilateral wheel/rail contact, suspension, collisions.
- Wind, temperature and weather-dependent physics.
- Audio.
- Multiple coasters per project.
- Terrain and scenery.
- Structural analysis.
- The remaining 26 catalog styles and 13 train families.
- Mechanical 4D or free-spin transmission playback.
- Advanced PBR and HDR.
- A self-updater.
- Linux.
- News.

The full catalog does **not** need to ship in 1.0. Planned entries can stay
visible with their "Planned" status, or be hidden. That is an owner decision.

**Physics model policy for 1.0 (recommendation).** Ship the ideal
captive-guide rigid-body model, clearly labeled. Real coaster wheel assemblies
are captive (running, side-friction and up-stop wheels), so a bilateral guide
is a defensible first-order model. Unilateral contact is a later fidelity
upgrade, not a precondition. Make no ride-safety or certification claim in any
version.

---

## 5. Separate native Simulator window — evaluation

**Today:** `panels.rs` hands the single window to `simulator_ui::workspace`.
Return swaps the window back. The Editor `Session` survives by staying in
memory, not by being visible.

**Feasibility:** high. Everything needed is already in the dependency set:

- winit 0.30 supports multiple windows in one event loop.
- wgpu allows one `Instance`/`Adapter`/`Device`/`Queue` to drive several
  `Surface`s.
- egui-winit allows one `egui::Context` plus one `egui_winit::State` per
  window.

No new dependency is required.

**Proposed structure** (illustrative names):

```text
App (event loop, routes by WindowId)
 ├─ GpuShared: Instance, Adapter, Device, Queue, pipelines, mesh/material caches
 ├─ ProjectSession: Editor (doc + undo), path/dirty, generation service, drafts   ← unchanged owner
 ├─ SimulationController (extracted from train_playback::State)
 │    source key · frozen input snapshot · worker · epoch · reset snapshot · telemetry
 │    operations controller state · input/event log
 ├─ EditorWindow:    surface, egui ctx, camera bank, layout       → reads ProjectSession
 └─ SimulatorWindow: surface, egui ctx, sim camera bank, sim UI   → reads SimulationController
```

**How it preserves the active Editor session.** The Editor window is never
torn down. The Simulator window only *reads* a frozen snapshot that is
identified by a source key (committed revision, generation identity, train
revision, operations revision and initializer policy). Nothing written from
the Simulator reaches the document. This keeps the invariant M4K already
tests, and the existing `simulator_tests` editor-identity assertions carry over
unchanged.

**The key policy decision: editing while the Simulator is open.**

| Option | Behavior | Determinism | Recommendation |
| --- | --- | --- | --- |
| A. Lock the Editor while simulating | Editor read-only until the Simulator closes | Trivial | Simple but defeats the purpose of two windows |
| **B. Snapshot isolation + stale indicator** | Editing continues. The Simulator keeps running its frozen snapshot and shows "Layout changed — Reload". Reload prepares a fresh simulation from the new committed revision. | Preserved: each run is tied to one source key | **Recommended** |
| C. Hot-swap geometry under a running train | Track changes mid-run | Breaks reproducibility and replay | Reject |

**Other rules to settle with the owner:**

- Closing the Editor window closes the Simulator. The unsaved-changes guard
  stays on the Editor.
- Closing the Simulator retires its epoch, as Return does today.
- Simulator shortcuts apply only when that window has focus.
- Each window paces itself to its own monitor's refresh rate. There is one
  physics clock, and presentation interpolation is per window.
- Window placement is persisted in application preferences, never in the
  project.
- The same-window mode can stay as a fallback for single-monitor laptops.

**Risks:**

- Two surfaces on an integrated GPU add load (measure in R7).
- Focus and shortcut conflicts.
- Device loss must now protect two windows.
- The editor-owned `State` must be extracted first. That extraction is the
  real work, not the second surface.

**Owner decisions required:**

- separate window versus a same-window toggle;
- editing policy B;
- entry and return commands and their shortcuts;
- Simulator composition and visual design.

---

## 6. Completing operational ride simulation without compromising determinism

**Principle.** Operations are a *controller layer*. It reads physical state and
writes bounded actuator commands at fixed tick boundaries. It never assigns
positions or velocities (AGENTS.md §8.1).

1. **Single clock.**
   - Controllers run on the physics worker, once per 1/240 s outer step,
     before the force evaluation.
   - Their state (timers, block occupancy, dispatch latches, lift engaged)
     is part of the `World` snapshot. Reset restores it exactly.
   - Nothing in the controller reads wall-clock time.
2. **Inputs as tick-stamped events.**
   - UI actions (Dispatch, E-stop, manual mode) go into a queue.
   - Each is applied at the next tick boundary as `(tick, event)`.
   - The event log is enough to replay any run bit for bit, including runs
     with operator input.
3. **Sensors derived from physics.**
   - Block occupancy and photo-eye triggers are computed from body stations
     after integration, using half-open intervals like existing devices.
   - The rules for ties and boundaries are documented.
4. **Actuators as force laws with limits:**

   | Device | Model |
   | --- | --- |
   | Chain lift | Drive toward chain speed with a grip force limit, plus a ratchet-style anti-rollback as a one-sided force or constraint |
   | Tire drive | Traction force toward wheel surface speed, friction-limited |
   | Trim / block brakes | Speed-dependent force with fail-safe closed state |
   | Station | Brake + tire drive + hold state |

   Every actuator reports requested, applied and limited force telemetry, as
   Launch and Brake already do.
5. **Block system as an explicit state machine.**
   - Interlocks are pure functions of occupancy and controller state.
   - Invariants are tested with property-style tests, for example "a block is
     never released while the next block is occupied".
   - This is a simulation behavior check, **not** a safety certification.
6. **Multiple trains.**
   - Use one `World` per train, stepped in a fixed order. They interact only
     through the block controller, so there are no train-to-train contacts
     before collision work.
   - Overlapping occupancy is reported as a simulated fault, never resolved
     by moving a train.
   - Per-train worlds can later run in parallel, joined at the tick boundary
     to keep determinism.
7. **Real time is a presentation property.**
   - If stepping cannot keep up, the run slows and the rate shown drops below
     1.0×.
   - Steps are never enlarged, skipped or reordered. Deterministic results
     do not depend on the host.
8. **Persistence.**
   - Operations authoring (devices, blocks, station and dispatch settings)
     goes into one schema bump, planned together with R2's version metadata.
   - Runtime controller state stays transient.

**Determinism gates:**

- Bit-identical replay with operator event logs on the same machine.
- A cross-machine comparison (laptop ↔ desktop) of golden fixtures. Either
  prove the results are bit-identical, or document the measured divergence
  and a tolerance. Do not assume either outcome.

---

## 7. Windows packaging, startup, recovery and release automation — audit

| Area | Today | 1.0 requirement |
| --- | --- | --- |
| Subsystem | Console | GUI-subsystem `QUANTUM Dynamics.exe`; a separate small `quantum-x` dev/CLI binary (or a feature-gated build) so scenario harnesses do not ship to users |
| Routing | By executable file name | Explicit: the GUI binary always opens the app; the CLI binary always runs the CLI |
| Startup failure | stderr line + exit record | Native message dialog with the reason, log path and next step; same exit codes |
| Logging | stderr JSON exit record | Rolling log under `%LOCALAPPDATA%\QUANTUM Dynamics\logs`, bounded size, no personal data |
| Crash | A main-thread panic prints to stderr (invisible without a console) and exits with code 101; unsaved work is lost | Panic hook writes a crash report and an emergency autosave of the document, then shows a dialog |
| GPU loss | Exit | Save the recovery file, then recreate the device or offer restart |
| Adapter | LowPower | HighPerformance by default; a Setup option plus an environment override; show the adapter in About/Diagnostics |
| Compatibility | Schema 1→7 migration; newer files rejected generically | Backup before migrating; `saved_with` version; "This project was saved by QUANTUM Dynamics X.Y" message; a fixture corpus per schema checked in CI |
| Package | Portable folder, 2 × 22 MB exe, about 173 MB | Installer (per-user, no admin), Start menu, uninstall that keeps user projects, file association, version in Programs & Features; an optional portable zip |
| Extension | `.qx.json` | Owner decision: a single unique extension (for example `.qxd`) for association, still reading `.qx.json` |
| Signing | Unsigned | Owner decision. Unsigned installers trigger SmartScreen warnings; a signing certificate has a recurring cost |
| User data paths | `%LOCALAPPDATA%\QUANTUM-X\…` | `%LOCALAPPDATA%\QUANTUM Dynamics\…`, with a one-time migration that keeps the old files |
| Release automation | None | GitHub Actions on `windows-latest`: fmt, clippy, test, release build, package, WARP-adapter hidden-window smoke, license/SBOM report. Tagged builds produce a **draft** release artifact only; publishing stays a manual owner action |
| Third-party licenses | Manual copy of a few notices | Generated notice file from `Cargo.lock` (for example `cargo-about`); license allow-list check (for example `cargo-deny`) |

The installer technology (WiX/MSI, MSIX or Inno Setup) is a technical choice
to make in R9. MSIX has the best uninstall hygiene but needs signing. Inno and
WiX work unsigned.

---

## 8. Roadmap

The milestones use an **R** prefix (Readiness) to avoid colliding with the
existing M-series names. Dependencies are listed first. "Owner" marks design
decisions that block completion, not the start of the work.

```text
R0 CI & verification ──┬─▶ R1 Windows shell ──▶ R2 Data safety ──────────────┐
                       │                                                      ▼
                       ├─▶ R3 Sim controller + window (Owner UX) ──▶ R5 Operations (Owner UX) ─▶ R6 Content ─▶ R10 RC
                       ├─▶ R4 Physics perf & qualification ──────────▲        ▲                      ▲
                       ├─▶ R7 Graphics baseline ───────────────────────────────────────────────────────┤
                       ├─▶ R8 UX completion & docs (Owner) ────────────────────────────────────────────┤
                       └─▶ R9 Packaging & release pipeline (needs R1, R2) ─────────────────────────────┘
```

### R0 — Verification infrastructure (P0, first)

- **Scope:**
  - GitHub Actions on Windows running fmt, clippy `-D warnings`, `cargo test
    --workspace --locked` and a release build.
  - A package job that keeps the artifact as a CI artifact only.
  - An environment override that forces the WARP/fallback adapter, so
    `--gui-smoke-test` runs on GPU-less runners.
  - A golden determinism fixture set (pose hashes at fixed ticks).
  - A `tools/verify-desktop.ps1` script (proposed) that turns the desktop
    checklist into JSON results.
- **Acceptance:**
  - CI is required on every PR to `main`, with 10 consecutive green merges.
  - The WARP smoke passes welcome, edit and simulator.
  - The package built by CI from a clean checkout passes all
    `package-windows.ps1` self-checks.
  - Golden replay hashes are stored and compared.
- **Tests:** existing suite; new CI-only WARP smoke; golden-hash comparator.

### R1 — Windows application shell hardening (P0; needs R0)

- **Scope:**
  - GUI subsystem.
  - Split the public and dev binaries (or attach a console only for CLI use).
  - Panic hook with crash report.
  - Rolling log file.
  - Native startup-failure dialog.
  - HighPerformance adapter with Setup option and environment override.
  - Device-lost handling.
  - Rename user data paths to QUANTUM Dynamics (with migration).
  - Fix D3, D4 and D5.
  - Clamp the car count to qualified sizes (D6), or label the unqualified
    range as experimental.
  - Correct the M4K checklist command.
- **Acceptance:**
  - Double-clicking opens no console.
  - An injected panic in Editor and in Simulator produces a crash file,
    a dialog and a recovery file (after R2).
  - Injected device loss (`Device::destroy` in a test build) keeps the
    document.
  - Running with no adapter shows the dialog, exits with code 1 and writes
    a log entry.
  - `cargo run -p quantum-x` opens the app or prints an explicit hint.
  - Existing CLI scenarios still pass from the dev binary.
- **Tests:** startup routing unit tests; injected-fault native smoke; package
  self-check updated for one GUI binary.

### R2 — Data safety and compatibility freeze (P0; needs R1)

- **Scope:**
  - Autosave journal: periodic and on significant commands, written atomically
    off the UI thread, in a recovery folder, not over the user's file.
  - Recovery prompt on the next launch.
  - Timestamped backup before writing a migrated file.
  - `saved_with` application-version metadata and a clear message for newer
    files.
  - A written 1.x compatibility policy: additive schemas, explicit
    migrations, never silently dropped data.
  - A checked-in corpus with at least one fixture per schema 1…N.
  - Owner decision on project packaging for external GLBs: folder versus
    single-file bundle.
- **Acceptance:**
  - Killing the process during editing and relaunching restores state with
    at most 60 s or 20 commands lost (owner to confirm the target).
  - The recovery file is never offered for a project that has since been
    saved.
  - All corpus fixtures load, migrate, save and reload identically.
  - Truncated and corrupted inputs (property-generated) produce errors,
    never panics.
  - An older-schema file is never overwritten without a backup.
- **Tests:** process-kill integration test; corpus round-trip; malformed-input
  property tests; migration backup test.

### R3 — Simulation controller extraction and separate Simulator window (P0; Owner UX; needs R0)

- **Scope:**
  - Extract `SimulationController` from the editor-owned `train_playback::State`.
  - Multi-window `qx-rendering`: shared device, per-window surface, egui,
    input and camera bank.
  - Simulator window with editing policy B (§5).
  - Speed control (fixed-step scheduling only), single-step, and a displayed
    real-time rate.
  - Simulator entry moved out of the development menu once the owner design
    exists.
- **Acceptance:**
  - All existing `simulator_tests` editor-identity assertions pass in both
    window modes.
  - Edits in the Editor while the Simulator runs never change the running
    simulation's source key, and Reload uses the new revision.
  - 100 open/close cycles of the Simulator window show no leak: GPU queue
    drained, surfaces released, memory stable within a stated bound.
  - Closing the Editor with unsaved changes still prompts.
  - Physics replay is bit-identical to same-window mode for the same inputs.
- **Tests:** controller unit tests (stale epoch, cancel, reset); two-window
  hidden smoke; leak/cycle soak; manual owner checklist on laptop
  (single monitor) and desktop (multi-monitor, mixed refresh).

### R4 — Physics performance and v1 train qualification (P0; needs R0)

- **Scope:**
  - Profile the step.
  - Evaluate a convergence-based early exit and other solver changes **only**
    against the existing test suite and current tolerances. No tolerance may
    be loosened.
  - Define the shipped train envelope: car count, curvature, speed range.
  - Full-ride energy characterization (60–180 s circuits).
  - Cross-machine determinism comparison.
- **Acceptance (proposed numbers for owner confirmation):**
  - For the shipped reference train (for example up to 8 cars), step p99
    ≤ 2.0 ms on the desktop and ≤ 3.3 ms (80 % of budget) on the laptop,
    on battery and on AC.
  - Zero catch-up events over a 180 s ride on both machines.
  - Energy error over a full ride is reported and within an owner-accepted
    bound per fixture.
  - All M4A–M4J regression suites, including the ignored release controls,
    pass unchanged.
  - Car count in the UI is limited to the qualified envelope.
- **Tests:** solver A/B harness with fixed fixtures; full-ride energy ledger;
  golden cross-machine hashes; the existing `velocity_initialization` and M4I
  suites.

### R5 — Operational ride simulation (P0 single-train; P1 multi-train; Owner UX; needs R3, R4, R2 schema)

- **Scope:**
  - The deterministic operations controller (§6).
  - New device kinds: lift, tire drive, trim/block brake, station.
  - Block sections and dispatch.
  - Tick-stamped operator events and event-log replay.
  - Operations authoring UI (owner design).
  - P1: 2–3 trains with block interlocks.
- **Acceptance:**
  - A reference single-train circuit runs station → dispatch → lift →
    layout → brakes → station for 10 consecutive cycles, unattended, with
    deterministic replay.
  - Each device's applied force never exceeds its authored limit.
  - Anti-rollback holds a stopped train on the lift.
  - P1: block invariants hold over 1,000 simulated cycles of randomized
    dispatch timing.
  - The cycle time is reported, not asserted against a real ride.
  - Saving and reopening keeps all operations authoring.
- **Tests:** per-device analytic checks (for example lift speed under a known
  grade); state-machine property tests; event-log replay; schema migration
  test.

### R6 — Production content minimum (P0; Owner; needs R4, R5)

- **Scope:**
  - Final owner-approved Box Spined-100 dimensions.
  - At least one original production train: GLB, mechanical definition,
    seats and camera mounts, qualified in R4.
  - Train setup workflow outside development tools.
  - Decide how Planned catalog entries are presented.
- **Acceptance:**
  - A new user can create a project, assign style and train, and simulate
    without opening development tools.
  - Asset provenance and licenses are recorded.
  - The gray-box train remains available only as a labeled development
    fixture.

### R7 — Graphics production baseline (P1 overall; AA and adapter are P0; needs R0)

- **Scope:**
  - MSAA or an equivalent anti-aliasing method.
  - Ground plane and horizon.
  - Optional directional shadows.
  - Compressed textures (KTX2/BC7) with a package-size budget.
  - Frame-time instrumentation in visible windows.
- **Acceptance:**
  - Measured on both machines in a visible window, using a reference scene:
    a full circuit with supports and a train in motion.
  - Desktop: ≥ 60 FPS at 1440p (p95 frame time ≤ 16.7 ms).
  - Laptop integrated GPU: ≥ 60 FPS at 1080p, or a documented quality preset
    that achieves it.
  - Package assets ≤ 80 MB without visible loss, signed off by the owner.
- **Tests:** offscreen image regression, compared against references within a
  tolerance; frame-time capture script.

### R8 — UX completion, accessibility and documentation (P0 where blocking; Owner)

- **Scope:**
  - Owner design passes for the Simulator, Operations and train setup.
  - Error-message audit.
  - Keyboard and focus review.
  - AccessKit wiring (P1).
  - High-DPI and multi-monitor review.
  - Getting-started guide, shortcut reference, and a model-limitations and
    no-certification statement.
  - Consolidate developer docs into architecture references; the pass
    reports become history.
- **Acceptance:**
  - Owner sign-off on each listed workflow.
  - Every visible control performs its action, verified by a UI test or the
    manual checklist.
  - A user guide covers the 1.0 workflows end to end.

### R9 — Installer and release pipeline (P0; needs R1, R2)

- **Scope:**
  - Installer and uninstaller.
  - File association once the extension is decided.
  - Version scheme: semver plus git hash in About and logs.
  - Signing decision.
  - Generated third-party notices and license policy.
  - CI produces draft release artifacts on tags. No public publication.
- **Acceptance:**
  - On a clean Windows 11 user account on the desktop: install, launch,
    open by double-clicking a project file, uninstall. User projects remain;
    app files and Start menu entries are removed.
  - Upgrading RC n → RC n+1 keeps preferences and recent projects.
  - CI artifact hash equals the hash verified on the desktop.

### R10 — Release-candidate validation and 1.0 decision (P0; needs all P0)

- **Scope:**
  - Feature freeze.
  - RC builds tested on the desktop.
  - 2-hour Simulator soak.
  - The full project corpus.
  - Owner acceptance checklist.
  - Known-issues list.
- **Acceptance:**
  - Zero open S1 issues; every S2 fixed or explicitly accepted by the owner.
  - The soak shows stable memory (growth ≤ 5 % after warm-up) and no
    catch-up drift.
  - The owner signs off.
  - Publication remains a separate, explicit owner action.

**Suggested order for one developer:**

1. R0 → R1 → R2. This is reliability work. It can start now with no UX
   decisions.
2. R4 in parallel with owner design for R3 and R5.
3. R3 → R5 → R6.
4. R7 and R8 throughout.
5. R9, then R10.

---

## 9. Laptop-development → desktop-distribution verification strategy

**Roles:**

- **Laptop** (Intel integrated GPU, Vulkan, 240 Hz): development, unit and
  integration tests, quick native smokes.
- **Desktop:** consumes built packages only. It handles packaged-app testing,
  discrete-GPU compatibility and release validation, and does not need a Rust
  toolchain.
- **CI:** the canonical build, once R0 lands.

**Flow per candidate:**

1. **Laptop.** Before pushing a branch, run `cargo fmt --check`, clippy, and
   `cargo test --workspace`. For UI or simulation changes, also run the
   relevant `--gui-smoke-test`. Do not build packages locally for every
   change.
2. **CI.** On PR or merge, run the same checks plus a release build, package
   and WARP smoke. CI produces **one** package per commit, named
   `QUANTUM Dynamics-<semver>-<gitshort>-windows` with a SHA-256 manifest.
3. **Transfer.** The desktop downloads that CI artifact. Never copy a
   laptop-built `target/` folder: CI is the single source of truth for
   binaries, which also avoids duplicate artifacts.
4. **Desktop verification** (`verify-desktop.ps1`, proposed), writing one JSON
   file:
   - hash check against the manifest;
   - `--version`, `--self-check`, `--startup-check`;
   - `--gui-smoke-test` welcome, edit and simulator on the discrete GPU;
   - `simulator-timing` on the reference fixtures;
   - golden replay hash comparison with the laptop and CI;
   - after R9, an install and uninstall cycle in a scratch user account.
5. **Manual checklist** on the desktop, with results recorded in the PR or
   release-candidate issue:
   - Explorer double-click, console absence;
   - DPI 100/150/200 %, multi-monitor and mixed refresh;
   - Simulator window behavior and visible-window FPS;
   - device or driver reset, for example by changing the display mode while
     running.
6. **Evidence.** Commit only the small JSON summary and the checklist result
   under `docs/evidence/<rc>/`. Binaries, captures and logs stay in CI
   artifacts or local `target/`, never in git.
7. **Hygiene.** Keep the latest N CI artifacts. Locally, delete old
   `target/distribution/*` folders by hand; packaging already refuses to
   overwrite.

**What each machine can and cannot prove:**

- Integrated GPU (laptop) and NVIDIA (desktop) cover two vendors. AMD and older
  Windows 10 builds remain **unverified** unless an external tester is added.
- WARP on CI proves code paths, not performance.

---

## 10. Owner decisions that block 1.0

1. Separate Simulator window (§5): editing policy B, entry and return
   commands, composition.
2. Operations authoring UX: devices, blocks, station and dispatch controls.
3. Train setup workflow and the first production train: art, mechanics,
   seats.
4. Final Box Spined-100 dimensions, and how Planned catalog entries appear.
5. Project file extension, and single-file bundle versus folder for external
   GLBs.
6. Code signing (cost) and installer technology preference.
7. Whether multi-train operation is required for 1.0 or moves to 1.1.
8. Accepted numeric targets: autosave loss window, physics budgets, FPS
   targets, energy bound.

---

## 11. Audit verification record

- Checkpoint inspected:
  - `origin/main` `25ae183`, fetched 2026-11-14;
  - working branch `claude/production-readiness-audit` created from it;
  - no source files changed; this document is the only addition.
- Baseline `cargo test --workspace --locked --no-fail-fast` on the laptop at
  this checkpoint: see §11.1.
- **Not run:**
  - `cargo clippy` and release builds (no code changed; avoids an
    unnecessary large build);
  - `package-windows.ps1` (avoids duplicate artifacts);
  - native GUI smokes;
  - Python verifiers.
- Not manually verified: everything that needs the desktop UI. No desktop
  control was used.
- Facts quoted from earlier reports, such as timings and test counts, are
  attributed to those reports and were not re-measured here. They include
  M4K, M4K.1, the M4J, M4H and integrated-core reports, and the
  2026-10-30 publication audit.

### 11.1 Baseline test result

Run on the owner's laptop at `25ae183`, with a warm `target/` cache:

| | Result |
| --- | --- |
| Command | `cargo test --workspace --locked --no-fail-fast` |
| Outcome | **530 passed, 0 failed, 16 ignored**, across 65 test binaries and doctest groups |
| App binary | 202 passed, 1 ignored |
| Wall time | 5 min 24 s |

This matches the M4K.1 report.

The 16 ignored tests are long release-mode research controls and the
GPU-dependent `gui_smoke` test. They were not run.
