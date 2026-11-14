# QUANTUM Dynamics — Production Readiness Audit and Version 1.0 Roadmap

| | |
| --- | --- |
| Product (public) | **QUANTUM Dynamics** |
| Repository (internal) | **QUANTUM-X** |
| Audited checkpoint | `main` at `25ae183` (merge of PR #15: M4K Simulator Mode + M4K.1 Windows timing). Source tree identical to `67481a0`. |
| Audit date | 2026-11-14 (revised the same day after owner review) |
| Audit type | Source, documentation, packaging script and test-suite inspection. **No roadmap item was implemented and no code was changed.** |
| Status of this document | Roadmap for owner approval. Sections marked **owner direction** record established product decisions. Everything else is an agent recommendation. Every numeric target is **provisional** (see §8.1). GUI composition, shortcuts and detailed window behavior still need owner design (AGENTS.md §3). |

---

## 1. Executive summary

QUANTUM Dynamics has strong **authoring and numerical foundations**, a
**promising but still experimental physics system**, and a weak **product
shell**.

**What is verified today:**

- graph/timeline force-vector and geometric authoring with scalar NURBS regions;
- f64 geometry with independent verifiers;
- transactional undo/redo;
- validated, migrated and atomically written projects;
- a native Simulator Mode that preserves the Editor session.

These are supported by 530 passing automated tests and many independent Python
verifiers.

**Physics is not near production readiness.** It is extensively tested, but
experimental. These parts are verified within their tested scope:

- the fixed 1/240 s rigid-body integration;
- analytic checks for grades, launches and level running (M4K investigation);
- bit-identical replay on one machine (M4K.1).

The model is an idealized captive-guide approximation. These remain
**unvalidated**:

- full-ride energy behavior over 60–180 s;
- longer trains: the 16-car R20 curve fails to initialize, and 16 cars on a
  straight already exceed the real-time budget;
- real-time performance across hardware;
- determinism across machines;
- production train mechanics.

§2.2 separates verified results from experimental capabilities.

**The product is not yet a dependable desktop application.** The gaps are
mostly outside the mathematics:

- **Data safety.** A GPU error, device loss or panic ends the process. Unsaved
  work is lost. There is no autosave, recovery file or crash report.
- **Windows shell.** `QUANTUM Dynamics.exe` is a console-subsystem binary, so
  double-clicking it also opens a console window. Startup errors are written
  only to stderr. Behavior depends on the executable's file name. The shipped
  binary contains about 30 development CLI scenarios.
- **Ride operation.** Launches and brakes are the only devices. There are no
  lifts, stations, block sections, dispatch or multiple trains. A user can
  only add a train through a development-placeholder control.
- **Simulator window.** The separate native Simulator window is an established
  owner requirement (§5), but it is not implemented. The current Simulator
  Mode replaces the Editor inside the one window that `qx-rendering` supports.
- **Distribution.** The package is an unsigned portable folder: about 173 MB,
  with the same 22 MB executable included twice. There is no installer, file
  association or upgrade path. There is **no CI**: every check so far ran by
  hand on one machine.
- **Content.** 1 of 28 catalog styles is Available (Box Spined-100). No
  production train exists.

**Realistic distance to 1.0.** There are about ten milestones of work. Four of
them are reliability work that adds no new features: CI and verification,
Windows reliability, project data safety, and packaging. The largest technical
risk is physics validation and performance for full rides, longer trains and
multiple trains. The largest product risk is shipping simulation without lifts,
stations and blocks.

**Recommended 1.0 definition.** A person can install QUANTUM Dynamics on a
clean Windows 10/11 PC and author a complete single-coaster circuit. They can
fit at least one production train and simulate a full ride cycle: station,
dispatch, lift, layout, brakes and return. The simulation runs in a separate
Simulator window while the Editor stays open and usable. They can save, crash,
recover, reopen and upgrade without losing work. The physics stays
deterministic and is honestly labeled as an idealized model that has been
validated only within its stated scope. Multi-train block operation is the
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

**Physics maturity.** The physics system is a promising, extensively tested
**experimental** foundation. It is not production-ready. Three tiers of
evidence apply:

**Verified within the tested scope.** These results come from the cited
reports and are preserved here unchanged.

- **Fixed-step integration.** Integration uses a fixed 1/240 s outer step with
  4 internal substeps and 32 solver iterations. Steps are never enlarged or
  skipped.
- **Analytic checks**, using a 3-car gray-box train (11 bodies, 2440 kg) on
  80–400 m development tracks (M4K investigation §3; M4K.1 §6):

  | Case | Result |
  | --- | --- |
  | Level track, no initial speed | Stays at 0 |
  | Level track at 3 m/s, no resistance | Holds 3.0000 m/s |
  | 10° downhill | Accelerates at 1.7035 m/s², matching g·sin 10° |
  | 1 m/s² launch | Accelerates at 1.0000 m/s² with 2440.0 N, matching m·a |

- **Bit-identical replay on one machine.** 18 of 18 timing runs replayed
  identically, at ticks 1615 and 2853–2882 (M4K.1).
- **Reset.** Reset matched the initial poses in every run (M4K.1).
- **Windows worker pacing on the laptop (M4K.1):**
  - 239.9–240.0 steps/s;
  - 0 catch-up events;
  - drift 1.4–2.8 ms after 12 s;
  - at least 99.7 % of frames had fresh physics data.
- **Initializer and guidance qualification** (M4A–M4J test suites), covering
  the positive scope listed in those reports: 1- and 3-car curved trains, and
  the listed straight, shared and unequal fixtures.

**Experimental: works, but carries documented model limits.**

- **Guides.** Ideal bilateral (captive) track guides. There is no unilateral
  wheel/rail contact, friction, suspension or impact, and the guides can
  apply pulling forces.
- **Devices.** Launch and Brake are ideal distributed longitudinal actuators.
- **Telemetry.** Mass-weighted train telemetry, which is not a seat
  accelerometer.
- **M4J velocity initializer.** Opt-in only.

**Unvalidated: open before any production claim.**

- **Full-ride energy behavior.**
  - The integrator drifts: −15.10 J on the R8 fixture by tick 258, at
    4 internal steps (M4H).
  - No circuit longer than a few seconds of the 60–180 s ride range has an
    energy ledger.
- **Longer trains.**
  - The 16-car R20 curve fails bounded projection after 20,000 sweeps (M4J).
  - The 16-car straight steps at 7.2 ms mean, over the 4.167 ms budget (M4J).
- **Real-time performance across hardware.**
  - The 3-car step costs 1.3–2.3 ms mean, 5.5 ms worst, on the laptop only.
  - Battery, the desktop and multiple trains are not measured.
- **Cross-machine determinism.** Replay has been compared on the laptop only.
- **Production train mechanics.** No production train exists. Mass, inertia,
  wheel geometry and seats are all development values.

| Capability | Status | Evidence / notes |
| --- | --- | --- |
| Fixed 1/240 s rigid-body world, joints, hinges, limits, bounded motors | Experimental (verified within tested scope) | `qx-physics::solver`; 4 substeps × 32 iterations always run |
| Articulated consist on ideal bilateral (captive) track guides | Experimental | M4A–M4J; no unilateral wheel/rail contact, friction or impact |
| Bit-identical replay of the same inputs on one machine | Implemented | M4K.1 18/18 |
| Gravity, rolling resistance, CdA drag, initial velocity | Implemented | Analytic checks above; resistance defaults to 0 |
| Launch (constant / keyed / curve, force cap) and Brake devices | Implemented (ideal actuators) | `devices.rs` |
| Lift / chain / cable, tire drives, station, transfer, block system, dispatch | **Missing** | `DeviceKind` = {Launch, Brake} only |
| Multiple trains on one track | **Missing** | Playback resolves one associated train |
| M4J compatible-lead velocity initializer | Experimental, opt-in by environment variable | Fails on 16-car R20; not the default |
| Simulator Mode (same window, preserves Editor session) | Experimental (development UI) | `simulator*.rs`; 5 real-UI tests + native smoke |
| **Separate native Simulator window** | **Missing (established owner requirement)** | Single-window `qx-rendering::Workspace`; see §5 |
| Windows high-resolution worker pacing | Implemented | M4K.1 (laptop) |
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
| Hidden-window native GUI smoke (welcome / edit / simulator / timing) | Implemented (local GPU) | `#[ignore]` in default suite; needs a GPU and a desktop session |
| Cross-machine determinism check | Missing | Replay checked on the laptop only |

---

## 3. Findings: defects, limitations, risks

Severity: **S1** blocks 1.0 · **S2** must be fixed or explicitly accepted for 1.0 · **S3** polish / later.

### 3.1 Defects (verified in source or by evidence in the repository)

| # | Sev | Finding | Evidence | Milestone |
| --- | --- | --- | --- | --- |
| D1 | S1 | Unsaved work is lost on any rendering failure, GPU device loss or panic. The event loop exits with `RenderingFailed`. There is no autosave, no emergency save and no panic hook. | `workspace.rs` `finish()`, `gpu.rs` device-lost callback; no `set_hook` anywhere | R1 diagnostics; R2 document protection |
| D2 | S1 | The public executable is a console-subsystem program. Double-clicking `QUANTUM Dynamics.exe` also opens a console window. Fatal startup errors appear only there and disappear when the process exits. | No `windows_subsystem` attribute; `main()` prints to stderr | R1 |
| D3 | S2 | Startup route depends on the executable's file name. `quantum-x.exe` with no arguments prints CLI help. `cargo run -p quantum-x` therefore never opens the app (seen in the owner's local `startup-log.txt`). Two manual checklists say a no-argument launch opens the app, which is wrong: M4K step 1 (`cargo run --release -p quantum-x`) and M4K.1 step 6 (`.\target\release\quantum-x.exe`). | `startup::launch`; `docs/simulator-mode-m4k.md` §5.3; `docs/investigations/m4k1-windows-timing-correction.md` §9 | R1 |
| D4 | S2 | A user-visible GPU error says "restart inspector" (an old internal name). | `gpu.rs` | R1 |
| D5 | S2 | The seating error does not explain itself. A 3-car train on the default 10 m track fails with `distance query outside generated track`. | M4K investigation §6.4, still unchanged | R8 |
| D6 | S2 | The car count accepts 1–256. Only 1- and 3-car curved trains (and straight 16-car) are qualified, and 16 cars already exceed the real-time budget. | `layout_ui.rs:568`; M4J report | R4 |
| D7 | S2 | The internal name appears in user data paths: `%LOCALAPPDATA%\QUANTUM-X\recent-v1.json` and the workbench preferences path. | `recent.rs`, `workbench.rs` | R1 |
| D8 | S3 | The packaging script copies the same 22 MB executable twice and ships `assets/development`. A single package is about 173 MB. | `package-windows.ps1` | R9 |

