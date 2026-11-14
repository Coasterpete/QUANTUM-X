# M4K — Simulator Mode foundation (first vertical slice)

Status: **implemented development slice**. The workflow is real and tested
end to end. The visible Simulator composition and its entry placement are
**development-only, provisional UI**: no owner Simulator design exists yet.
Architecture reference: [simulator-mode-architecture-m4k.md](simulator-mode-architecture-m4k.md).

Branch `claude/m4k-simulator-mode`, based on `main` at `1ec98a8` (PR #14, GUI
startup diagnostics, is merged and contains `10ed2f0`). Not pushed.

## 1. Architecture findings (verified by source inspection)

| Area | Finding | M4K use |
| --- | --- | --- |
| Document / history | `Session` owns the one `Editor` (undo/redo), path, saved-state comparison, drafts and selection | Kept alive and untouched for the whole visit; no reload on return |
| Physics | `train_playback::State` drives **one** worker thread: `TrackSimulation`, fixed 1/240 s steps, epoch-tagged requests, reset snapshot, bounded catch-up | Reused as is; Simulator retires any editor playback and prepares a fresh simulation, so there is never a second worker, clock or physics path |
| M4J initializer | Opt-in via `QX_EXPERIMENTAL_TRAIN_VELOCITY=compatible-lead` at launch | Unchanged; the Simulator inherits the same policy and shows the M4J report when active |
| Presentation | `ride_camera::present` latches one interpolated pose sample per frame for both train instances and attached cameras | Reused unchanged; the Simulator offers only cameras attached to the physical pose (Free, Rider, Front, Rear, Chase); geometric Track Preview is excluded |
| Renderer | `qx-rendering` keeps per-profile camera/display banks selected by `SurfaceOutput::profile` (was 2: Track, Train) | Third bank (index 2) for the Simulator; the editor bank is restored exactly on return |
| Performance | Track mesh is cached by `Arc` identity, but **any** `DisplayOptions` change rebuilt line vertices, so a moving train regenerated them every frame (also true in existing editor playback) | Presentation-only changes now upload train instances only (see §3) |
| Persistence | Workbench layouts are persisted per Track/Train profile | Not changed: Simulator is not a workbench profile and adds no persisted state or schema |
| Audio / environment | `qx-audio` is a placeholder; no environment model | Out of scope, not touched |

Risks identified: the editor-owned playback `State` is shared rather than
extracted into a separate controller (acceptable for one consumer at a time;
see §6). M4J remains a bounded experimental initializer. Production dynamics
are an ideal bilateral-guide rigid-body model, not wheel/rail contact.

## 2. Boundaries and lifecycle

```text
Editor ──(entry request, readiness OK)──▶ Preparing ──(worker: valid tick-0 state)──▶ Simulator
  ▲            │ readiness error                 │ failure / cancel / document change      │
  └────────────┴── error modal, nothing changed ─┴──────────────────────────────────────────┘
                                         Return to Editor / close / open / invalidation
```

* `authoring/simulator.rs` holds the transient mode state machine (`Phase`),
  readiness checks, entry, preparation polling, exit and shortcuts. It holds no
  document, physics state or worker.
* Entry requires the committed document: no drafts (region, construction,
  heartline, operations, metadata and so on), no generation in flight or draft
  preview, no generation error, and an associated **saved** train. A nominal
  rider-reference ID such as `nominal-reference-point-only` is refused with an
  actionable message.
* Entry fails closed. Seating or initialization failures, a worker error, or a
  document revision change during preparation all return to the Editor with a
  modal message. There is never a partial Simulator.
* While preparing or simulating, viewport picks and hover inspection are
  ignored, so editor selection and inspection station are preserved.
* Close and open requests first return to the Editor, so the existing
  unsaved-changes guards always run.
* Return retires the simulation (`discard`: worker cleared, epoch advanced so
  late packets are ignored) and restores the editor ride-camera mode. The
  Simulator camera choice is remembered for the session only.
* Simulation never writes the document: no command is issued from Simulator
  code, and tests assert project, history, dirty state, selection, draft,
  layouts and inspection state are identical after return.

