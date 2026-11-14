# M4J — production velocity initialization and native integration

## Stage 0 baseline and modification map

Starting checkpoint: `4b0375065f8aeb847fee7c48f9c163c69e82a977`, clean M4I
worktree on `codex/train-articulation-m4i`. The separate managed M4J worktree
and `codex/train-articulation-m4j` branch start at that exact commit. The original
checkout's unrelated README/.vscode/tooling work is preserved. This milestone
overrides the repository's publication default: local commits only, no push/PR/merge.

Read AGENTS.md, README, core/vehicle architecture, owner UX references and M4F–M4I
reports/proposal before production edits. The M4I proposal's former approval
requirement is superseded by the explicit M4J request and initial-speed contract.

Existing path:

1. `qx-document::Editor` owns the project, history and dirty state. `Session`
   owns drafts, selection, workbench, generation revisions and transient playback.
2. A background generation worker produces `InspectionScene`; `visible_revision`,
   `revision` and `visible_preview` gate initialization. `train_track()` selects
   the generated physical reference rather than the decorative mesh.
3. `train_playback::State` captures a mechanical `Source`, sends a load epoch,
   cloned train/operations and an Arc track to the simulation worker.
4. `TrackSimulation::with_operations` calls `consist::initialize` for bounded
   seating, mass properties, ball/angular joints, centerline guides and rail twist.
   It then seeds each body's sampled tangent translation and zero angular speed.
5. `World` independently integrates bodies with production ideal captive guides,
   joints, roll constraints and torque motors. Outer dt is 1/240 s (four internal
   substeps in playback). Devices apply forces before a transactional real step.
6. The worker retains bounded timestamped pose snapshots. `ride_camera` latches
   one interpolated presentation for both GLB/gray-box instances and attached
   cameras. Free, Rider, Front, Rear, Chase and geometric Track Preview already
   exist; Track Preview is explicitly distinct from physical playback.
7. Pause/reset and source-change invalidation exist. No dedicated Simulator Mode,
   independent simulator camera bank, session preparation lifecycle, station/block
   control, wheel contact, audio playback or qualified environmental physics exists.

Proposed production changes (recorded before implementation):

- Extract shared constraint geometry in `qx-physics` without replacing stepping
  or altering default initialization. Retain independent read-only M4I diagnostics.
- Add a bounded sparse mass-weighted velocity projection with explicit lead-point
  speed equality, reached-limit inequalities, convergence diagnostics and atomic
  commit. No forces, positional correction, pose advance or physical time.
- Add a separate experimental `TrackSimulation` constructor; leave the existing
  default constructor path intact. Snapshot the successful final initial world.
- Route one explicit launch-only developer setting through the existing worker;
  reuse existing error presentation and rendering. No document/schema flag or
  permanent initializer selector.
- Qualify production mechanisms and independent bounded references, then exercise
  the actual native application worker and offscreen wgpu/egui path. Desktop
  interaction is not authorized; manual checks will be separately identified.
- Specify M4K state/lifecycles in `simulator-mode-architecture-m4k.md`; implement
  no mode switch or redesign here.

Acceptance: default compatibility; <=1e-8 final velocity-row/speed errors within
the bounded admitted scope; unchanged poses/hints/forces/time; independently
checked mass-metric solution; improved curved starts; deterministic reset and
transactional failures; native integration and honest qualification limits.

## Implemented production path

`constraint_geometry.rs` shares ball axes, line/track guide normals and the full
rail-twist row with stepping. `AngularJoint::bilateral_rows` shares fixed/hinge/
roll geometry and mounted frames. The stepping impulse laws, force integration,
motor laws, guide projections, timestep and positional stabilization remain the
existing implementation. M4I's read-only `assembly_diagnostics::velocity_residuals`
is retained independently and unchanged.

`velocity_initialization.rs` solves a sparse dual quadratic program at frozen q.
Starting from the legacy tangent seed, each equality row updates its free dual
multiplier by `-(Jv-b)/(J M^-1 J^T)`. A reached-limit row is expressed as `Jv>=0`
and clamps its accumulated multiplier to nonnegative values. Velocities always
change by `M^-1 J^T delta_lambda`: translation and angular motion are coupled,
using mass and the **full rotated world inverse inertia**, not an isotropic proxy.
This is mass-metric projection/dual coordinate descent, not a physical impulse
over an artificial timestep. No gravity, force, motor, device, Baumgarte or pose
integration routine is called. Applied force/torque fields are preserved.