### 3.2 Architecture limitations

- **Simulation ownership.** The single `train_playback::State` belongs to the
  Editor `Session`. The M4K proposal's own first step, extracting a
  mode-independent `SimulationController`, was deferred. This blocks the
  separate Simulator window and multi-train operations.
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
- **GPU adapter selection.** Adapter selection is hard-coded:
  `PowerPreference::LowPower`, never forcing a fallback adapter, with no
  override. This blocks both hybrid-GPU correctness and any software-GPU CI
  experiment (§7.1).
- **Large binary assets in git.** 105 MB of PNG textures are stored directly
  in git, with no LFS and no compressed GPU format.

### 3.3 Performance risks

| Risk | Data (laptop, from cited reports) | Exposure |
| --- | --- | --- |
| Physics budget | 3-car: mean 1.3–2.3 ms, worst 5.5 ms; 16-car straight: 7.2 ms mean (budget 4.167 ms) | Longer trains, multiple trains, battery, unmeasured desktop |
| Energy drift | R8 fixture −15.10 J by tick 258 (M4H) | Full-ride (60–180 s) circuits not yet characterized |
| Rendering | Frame CPU p50 1.44 ms in hidden profile; **no visible-window FPS ever measured** | 4K, 240 Hz, iGPU, large support structures, second window |
| Worker CPU | 28–60 % of one core while playing | Battery life; multi-train scaling |
| Open/generation | Synchronous at open | Large projects |