## 3. Rendering and performance

* The same wgpu device, surface, scene and egui pipeline are used. Nothing is
  Vulkan-specific, and no dependency was added.
* `DisplayOptions::differs_only_in_presentation` (exhaustive destructuring)
  classifies `asset_instances`, `camera_view`, `train_bounds`,
  `selected_car_bounds`, and `inspection_distance_m` while the marker is hidden,
  as presentation only. Those changes now call `Gpu::upload_assets` instead of
  a full line-vertex and scene rebuild. This also applies to existing editor
  playback when the inspection marker is hidden. **Frame-time benefit was not
  measured.**
* Physics timing is unchanged: wall-clock-scheduled 1/240 s fixed steps on the
  worker, with interpolation for presentation only.

## 4. Implemented UI (development-only)

* Entry: **Window / Layout › Simulator Mode (development)…**, disabled with the
  readiness reason as hover text. This placement does not change the
  owner-approved command row; an earlier command-row button wrapped small
  windows and broke the existing m5/m6 layout tests.
* Preparing: a modal with a spinner and **Cancel**. Failures appear in a
  "Simulator unavailable" modal; the project, history and layout are unchanged.
* Simulator composition: a top bar with the `SIMULATOR MODE` indicator, project
  name, save state and **Return to Editor**; a right panel with **Play/Pause**
  (Ctrl+Space), **Reset**, state, tick and simulated time; telemetry; camera
  choice (F6 cycles), Fit Train/Fit All (F over viewport) and track display
  mode; the viewport fills the rest. The window title reads
  `QUANTUM Dynamics — Simulator — <project>`.
* Telemetry is read only from worker `TrainTelemetry` and `Diagnostics`: speed,
  distance traveled, longitudinal acceleration, mass-weighted body-local
  specific force in g (labeled as *not* a rider-seat accelerometer),
  device/resistance force, constraint residuals, catch-up and starvation
  notices, and the M4J report when that initializer is active. Nothing is
  invented or placeholder.

## 5. Verification