All rows are assembled once from the seated production mechanism. Ball anchors,
local COM offsets, fixed/hinge/roll mounting frames, guide-point arms and rail-bank
station derivative are included. Torque motors leave their hinge freedom intact;
they are force laws, not speed rows. A reached lower/upper hinge bound allows
inward velocity and prohibits outward velocity. The numerical reached-bound
allowance is 1e-8 rad; actual positional-limit violations outside that allowance
are rejected. Seating defects above 1e-5 m/rad are errors, never repaired here.

Every eight sweeps and on the final sweep, check **all** equality residuals,
inequality feasibility and complementarity. Stop at 1e-9 (m/s point/speed rows,
rad/s angular rows). Do not confuse these mixed-unit row limits with a dimensionless
norm or the separate production/B1 quality gates. Singular/nonfinite mass response,
invalid inputs, over-budget size or unsuccessful convergence returns a group-specific
error. Failure never publishes the candidate World. There is no production rank
certificate: redundancy is admitted, but incompatible or poorly conditioned rows
can exhaust the budget without proving global infeasibility.

Before publishing, reconstruct `v_seed + M^-1 J^T lambda` independently of the
accumulated updates and require stationarity velocity-component errors <=1e-8
(m/s or rad/s). Also reject guide velocities immediately leaving an open endpoint;
this supplies an error, not an invented end-stop constraint.

Resource bounds: 64 bodies, 1024 rows (conservative preflight count), 20,000 sweeps,
and 10,000,000 row updates, whichever iteration bound is smaller. Storage is linear
in bodies/rows, with one/two body terms per row; no dense train-sized matrix.
This is deliberately narrower than the document's maximum train size. The bounded
independent SVD reference is an offline verifier, not a production solver.

### Initial speed and reset

`TrackSimulation::with_operations` and `new` retain legacy semantics. The separate
`with_experimental_velocity_initialization` constructor first uses that identical
assembly/seed, then projects velocities. It selects the highest-initial-station
guide point belonging to RunningGear/Bogie, breaking ties by authored guide order.
This is the lead in increasing track station even for negative requested speeds;
it is not switched to the rear when reversing. Its **material point velocity dot
the sampled tangent**, not COM magnitude or centreline station rate, is constrained
to the signed requested speed. The report identifies body, local point, projected
station and guide index; developer feedback also names its component.

The speed row is solved together with all mechanical rows, including limit
inequalities. No uniform scaling or implicit change of the target occurs.
Current production joints support no prescribed-relative-velocity drive:
MechanicalProfile transmission remains explicitly unsupported. The affine speed
row is supported and independently verified; no additional motor functionality
is invented to satisfy the matrix.

The successful final World, guide/attitude hints and initial telemetry speed are
stored together. Reset restores them without re-solving or warming caches.
Telemetry retains its existing mass-weighted COM tangent-speed meaning, initialized
consistently from the constructed velocities; it need not equal the lead-point
speed. Existing project `initial_speed_mps` bytes/schema are unchanged; this
different interpretation is confined to the explicit experimental constructor.

### Native developer opt-in

From this M4J worktree, in PowerShell:

```powershell
$env:QX_EXPERIMENTAL_TRAIN_VELOCITY = 'compatible-lead'
cargo run -p quantum-x --release -- edit target/m4j/native-final/fixture.qx.json
```

For legacy comparison, open a second launch after removing the setting:

```powershell
Remove-Item Env:QX_EXPERIMENTAL_TRAIN_VELOCITY -ErrorAction SilentlyContinue
cargo run -p quantum-x --release -- edit target/m4j/native-final/fixture.qx.json
```

The process reads the policy when a playback State is created. Other values are
explicit initialization errors. There is no permanent selector or persisted flag.
Use existing View → Development tools, Train workspace, Initialize playback and
Play/Pause/Single step/Reset. Experimental feedback states the lead-speed contract
and actual convergence. The usual startup command remains
`cargo run -p quantum-x --release -- edit [project.qx.json]`.