### 3.4 User-experience problems

1. Simulating a ride requires development tools. A train exists only through
   the gray-box development control. Simulator entry is under
   *Window / Layout › Simulator Mode (development)…*.
2. There is no lift, station or dispatch button. The common first attempt,
   a level track with no initial speed, does nothing. That is correct physics
   but a dead end for users.
3. A console window appears behind the app (D2). Errors are invisible to
   non-developers.
4. The project extension `.qx.json` cannot carry a Windows file association
   or a custom icon.
5. Only one window: users cannot watch the ride while editing the layout.
6. Telemetry g-values are mass-weighted train averages. Users will read them
   as rider loads unless the presentation makes the difference clear.
7. There is no user documentation. More than 60 documents are pass-by-pass
   engineering reports.
8. There is no screen-reader support. Keyboard focus and high-DPI behavior
   have not been verified by hand.

---

## 4. Version 1.0 scope decision

### 4.1 Essential for 1.0 (P0)

1. **Windows application reliability (R1):**
   - a GUI-subsystem executable;
   - persistent logs;
   - crash and fatal-error reporting;
   - startup error handling;
   - GPU failure diagnostics and a recovery strategy.
2. **Project data safety (R2):**
   - autosave, recovery files and a recovery prompt;
   - backup before migration;
   - version compatibility.
3. **Verification infrastructure (R0):** tiered CI (§7.1) and the
   laptop-to-desktop verification flow (§9).
4. **Separate native Simulator window (R3, owner direction, §5):**
   - one authoritative simulation controller;
   - snapshot isolation;
   - speed control and single-step;
   - an honest display of the real-time rate.
5. **Single-train operational ride cycle (R5):**
   - station stop, hold and dispatch;
   - lift (chain or cable with anti-rollback);
   - tire or friction drives;
   - trim and block brakes;
   - all under a deterministic operations controller.
6. **Physics validation (R4)** for the shipped train configurations:
   real-time, full-ride energy and cross-machine evidence.
7. **Minimum production content (R6):**
   - at least one Available track style;
   - at least one original production train with validated mechanics;
   - seat and camera definitions for that train;
   - a train setup workflow outside development tools.
8. **Graphics baseline (R7):**
   - anti-aliasing;
   - a ground reference;
   - correct adapter choice on hybrid laptops;
   - frame rate measured on both owner machines.
9. **Installable distribution (R9):**
   - installer and uninstaller;
   - file association, which needs an extension decision;
   - versioning;
   - a decision on code signing;
   - a private release-candidate pipeline.
10. **User-facing documentation (R8):** a getting-started guide, a shortcut
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
captive-guide rigid-body model, clearly labeled, and validated only within the
scope R4 establishes. Real coaster wheel assemblies are captive (running,
side-friction and up-stop wheels), so a bilateral guide is a defensible
first-order model. Unilateral contact is a later fidelity upgrade, not a
precondition. Make no ride-safety or certification claim in any version.

---

## 5. Separate native Simulator window

### 5.1 Owner direction (established requirement)

The separate Simulator window is an established product direction, not an
open option. It requires:

- a separate native Windows Simulator window;
- the Editor remaining **open and usable** while the Simulator runs;
- independent window positioning and resizing;
- an independent Simulator camera and playback controls;
- **one** authoritative project and **one** simulation controller;
- **no duplicate physics engines**: one worker, one clock, one `World` set per
  simulation;
- **snapshot isolation** as the preferred architecture (§5.3). The Simulator
  keeps running on its frozen track snapshot while the Editor is modified.
  When the committed layout changes, it says that an updated layout is
  available and offers **Reload**.

Precise GUI composition, shortcuts and window behavior remain subject to owner
approval. This PR documents the direction only. The multiwindow architecture
is **not** implemented here; it is milestone R3.

### 5.2 Current state and feasibility

**Today:** `panels.rs` hands the single window to `simulator_ui::workspace`.
Return swaps the window back. The Editor `Session` survives in memory but is
not visible.

**Feasibility:** high. Everything needed is already in the dependency set:

- winit 0.30 supports multiple windows in one event loop.
- wgpu allows one `Instance`/`Adapter`/`Device`/`Queue` to drive several
  `Surface`s.
- egui-winit allows one `egui::Context` plus one `egui_winit::State` per
  window.

No new dependency is required. The real work is extracting simulation
ownership from the Editor session; adding a second surface is the smaller
part.

**Proposed structure** (illustrative names):