Automated, on this Windows laptop (Intel integrated graphics, Vulkan,
driver 101.8724):

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace --no-fail-fast` | pass: app 198 passed / 1 ignored; all crate suites pass (including qx-physics `velocity_initialization` 8 passed, `default_velocity_compatibility`, `dynamics`, `devices`) |
| New `simulator_tests` (5) | Real egui passes over the live Session and production worker: menu entry, Play/Pause/Reset buttons, Return button, exact reset poses, editor-state identity, edit then re-entry, refusals (no train, nominal reference, drafts, generation), failed seating at station 0, document change while preparing, close guard |
| New renderer unit test | presentation-only classification |
| `--gui-smoke-test welcome` / `edit` (release) | pass, 5 frames presented, resources released |
| `--gui-smoke-test simulator` (release, development fixture) | pass on two builds. Native hidden-window round trip: entry → run until ≥ 48 ticks (49 and 53 observed, for example 49 ticks = 0.2042 s simulated, exactly ticks/240) → pause → reset equal to initial poses → editor restored → resources released |
| `--gui-smoke-test simulator examples/heartline-reference.qx.json` | expected failure: refused after the first frame because the rider reference is a nominal configuration, not a saved train; resources released |
| M4J ignored release controls / M4I feature suite | see §5.1 |

The `--gui-smoke-test` frame counts and timings are evidence of a working
native path, **not** performance measurements: the hidden loop draws on a
5 ms poll.

### 5.1 M4I/M4J numerical regressions

`crates/qx-physics`, `qx-document` and `qx-geometry` have **no changes** in
this branch. The default M4I/M4J tests passed in the workspace run. Ignored
release controls and the feature-gated M4I suite are recorded below.
`tools/verify-velocity-initialization-m4j.py` was **not run**: Python/NumPy is
not installed on this machine.

| Command | Result |
| --- | --- |
| `cargo test -p qx-physics --release --test velocity_initialization -- --ignored --test-threads=1` | 2 passed (`four_second_production_continuation`, `initialization_cost_separate_from_steady_stepping`), 23.2 s |
| `cargo test -p qx-physics --release --features experimental-source-guidance --test source_guidance_m4i` | 4 passed, 4 ignored |

The four ignored M4I items are long native release research experiments
(six-second references, 563-tick sweeps and trajectory refinement). They were
not run because the physics crate is unchanged in this branch.

### 5.2 Intermittent test failure (open item)

`authoring::simulator_tests::editor_simulator_editor_round_trip_preserves_the_open_document`
failed **once** in a full `cargo test --workspace` run, at the editor-state
identity assertion after Return. Bounded investigation:

* Frequency: 1 failure in 10 full app-binary runs (the other 9 ran either
  through cargo or directly from the correct crate working directory, including
  the final verification run), and 0 failures in 40 focused runs of that test
  alone, 4 concurrent at a time.
* The failing field is unknown: the original assertion printed two whole
  `Project` values and the log was truncated. The assertion was replaced by
  `assert_same`, which names each differing field. The fields and strictness
  are unchanged; the test was not weakened.
* Ruled out: the Train Summary station widget (it writes only on user input);
  an apparent `previous_spline_projects…` failure, which was an artifact of
  running the binary from the repository root (`os error 3`).
* This is an M4K test, so it is **not** a pre-existing issue. It may be a
  timing-sensitive real defect or a harness race. Treat it as unresolved until
  a failure is captured with the field-level message.

### 5.3 Not verified

* **No manual graphical verification**: Simulator visuals, focus, DPI, resize,
  camera feel and readability were not looked at by a person.
* No frame-rate or performance measurement of the Simulator or of the
  instance-only upload path.
* No GLB-model train in the native smoke (the development gray-box consist
  only).
* Surface loss or device recovery during Simulator Mode.

Owner manual checklist:

1. `cargo run --release -p quantum-x`, then open a project with an associated
   saved train. Make an unsaved edit and note the selection and graph view.
2. Window / Layout › Simulator Mode (development)…. Confirm the preparing modal,
   then the Simulator composition and window title.
3. Play, Pause, Reset, Ctrl+Space and F6; try each camera, Fit Train and Fit All.
4. Return to Editor. Confirm the same file, unsaved state, Undo/Redo, selection,
   graph view, layout and editor camera.
5. With a pending draft, confirm the menu item is disabled and its hover reason.
6. Close the window from the Simulator with unsaved edits and confirm the
   Editor close prompt appears.

## 6. Limitations and deferred work

* The UI is provisional: entry placement, composition, styling, shortcuts
  (none added for entry or return) and draft-entry behavior (disabled versus
  prompt) need owner design.
* Entering retires any active editor development playback rather than
  coexisting with it. One playback `State` is shared, not yet extracted into a
  mode-independent controller.
* Every entry prepares a fresh simulation; paused sessions are not reused
  across visits.
* Speed is 1× only, with no step control in the Simulator (Single step remains
  in editor development playback).
* The cameras are the existing development ride cameras with existing offsets;
  there is no seat catalog, free-fly, or trackside camera.
* Telemetry is mass-weighted train data, not per-seat rider loads. The model is
  experimental ideal bilateral guides: no wheel contact, collision, wind or
  weather.
* Long or ill-conditioned trains may fail seating; that appears as an entry
  error.
* Deferred: audio, environment and weather, advanced cameras, train selection,
  operations UI, PBR and environment rendering.

## 7. Files

New: `apps/quantum-x/src/authoring/simulator.rs`, `simulator_ui.rs`,
`simulator_smoke.rs`, `simulator_tests.rs`, `docs/simulator-mode-m4k.md`.

Modified: `authoring/{mod,session,panels,commands,draft_workflow,train_playback,ride_camera,startup}.rs`,
`apps/quantum-x/src/main.rs` (help text), `crates/rendering/src/{workspace,scene,gpu,lifecycle,lib}.rs`,
`docs/simulator-mode-architecture-m4k.md` (status link), `docs/welcome-startup.md` (smoke route).