The existing worker calls the chosen constructor, then uses the same stepping,
telemetry, pose snapshot and renderer pipeline. Failed reinitialization on an
unchanged source leaves the previous simulation paused and recoverable by Reset.
Source changes still invalidate incompatible old playback; no stale old source
is made playable to hide an initialization error.

Production modification map:

| Files | Change |
| --- | --- |
| `qx-physics/src/{constraint_geometry,angular,solver}.rs` | Shared production row geometry; existing step laws retained |
| `qx-physics/src/{velocity_initialization,playback,lib}.rs` | Bounded projector/report, explicit experimental constructor, reset snapshot and export |
| `authoring/{train_playback,train_playback_ui}.rs` | Launch-only policy, worker routing/rollback, developer status |
| `authoring/startup.rs`, app `main.rs` / `authoring/mod.rs` | Startup policy diagnostic and native acceptance command wiring |
| `authoring/m4j_scenario.rs` | Real native worker/viewport/persistence acceptance |
| Existing snapshot-building scenarios/tests | Explicit absent initializer report for their legacy control packets |
| Physics tests, `m82_tests.rs`, Python verifier | Production qualification, native worker/history checks and independent references |

`qx-document`, `qx-geometry`, `qx-rendering`, Cargo manifests/lock, persisted schema,
the B1 test engine and all original numerical tests are unchanged. README links
the integration report and separate M4K proposal.

## Mechanical qualification and independent results

Production test matrix: six tracks (level, incline, horizontal R20, constant bank
0.6 rad, changing-bank R80, and the foundation source's supported curvature
transition), one/three conventional cars, speeds 0/+6/-6/+15 m/s: **48 starts**.
Each compares legacy poses, initial residuals, first-step energy/reactions and exact
reset/replay. The +6 cases also run 120 subsequent steps with exact cloned replay.
Additional production fixtures cover a single COM guide, a two-guide carriage,
five-body shared middle bogie, unequal masses, anisotropic inertias, off-axis COMs/
anchors, mounted fixed/roll joints, passive/torque hinges, both reached limits,
inward/outward rates, launch/brake startup, failed seating/convergence/size/input
validation and preservation of forces/poses/hints. Fixed-joint rotational momentum
is independently checked in rotated principal axes. No original test/gate is weakened.

Across the final 48 starts, maximum row residual is 9.956e-10. Maximum absolute
lead-speed error changes from 0.00300003 m/s (legacy) to 1.78e-15 m/s; maximum
stationarity reconstruction error is 2.31e-13 in velocity-component units.

The physical inertia model stores principal moments about COM with principal axes
parallel to component axes; arbitrary off-diagonal component-frame inertia is not
a supported document capability. Rotation into world coordinates is fully used.
No physical rail datum, contact, suspension, friction or new vehicle catalog is added.
Assembly remains bounded/local and open-track-seated; circuits still require the
existing closure check and no new initial consist-seam seating is claimed.
The native experimental constructor requires a guide on an explicitly identified
RunningGear/Bogie component. A role-less or CarBody-only train receives an error
rather than an implicit lead-reference fallback. The single-point and single
carriage controls exercise the public production World initializer directly;
the native positive cases use actual assembled train definitions.

`verify-velocity-initialization-m4j.py` constructs R20 geometry from authored
constants and raw body states. Point rows are independently reconstructed;
rail/relative twist derivatives use quaternion scalar-gap centered differences
at 2e-6/1e-6. NumPy SVD solves the whitened least-norm problem with the explicit
speed row. It consumes no production Jacobian/mass-response/impulse outputs.

| Reference | SVD rank | Maximum velocity discrepancy | Metric discrepancy J |
| --- | ---: | ---: | ---: |
| R20 carriage | 6 | 2.922e-11 | 4.383e-10 |
| Banked R20 carriage | 6 | 8.629e-11 | 4.382e-10 |
| Unequal/off-axis two-car assembly | 41 | 2.935e-9 | 8.770e-7 |
| Shared middle bogie | 30 | 1.073e-9 | 7.184e-8 |