```text
App (event loop, routes by WindowId)
 ├─ GpuShared: Instance, Adapter, Device, Queue, pipelines, mesh/material caches
 ├─ ProjectSession: Editor (doc + undo), path/dirty, generation service, drafts   ← the one authoritative project
 ├─ SimulationController (extracted from train_playback::State)                   ← the one simulation controller
 │    source key · frozen input snapshot · single worker · epoch · reset snapshot · telemetry
 │    operations controller state · input/event log
 ├─ EditorWindow:    surface, egui ctx, editor camera bank, layout     → reads/writes ProjectSession
 └─ SimulatorWindow: surface, egui ctx, simulator camera bank,          → reads SimulationController
                     playback controls, own position/size                  (never writes the document)
```

### 5.3 Snapshot isolation (preferred architecture)

1. **Prepare.** Preparing a simulation captures a **source key**: committed
   revision, generation identity, train revision, operations revision and
   initializer policy. It also freezes the immutable inputs.
2. **Run.** The simulation runs only against that frozen snapshot. Editor
   edits, drafts and generation never touch it.
3. **Detect change.** When the Editor commits a change that alters the
   physical inputs, the Simulator compares its source key with the current
   committed key and shows **"Updated layout available — Reload"**. It keeps
   running on the old snapshot until the user chooses Reload.
   - Drafts and visual-only changes do not count as committed physical
     changes.
   - The exact rules extend the M4K source-key proposal.
4. **Reload.** Reload retires the old epoch and prepares a fresh simulation
   from the current committed revision, using the same fail-closed readiness
   checks as M4K entry. If preparation fails, the old snapshot stays on
   screen, paused, together with the error message.
5. **Determinism.** Each run is tied to exactly one source key and replays
   bit-identically for the same inputs. Geometry is never hot-swapped under a
   running train.

**Alternatives rejected:**

- Locking the Editor while simulating defeats the "Editor remains usable"
  requirement.
- Hot-swapping geometry mid-run breaks determinism and replay.

### 5.4 Behaviors needing owner approval (not decided here)

- Simulator window composition.
- Entry command and its placement.
- Shortcuts and focus rules.
- Whether closing the Editor also closes the Simulator. The unsaved-changes
  guard stays with the Editor in all cases.
- What closing the Simulator does to the running simulation. Today's Return
  retires the epoch.
- Whether window placement is remembered (application preferences, never the
  project).
- Behavior on a single monitor.
- Whether the same-window mode remains as an option.
- How the Reload indicator looks.

### 5.5 Risks

- Two surfaces add load on an integrated GPU (measured in R7).
- Keyboard focus and shortcut conflicts.
- Device loss must protect both windows (R1/R2).
- Each window paces itself to its own monitor's refresh rate. There is one
  physics clock; presentation interpolation is per window.

---

## 6. Completing operational ride simulation without compromising determinism

**Principle.** Operations are a *controller layer*. It reads physical state and
writes bounded actuator commands at fixed tick boundaries. It never assigns
positions or velocities (AGENTS.md §8.1). It runs inside the single simulation
controller from §5; it is not a second engine.

1. **Single clock.**
   - Controllers run on the physics worker, once per 1/240 s outer step,
     before the force evaluation.
   - Their state (timers, block occupancy, dispatch latches, lift engaged)
     is part of the snapshot. Reset restores it exactly.
   - Nothing in the controller reads wall-clock time.
2. **Inputs as tick-stamped events.**
   - Simulator UI actions (Dispatch, E-stop, manual mode) go into a queue.
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
   - Use one `World` per train inside the one controller, stepped in a fixed
     order. They interact only through the block controller, so there are no
     train-to-train contacts before collision work.
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
- A laptop ↔ desktop comparison of golden fixtures. Either prove the results
  are bit-identical, or document the measured divergence and a tolerance.
  Do not assume either outcome.

---

## 7. Windows packaging, startup, recovery and release automation — audit

| Area | Today | 1.0 requirement | Milestone |
| --- | --- | --- | --- |
| Subsystem | Console | GUI-subsystem `QUANTUM Dynamics.exe` with no unwanted console; a separate small `quantum-x` dev/CLI binary (or feature-gated build) so scenario harnesses do not ship to users | R1 |
| Routing | By executable file name | Explicit: the GUI binary always opens the app; the CLI binary always runs the CLI | R1 |
| Startup failure | stderr line + exit record | Native message dialog with the reason, log path and next step; same exit codes | R1 |
| Logging | stderr JSON exit record | Persistent rolling log under `%LOCALAPPDATA%\QUANTUM Dynamics\logs`, bounded size, no personal data | R1 |
| Crash / fatal error | A main-thread panic prints to stderr (invisible without a console) and exits with code 101 | **R1:** panic hook and fatal-error path write a crash report and log entry, then show a dialog. **R2:** the same path writes an emergency recovery file of the open document. R1 alone does not claim the document is protected. | R1 + R2 |
| GPU loss | Exit | **R1:** diagnostic log and user message, plus a documented recovery strategy: recreate the device where possible, otherwise shut down in a controlled way. **R2:** recovery file written before any shutdown. | R1 + R2 |
| Adapter | LowPower, no override | HighPerformance by default; a Setup option and an environment override (also needed by §7.1 Tier 2); adapter shown in About/Diagnostics | R1 (override needed early by R0 Tier 2) |
| Compatibility | Schema 1→7 migration; newer files rejected generically | Backup before migrating; `saved_with` version; "This project was saved by QUANTUM Dynamics X.Y" message; a fixture corpus per schema in CI | R2 |
| Package | Portable folder, 2 × 22 MB exe, about 173 MB | Installer (per-user, no admin), Start menu, uninstall that keeps user projects, file association, version in Programs & Features; an optional portable zip | R9 |
| Extension | `.qx.json` | Owner decision: a single unique extension (for example `.qxd`) for association, still reading `.qx.json` | R9 |
| Signing | Unsigned | Owner decision. Unsigned installers trigger SmartScreen warnings; a signing certificate has a recurring cost | R9 |
| User data paths | `%LOCALAPPDATA%\QUANTUM-X\…` | `%LOCALAPPDATA%\QUANTUM Dynamics\…`, with a one-time migration that keeps the old files | R1 |
| Release automation | None | Tiered CI (§7.1). Tagged builds produce a **draft** release artifact only; publishing stays a manual owner action | R0, R9 |
| Third-party licenses | Manual copy of a few notices | Generated notice file from `Cargo.lock` (for example `cargo-about`); license allow-list check (for example `cargo-deny`) | R9 |

The installer technology (WiX/MSI, MSIX or Inno Setup) is a technical choice
to make in R9. MSIX has the best uninstall hygiene but needs signing. Inno and
WiX work unsigned.

### 7.1 CI feasibility and test tiers

**Evidence gathered 2026-11-14:**

- **No GPU on standard runners.** GitHub's runner specifications list no GPU
  for the standard `windows-latest` / `windows-2025` runners.
  - GPU runners exist only as paid "larger runners" (an NVIDIA T4 class)
    tied to Team or Enterprise Cloud plans.
  - This roadmap assumes **no** hosted GPU.
- **wgpu installs software adapters.** wgpu's own CI
  (`gfx-rs/wgpu` `.github/workflows/ci.yml`) runs its Windows GPU tests on
  `windows-2025` runners. It explicitly installs **WARP** (a software DX12
  adapter) and **Mesa** (software Vulkan) before testing.
  - So software adapters can work on hosted runners, but must be installed
    and selected deliberately.
  - wgpu's suite is mostly headless (no window surface). It does not show
    that a winit window plus swapchain presents on a hosted runner.
- **Our default suite needs no GPU** (source inspection of this checkout).
  - The default `cargo test --workspace` suite requests no GPU adapter.
  - Every offscreen or windowed GPU path is opt-in: `#[ignore]`
    (`headless.rs` `gui_smoke`, `graph_precision_offscreen_evidence`), gated
    by an environment variable (`QX_SEGMENT_RENDER_TEST_DIR`), or reachable
    only from CLI scenarios.
  - Confirm this on the first real CI run; a passing local run is not proof.
- **Our code cannot choose a software adapter.**
  - `gpu.rs` and `recording.rs` request `LowPower`.
  - `offscreen.rs` and `material_probe.rs` request the default options.
  - None sets `force_fallback_adapter`, and there is no adapter override.
  - Whether wgpu selects an installed WARP adapter without being forced has
    **not** been tested.

**Test tiers:**

| Tier | Runs on | Contents | Gate status |
| --- | --- | --- | --- |
| **1 — Standard CI** (no GPU) | GitHub-hosted Windows runner | `cargo fmt --check`; clippy `-D warnings`; default `cargo test --workspace --locked`; release build; `package-windows.ps1` self-checks (`--version`, `--self-check`, `--startup-check`, which create no window or device); CPU-only CLI exports and their Python verifiers (each verifier classified during R0 as rendering or not); golden determinism hashes (physics is CPU-only); schema corpus; license check | **Blocking** for PRs to `main`, once proven green |
| **2 — Software GPU** (experimental) | Hosted runner with WARP and/or Mesa installed, using the adapter override from §7 | Offscreen render scenarios; hidden-window `--gui-smoke-test` welcome/edit/simulator | **Non-blocking** until feasibility is demonstrated. Proposed promotion criterion (provisional): 20 consecutive successful runs with no flaky failures. Proves code paths only, never performance or visual quality. |
| **3 — Owner hardware** | Laptop (Intel iGPU) and desktop (discrete GPU) | Native GPU smokes; `simulator-timing` and every timing or performance gate; visible-window FPS; visual quality review; DPI, multi-monitor, focus; Explorer double-click; installer and uninstall; long ignored release-mode physics controls | Required for release candidates (§9); never on shared CI runners |

Shared CI runners are unsuitable for timing gates because their CPU timing
is noisy. Self-hosted runners on the owner's machines are **not** recommended
for this public repository: pull-request jobs would execute untrusted code
there. If one is ever added, it must be restricted to trusted branches. That
needs a separate owner decision.

---

## 8. Roadmap

The milestones use an **R** prefix (Readiness) to avoid colliding with the
existing M-series names.

### 8.1 Provisional numeric targets

Every number in an acceptance criterion below is a **provisional engineering
target**:

- Each one must be measured on the owner's hardware before it is used as a
  gate.
- Each one needs owner approval before it binds.
- Each one may be revised from evidence.

Existing numerical tests, tolerances and qualification scopes stay as they
are. A target in this roadmap never justifies weakening them.

| Target | Provisional value | Milestone | Measured on | Status |
| --- | --- | --- | --- | --- |
| Autosave loss window | ≤ 60 s or ≤ 20 commands of lost work | R2 | Laptop | Provisional |
| Simulator window open/close stability | 100 cycles, no leak; memory within a stated bound | R3 | Laptop + desktop | Provisional |
| Physics step, shipped reference train (for example ≤ 8 cars) | p99 ≤ 2.0 ms desktop; ≤ 3.3 ms (80 % of 4.167 ms) laptop, AC and battery | R4 | Laptop + desktop | Provisional |
| Real-time pacing | 0 catch-up events over a 180 s ride | R4 | Laptop + desktop | Provisional |
| Full-ride energy | Error reported per fixture and within an owner-accepted bound (not yet proposed; needs the R4 ledger first) | R4 | CPU, any | Open |
| Rendering | Desktop ≥ 60 FPS at 1440p (p95 ≤ 16.7 ms); laptop iGPU ≥ 60 FPS at 1080p or a documented quality preset | R7 | Laptop + desktop | Provisional |
| Package asset size | ≤ 80 MB without visible loss, signed off by the owner | R7 | — | Provisional |
| Simulator soak | 2 h; memory growth ≤ 5 % after warm-up; no catch-up drift | R10 | Desktop | Provisional |
| Software-GPU CI promotion | 20 consecutive successful Tier 2 runs | R0 | CI | Provisional |