Reference feasibility residuals are below 4e-15; stencil-solution differences below
2.55e-10. For the two-guide single carriage, the independent closed form under
the **lead point** contract is COM speed `15 sqrt(20²-.3²)/20` and
`omega_y=-15/20`, rather than M4I's COM-speed contract. Both are checked within
1e-8. No single-body orbit formula is applied to an articulated train.

Representative first-step total mechanical energy changes, 15 m/s (J):

| Fixture | Legacy | Experimental |
| --- | ---: | ---: |
| Analytic two-guide carriage, flat/banked | -5.639770 | -0.016342 |
| One conventional car, R20 | -162.326122 | -0.212809 |
| Three conventional cars, R20 | -487.953334 | -0.650420 |
| Three conventional cars, banked R20 | -512.922022 | -0.351302 |
| Three conventional cars, changing-bank R80 | -48.010010 | -0.086862 |

The complete raw test log records per-joint and per-guide average reactions and
all 48 start metrics. For the analytic carriage, initial guide force magnitudes
change from approximately 6472.8/5564.2 N to 669.84/669.85 N; these are real first-step
production reactions, not initializer pseudo-impulses converted to forces.
Residual improvement does not certify reaction accuracy for arbitrary assemblies.

Four-second/960-step actual three-car continuations also pass with finite joint/
guide/twist defects below the unchanged 0.02 production regression envelope:

| Track | Legacy energy change J | Experimental energy change J |
| --- | ---: | ---: |
| Incline | -4.876017 | -4.876017 |
| Flat R20 | -489.723589 | -2.548524 |
| Banked R20 | -513.547567 | -0.953990 |
| Changing-bank R80 | -49.751827 | -1.027965 |

These compare each policy to its own initialized energy, not equal-energy starts.
Max observed guide defect is 3.16e-5 m and twist defect 1.66e-4 rad across both
policies. The initializer does not fix ongoing discrete integration/stabilization
drift, M4I's B1 tick-563 rejection or general wheel/rail physics.

### Cost and bounded failure

Native MSVC Release, shared desktop, five projection observations per fixture;
legacy constructor (seating and seed) and 240 subsequent fixed steps timed separately. Illustrative
observations (milliseconds, not worst-case guarantees):

| Track / cars | Legacy constructor | Projection median | Sweeps | Stepping mean / max |
| --- | ---: | ---: | ---: | ---: |
| Straight / 1 | 0.037 | 0.014 | 0 | 0.619 / 2.283 |
| Straight / 3 | 0.099 | 0.036 | 0 | 2.279 / 4.486 |
| Straight / 16 (63 bodies) | 0.444 | 0.224 | 0 | 7.193 / 12.784 |
| R20 / 1 | 0.107 | 0.156 | 1064 | 0.424 / 0.500 |
| R20 / 3 | 1.681 | 0.914 | 2080 | 1.540 / 1.669 |

The 16-car R20 fixture **fails** the bounded projection after 20,000 sweeps at
1.567e-5 m/s worst row; five failures cost 51.3–53.3 ms each and retain the old
World exactly. This is an admitted resource size, not a qualified curved-train
configuration. Do not increase tolerances or iteration budgets to call it qualified.
Positive curved qualification is limited to the one/three-car and listed shared/
unequal fixtures. Even the admitted straight 16-car stepping exceeds the 4.167 ms
physics budget on this machine; no real-time, rendering FPS or broad train-size
performance guarantee follows. GPU/egui preparation is excluded from those timings.

Stage B gate: PASS for the explicitly tested positive scope; opt-in remains
disabled by default. The large curved negative remains a documented blocker to
broad adoption, not a waived positive gate.

## Native application verification

`m4j-native-scenario [existing-project.qx.json] <fresh-directory>` is a bounded
noninteractive acceptance command inside the **actual application executable**.
Without a project it explicitly creates/saves a synthetic development fixture,
then reopens that same valid file for both policies. It invokes real Session,
worker initialization, step/play/pause/reset, presentation, gray-box/GLB pipeline,
egui and offscreen wgpu. It asserts viewport poses equal physics snapshots,
preserves an unsaved metadata transaction, dirty state, selection and undo/redo,
then verifies save/reopen and unchanged input bytes. It is not a separate viewer.