### 8.2 Dependencies

```text
R0 CI & verification ──┬─▶ R1 Windows reliability ──▶ R2 Project data safety ─────────────┐
                       │                                                                   ▼
                       ├─▶ R3 Sim controller + separate window ──▶ R5 Operations ─▶ R6 Content ─▶ R10 RC
                       ├─▶ R4 Physics validation & performance ─────▲      ▲                ▲
                       ├─▶ R7 Graphics baseline ────────────────────────────────────────────┤
                       ├─▶ R8 UX completion & docs (Owner) ─────────────────────────────────┤
                       └─▶ R9 Packaging & release pipeline (needs R1, R2) ──────────────────┘
```

"Owner" marks design approvals that block **completion**, not the start of
work.

### R0 — Verification infrastructure (P0, first)

- **Scope:**
  - Tier 1 CI on a GitHub-hosted Windows runner (§7.1).
  - A golden determinism fixture set (pose hashes at fixed ticks).
  - A schema fixture corpus job.
  - A Tier 2 feasibility spike: install WARP and/or Mesa as wgpu's CI does,
    and use a minimal adapter-override environment variable, implemented with
    or ahead of the R1 adapter work.
  - A `tools/verify-desktop.ps1` script (proposed) that turns the Tier 3
    desktop checklist into JSON results.
- **Acceptance:**
  - Tier 1 is required on every PR to `main`, with 10 consecutive green
    merges (provisional).
  - The first CI run confirms that the default suite passes without a GPU.
  - Golden replay hashes are stored and compared.
  - The CI package passes all `package-windows.ps1` self-checks.
  - Tier 2 has a recorded feasibility verdict: works, flaky or not feasible,
    with logs.
  - Tier 2 is **not** a required check unless it meets its promotion
    criterion.
- **Tests:** existing suite; golden-hash comparator; Tier 2 spike job.

### R1 — Windows application reliability (P0; needs R0)

R1 makes failures visible and diagnosable. **It does not protect documents.**
Document protection is R2.

- **Scope:**
  - **Native GUI executable** with no unwanted console. Split the public and
    dev/CLI binaries, or attach a console only for CLI use. Routing is
    explicit, not based on the file name (fixes D2, D3).
  - **Persistent diagnostic logs:** a rolling, size-bounded log under
    `%LOCALAPPDATA%\QUANTUM Dynamics\logs`, containing no personal data.
    Includes a user-data path migration away from `QUANTUM-X` that keeps the
    old files (D7).
  - **Crash and fatal-error reporting:** a panic hook and fatal-error path
    write a crash report with version, adapter and stage, plus a log entry,
    then show a native dialog that names the report location.
  - **Startup error handling:** a native dialog for window, surface, adapter,
    device and first-frame failures, with the existing exit codes and exit
    record preserved.
  - **GPU failure diagnostics and recovery strategy:**
    - device-lost and uncaptured errors are logged with details;
    - the user sees a message without internal names (D4);
    - a written, tested strategy says when the device is recreated and when
      the app shuts down in a controlled way;
    - HighPerformance adapter by default, with a Setup option and an
      environment override.
  - Correct the M4K and M4K.1 checklist launch commands (D3).
- **Acceptance:**
  - Double-clicking the packaged executable shows no console.
  - An injected panic, in both Editor and Simulator, produces a crash report,
    a log entry and a dialog.
  - Injected device loss (for example `Device::destroy` in a test build)
    produces a log entry and a user message, and follows the documented
    strategy. The process never exits silently.
  - With no usable adapter, the app shows a dialog, exits with code 1 and
    writes a log entry.
  - `cargo run -p quantum-x` opens the app, or prints an explicit hint.
  - Existing CLI scenarios pass from the dev binary.
  - **R1 makes no claim that unsaved work survives a crash.** That is
    verified in R2.
- **Tests:** startup routing unit tests; injected-fault native smoke (Tier 3,
  and Tier 2 if promoted); package self-check updated for one GUI binary.

### R2 — Project data safety (P0; needs R1)

- **Scope:**
  - **Autosave:** periodic and after significant commands, written atomically
    off the UI thread to a recovery folder, never over the user's file.
  - **Recovery files:** an emergency recovery write hooked into R1's
    crash/fatal and GPU-loss paths.
  - **Recovery prompts:** on the next launch, offer, compare or discard
    recovered work. Never offer recovery for a project that has since been
    saved.
  - **Backup before schema migration:** a timestamped copy before any
    migrated file overwrites an older-schema file.
  - **Project version compatibility:**
    - `saved_with` application-version metadata;
    - a clear message for newer files;
    - a written 1.x policy: additive schemas, explicit migrations, never
      silently dropped data;
    - a checked-in corpus with at least one fixture per schema 1…N;
    - an owner decision on bundling external GLBs (folder versus
      single-file).
- **Acceptance** (values provisional, §8.1):
  - Killing the process during editing, and an injected panic or device
    loss, then relaunching, restores work within the autosave loss window.
  - All corpus fixtures load, migrate, save and reload identically.
  - Truncated and corrupted inputs (property-generated) produce errors,
    never panics.
  - An older-schema file is never overwritten without a backup.
- **Tests:** process-kill integration test; injected-fault recovery test
  reusing R1's harness; corpus round-trip; malformed-input property tests;
  migration backup test.

### R3 — Simulation controller extraction and separate Simulator window (P0; owner direction §5; composition approval; needs R0)

- **Scope:**
  - Extract one `SimulationController` from the editor-owned
    `train_playback::State`.
  - Multi-window `qx-rendering`: shared device; a surface, egui context,
    input state and camera bank per window.
  - A separate Simulator window with independent position, size, camera and
    playback controls.
  - Snapshot isolation with an "Updated layout available — Reload"
    indicator (§5.3).
  - Speed control (fixed-step scheduling only), single-step, and a displayed
    real-time rate.
  - Composition, shortcuts and window behaviors per owner approval (§5.4).
- **Acceptance:**
  - The Editor stays usable for editing, undo/redo and saving while the
    Simulator runs.
  - The process has exactly one simulation worker and one controller, tested
    by assertion.
  - All existing `simulator_tests` editor-identity assertions pass.
  - Committing an Editor change never alters the running simulation's source
    key. The indicator appears for committed physical changes only, and
    Reload prepares from the new revision.
  - A failed Reload keeps the previous snapshot paused, with the error shown.
  - Physics replay is bit-identical to the current same-window mode for the
    same inputs.
  - Open/close stability target met (provisional).
  - Closing with unsaved changes still prompts.
- **Tests:** controller unit tests (stale epoch, cancel, reset, reload
  failure); two-window hidden smoke (Tier 3; Tier 2 if promoted); open/close
  cycle soak; owner checklist on the laptop (single monitor) and desktop
  (multi-monitor, mixed refresh).

### R4 — Physics validation and performance (P0; needs R0)

- **Scope:**
  - Profile the step.
  - Evaluate solver changes, such as a convergence-based early exit, **only**
    against the existing suites at their current tolerances.
  - Define the shipped train envelope (car count, curvature, speed range) and
    limit the UI to it (D6).
  - Build a full-ride energy ledger for 60–180 s circuits.
  - Compare golden fixtures between laptop and desktop.
  - Validate the mechanics of the first production train (with R6).
- **Acceptance** (values provisional, §8.1):
  - The step-time and pacing targets are met for the shipped envelope on
    both machines.
  - The energy behavior over a full ride is measured and reported, and the
    owner accepts a bound.
  - Cross-machine results are bit-identical, or the divergence is measured
    and documented.
  - All M4A–M4J regression suites, including the ignored release controls,
    pass **unchanged**.
- **Tests:** solver A/B harness on fixed fixtures; full-ride energy ledger;
  golden cross-machine hashes; existing `velocity_initialization` and M4I
  suites (Tier 3 for the timing parts).

### R5 — Operational ride simulation (P0 single-train; P1 multi-train; owner UX; needs R3, R4, R2 schema)

- **Scope:**
  - The deterministic operations controller (§6), inside the R3 controller.
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
  - Cycle time is reported, not asserted against a real ride.
  - Saving and reopening keeps all operations authoring.
- **Tests:** per-device analytic checks; state-machine property tests;
  event-log replay; schema migration test.

### R6 — Production content minimum (P0; owner; needs R4, R5)

- **Scope:**
  - Final owner-approved Box Spined-100 dimensions.
  - At least one original production train: GLB, mechanical definition,
    seats and camera mounts, validated in R4.
  - A train setup workflow outside development tools.
  - A decision on how Planned catalog entries are presented.
- **Acceptance:**
  - A new user can create a project, assign style and train, and simulate
    without opening development tools.
  - Asset provenance and licenses are recorded.
  - The gray-box train remains available only as a labeled development
    fixture.

### R7 — Graphics production baseline (P1 overall; anti-aliasing and adapter choice are P0; needs R0)

- **Scope:**
  - MSAA or an equivalent anti-aliasing method.
  - Ground plane and horizon.
  - Optional directional shadows.
  - Compressed textures (KTX2/BC7) with a package-size budget.
  - Frame-time instrumentation in visible windows, including the second
    (Simulator) window.
- **Acceptance** (values provisional, §8.1):
  - The rendering and package-size targets are measured on both machines in
    visible windows.
  - The reference scene is a full circuit with supports and a moving train.
- **Tests:** offscreen image regression compared against references within a
  tolerance (Tier 2 if promoted, otherwise Tier 3); frame-time capture
  script (Tier 3).

### R8 — UX completion, accessibility and documentation (P0 where blocking; owner)

- **Scope:**
  - Owner design passes for Simulator composition, Operations and train
    setup.
  - Error-message audit, including the seating message (D5).
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
  - One executable copy per package (D8).
  - Version scheme: semver plus git hash in About and logs.
  - Signing decision.
  - Generated third-party notices and license policy.
  - Tier 1 CI produces draft release artifacts on tags. No public
    publication.
- **Acceptance:**
  - On the desktop, in a clean Windows 11 user account: install, launch,
    open a project by double-clicking its file, and uninstall. User
    projects remain; app files and Start menu entries are removed.
  - Upgrading RC n → RC n+1 keeps preferences and recent projects.
  - The CI artifact hash equals the hash verified on the desktop.

### R10 — Release-candidate validation and 1.0 decision (P0; needs all P0)

- **Scope:**
  - Feature freeze.
  - Tier 3 validation of release-candidate packages on the desktop.
  - Simulator soak.
  - The full project corpus.
  - Owner acceptance checklist.
  - Known-issues list.
- **Acceptance:**
  - Zero open S1 issues; every S2 fixed or explicitly accepted by the owner.
  - The soak target is met (provisional).
  - The owner signs off.
  - Publication remains a separate, explicit owner action.

**Suggested order for one developer:**