Local evidence is ignored under `target/m4j/`: JSON, eight authentic native PNGs per paired run,
physics/reference/timing logs and startup results. Inspected captures show three
gray-box car bodies, independently posed running gear and drawbars on the synthetic
curve, the real editor panes, developer initializer status, and Rider POV after
reset. Gray boxes remain development assets. The existing renderer already supports
visible articulated physical motion; no rendering rewrite is needed for M4K's
initial supported train. Camera profile restoration needed an explicit settled
workspace in the capture harness; this changes no production camera workflow.

The final `native-verified` run reopens the already-saved
`native-final/fixture.qx.json` for both policies, at the normal midpoint station
60 m. It waits for the real generation worker, requires matching committed
revision/no draft preview, and never forces a visible revision. Its evidence
asserts generation readiness, unchanged input, identical initial poses, actual
subsequent motion, history/dirty/selection preservation and save/reopen. Both
Chase and Rider captures were inspected. The initializer reports 64 rows, 2664
sweeps, 9.864e-10 residual and 15.000000000000002 m/s at the lead point. The earlier
explicit synthetic-fixture run also passed at station 40 m.

GPU evidence: NVIDIA GeForce RTX 4070, Vulkan / driver 617.42, 1440×960; real native
mesh/instance uploads with unchanged-resource reuse. This is automated offscreen
rendering and image inspection, **not manual native desktop interaction**, surface
recovery testing, UX approval or an FPS benchmark. No mouse/keyboard/desktop was
controlled. First mode transition remains M4K work.

### Owner-executable desktop checklist — not manually verified

1. Run the scenario below to obtain the saved fixture, or choose an existing valid
   associated-train project. Launch the standard `cargo run ... -- edit` command
   once without the environment setting and once with `compatible-lead`.
2. Confirm project identity, track viewport, graph/timeline, authoring numeric
   controls and existing camera navigation. Make an unsaved authored edit and
   verify undo/redo before playback.
3. Enable View → Development tools, choose Train workspace and a station with room
   for the consist. Initialize. Experimental launch should name the lead gear,
   show the experimental speed contract and report a residual <=1e-9; legacy
   should retain its existing feedback and behavior.
4. Single step, Play, Pause; inspect running gear/body articulation and reaction
   feedback. Use Free, Rider, Front/Rear and Chase camera choices. Confirm physical
   playback, not the separately labeled geometric preview, is active.
5. Reset and repeat the first step; verify poses/speeds restore. Attempt a bad
   initial station, inspect the error, then reset the retained valid simulation.
6. Return to inspection, continue authoring, save a copy and reopen. Confirm project
   data, dirty behavior, selected section and undo/redo remain sensible. No project
   flag should appear for M4J. Exercise window resize/minimize/DPI separately.
7. Remove the environment setting and relaunch to return to default initialization.

## M4K recommendation and UX status

Use the existing physical worker, presentation packets, instance renderer and
attached cameras. Extract a transient simulator-session lifecycle with revision
gating, independent navigation state and retained authoritative Editor. The
separate [M4K architecture proposal](simulator-mode-architecture-m4k.md) specifies
transitions, dependencies, risks, future audio/environment hooks and a minimal
native mode implementation. No full Simulator Mode is implemented or approved.

Owner-approved scope implemented: developer opt-in through the native application,
the experimental lead-speed contract and existing playback actions. Temporary UI:
small diagnostic status inside the existing development controls only. No new
normal authoring workflow, layout, schema, mode switch or permanent selector.
Mode-switch placement, simulator composition, draft-entry behavior and expanded
camera/seat controls still require human design.

## Reproduction

Native Windows/MSVC; Rust/Cargo 1.99.1. NumPy is an offline verification dependency,
not a new application dependency. Commands below run from this worktree. Evidence
directories must be fresh. Cargo artifacts may reuse an existing local cache with
`CARGO_TARGET_DIR`; logs/PNGs remain under this worktree's ignored `target/m4j/`.