1. R0 → R1 → R2. This is reliability work and needs no UX decisions.
2. R4 in parallel with owner composition design for R3 and R5.
3. R3 → R5 → R6.
4. R7 and R8 throughout.
5. R9, then R10.

---

## 9. Laptop-development → desktop-distribution verification strategy

**Machine roles (owner direction):**

- **Laptop: primary development machine** (Intel integrated GPU, Vulkan,
  240 Hz). Source development, the full local test suite, quick native GPU
  smokes, integrated-GPU checks, and initial timing measurements.
- **Desktop: packaged Windows builds and validation.** Runs packaged builds
  only, never source builds. Used for discrete-GPU testing, visual quality
  assurance, installer checks and release-candidate verification. Needs no
  Rust toolchain.
- **CI: Tier 1 builds and checks** (§7.1). The canonical package source once
  R0 lands. It never replaces the Tier 3 checks on the owner's machines.

**Flow per candidate:**

1. **Laptop.** Before pushing, run `cargo fmt --check`, clippy and
   `cargo test --workspace`. For UI or simulation changes, also run the
   relevant `--gui-smoke-test` locally (Tier 3). Do not build packages
   locally for every change.
2. **CI.** On PR or merge, run Tier 1 and produce **one** package per commit,
   named `QUANTUM Dynamics-<semver>-<gitshort>-windows`, with a SHA-256
   manifest. Until R0 lands, the laptop builds that single package once per
   candidate with `package-windows.ps1`.
3. **Transfer.** The desktop takes that one package. Never copy a laptop
   `target/` folder. One package per commit avoids duplicate artifacts.
4. **Desktop verification** (`verify-desktop.ps1`, proposed), writing one JSON
   file:
   - hash check against the manifest;
   - `--version`, `--self-check`, `--startup-check`;
   - `--gui-smoke-test` welcome, edit and simulator on the discrete GPU;
   - `simulator-timing` on the reference fixtures;
   - golden replay hash comparison with the laptop and CI;
   - after R9, an install and uninstall cycle in a scratch user account.
5. **Desktop manual checks (visual QA):** recorded in the PR or
   release-candidate issue.
   - Explorer double-click with no console.
   - Render quality and visible-window FPS.
   - DPI at 100/150/200 % and multi-monitor setups with mixed refresh rates.
   - Separate Simulator window: placement, resize, focus, Reload indicator.
   - Driver or device reset while running.
6. **Evidence.** Commit only the small JSON summary and the checklist result
   under `docs/evidence/<rc>/`. Binaries, captures and logs stay in CI
   artifacts or local `target/`, never in git.
7. **Hygiene.** Keep the latest N CI artifacts. Locally, delete old
   `target/distribution/*` folders by hand; packaging already refuses to
   overwrite.

**Coverage limits:**

- The laptop's Intel integrated GPU and the desktop's discrete GPU cover two
  configurations. AMD and older Windows 10 builds remain **unverified**
  unless an external tester is added.
- Tier 2 software-GPU CI, if promoted, proves code paths only, not
  performance or visual quality.

---

## 10. Owner decisions

**Established direction:**

- A separate native Simulator window with snapshot isolation and Reload
  (§5.1).
- Laptop for development, desktop for packaged and release validation (§9).

**Still open:**

1. Simulator window details (§5.4): composition, entry, shortcuts, focus and
   close behavior, placement memory, the same-window option, the Reload
   indicator.
2. Operations authoring UX: devices, blocks, station and dispatch controls.
3. Train setup workflow and the first production train: art, mechanics,
   seats.
4. Final Box Spined-100 dimensions, and how Planned catalog entries appear.
5. Project file extension, and single-file bundle versus folder for external
   GLBs.
6. Code signing (cost) and installer technology preference.
7. Whether multi-train operation is required for 1.0 or moves to 1.1.
8. Approval of the provisional numeric targets (§8.1), after hardware
   measurement.
9. Whether to accept the Tier 2 software-GPU CI verdict, or rely on Tier 3
   only.

---

## 11. Audit verification record

- **Checkpoint inspected:**
  - `origin/main` `25ae183`, fetched 2026-11-14.
  - Branch `claude/production-readiness-audit` (PR #16) was created from it.
  - This document is the only file added or changed by the PR. No production
    code changed.
- **CI evidence (§7.1):**
  - GitHub runner documentation (runner specifications and GPU larger
    runners);
  - `gfx-rs/wgpu` `.github/workflows/ci.yml`, read 2026-11-14;
  - source inspection of adapter requests and ignored or env-gated tests in
    this checkout.
  - No CI job was run.
- **Not run:**
  - clippy and release builds (no code changed; avoids large builds);
  - `package-windows.ps1` (avoids duplicate artifacts);
  - native GUI smokes;
  - Python verifiers;
  - any WARP or Mesa experiment.
- **Not manually verified:** everything that needs the desktop UI. No desktop
  control was used.
- **Quoted facts:** timings, energy values and test counts are attributed to
  their reports and were not re-measured here. Those reports are M4K, M4K.1,
  M4J, M4H, the integrated-core report and the 2026-10-30 publication audit.

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

### 11.2 Revision history

- **2026-11-14, initial audit.**
- **2026-11-14, owner review.**
  - Qualified physics maturity: verified, experimental and unvalidated tiers
    in §1 and §2.2.
  - Recorded the separate Simulator window as owner direction, with snapshot
    isolation and Reload (§5).
  - Separated R1 (Windows reliability) from R2 (project data safety). R1 no
    longer claims document recovery.
  - Marked every numeric target provisional (§8.1).
  - Replaced the mandatory WARP smoke with evidence-based CI tiers (§7.1).
  - Restated the laptop and desktop roles (§9).
  - Kept R0–R10 unchanged in number and order.