```powershell
New-Item -ItemType Directory -Force target/m4j | Out-Null
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features qx-physics/experimental-source-guidance -- -D warnings
cargo test --workspace -- --test-threads=1
cargo test --workspace --features qx-physics/experimental-source-guidance -- --test-threads=1
cargo test --workspace --release -- --test-threads=1
cargo test --release -p qx-geometry -p qx-physics --features qx-physics/experimental-source-guidance
cargo test --release -p qx-physics --test velocity_initialization -- --include-ignored --nocapture --test-threads=1 > target/m4j/qualification.log 2>&1
python tools/verify-velocity-initialization-m4j.py target/m4j/qualification.log
cargo test --release -p qx-physics --test default_velocity_compatibility -- --nocapture
./tools/run-constraint-validation-m4i.ps1
cargo build --release -p quantum-x
cargo run -p quantum-x --release -- m4j-native-scenario target/m4j/native-final
cargo run -p quantum-x --release -- m4j-native-scenario target/m4j/native-final/fixture.qx.json target/m4j/native-verified
cargo run -p quantum-x --release -- --startup-check edit target/m4j/native-final/fixture.qx.json
git diff --check
git diff --cached --check
```

An initial unconstrained parallel debug build exhausted Windows paging/commit
space (`os error 1455` / compiler allocation failures). The serial build-job
setting above avoids that environmental failure without changing source,
numerical thresholds or test coverage. It is a command-local verification choice,
not a repository configuration change. Fetch used Windows Schannel trust after
the bundled Git OpenSSL CA setup could not verify the remote certificate; TLS
verification was not disabled.

For the independent legacy control, a temporary ignored standalone Cargo package
compiled the same `default_velocity_compatibility.rs` against untouched M4I
dependencies at `4b03750`. M4I and M4J both emitted `d3188e7a2680c866` for six
straight/curved conventional/below-track/torque-seat worlds, their first 12 steps,
diagnostics, telemetry and resets. This is a same-toolchain regression observation,
not a portable hash guarantee across compilers. The verifier accepts optional
baseline/current log paths to compare these controls automatically.

## Executed checks and checkpoint

All successful results below are native Windows/MSVC. The initial parallel-build
memory failure is described above and retained in its separate log.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace` | PASS |
| Strict workspace/all-target Clippy, default and source-guidance feature | PASS, both configurations |
| Serial default debug workspace | 512 passed, 0 failed, 15 ignored (before the final endpoint test addition) |
| Serial feature-enabled debug workspace | 543 passed, 0 failed, 29 ignored |
| Final default Release workspace | 513 passed, 0 failed, 15 ignored |
| Release geometry/physics with source-guidance feature | 270 passed, 0 failed, 28 ignored |
| Explicit M4J Release suite, including both opt-in runs | 10 passed, 0 failed |
| Independent M4J SVD references and baseline fingerprint comparison | PASS, four references and identical legacy digest |
| Full M4I Release suite, including opt-in runs | 8 passed, 0 failed; fresh `target/m4i/validation.log` |
| Unchanged M4I independent verifier | PASS: 24 analytical runs, 75 decompositions, 13 local systems; corrupt-evidence negatives retained |
| Final default application M4J worker/configuration tests | 2 passed, 0 failed |
| Native Release build and offscreen existing-project scenario | PASS; current generation, physical poses, Chase/Rider, play/pause/reset, persistence/history |
| Real application startup checks, default/experimental launch environment | PASS; Editor opens the same document with policy false/true respectively |
| Default application feature-tree audit | PASS; research graph/source-guidance features absent |
| `git diff --check`, `git diff --cached --check` | PASS |

Other ignored long/profiling fixtures are not claimed as executed. Manual desktop,
focus/input feel, DPI/resize/surface-loss and display FPS checks remain **not
manually verified**. No full Simulator Mode, wheel contact, suspension, new catalog,
schema migration, browser viewer, release or remote publication is introduced.

Local branch: `codex/train-articulation-m4j`, based exactly on `4b03750`.
Checkpoints: Stage 0 audit `5455339`; M4K architecture `967dd68`; production
implementation/qualification `3ff6c78`; final native-generation readiness and this
verification record are in the subsequent final checkpoint. The M4I worktree and
original checkout's unrelated tooling edits are preserved. Fetch succeeded;
no push, PR, merge or release was performed. The resulting M4J worktree is left clean.
