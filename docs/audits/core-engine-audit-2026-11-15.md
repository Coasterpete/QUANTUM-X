# QUANTUM DYNAMIX — Comprehensive Core Engine Audit

**Roller Coaster Engine · QUANTUM-X Rust workspace · 2026-11-15**

| Audit boundary | Recorded state |
| --- | --- |
| Checkout | `C:\Dev\QUANTUM-X` |
| Branch | `codex/r0-2-automated-packaging`, tracking its existing remote branch |
| Commit | `aa3dfd0` — `ci: automate verified portable Windows development packages (R0.2)` |
| Starting working tree | Clean |
| Platform | Native Windows, `x86_64-pc-windows-msvc`; Rust/Cargo 1.100.0 |
| Scope | All seven workspace packages, their production module trees, integration tests, diagnostic tooling and relevant engineering reports; emphasis on engine behavior and interfaces |
| Comparison baseline | [Production-readiness roadmap](../roadmaps/production-readiness-v1.md), audited at `25ae183`, plus [R0.1/R0.2 documentation](../ci.md) in this checkout |
| Changes authorized | This Markdown audit and ignored local diagnostic artifacts only. No production edits, refactoring, feature implementation or milestone execution |

The title follows the requested audit framing. The repository's current public identity is **QUANTUM Dynamics**, with **QUANTUM-X** as its internal name. No identity files or product branding were changed. The wider amusement-ride and park-simulation ambition is treated as a future architecture requirement, not approval to replace graph/timeline coaster authoring or introduce park-management gameplay.

## 1. Executive assessment

**The workspace is a substantial coaster-authoring engine with experimental articulated rigid-body simulation. It is not yet a general amusement-ride or park-simulation engine.** Its most mature systems are mathematical track authoring, versioned documents, transactional commands and procedural track construction. The important boundary between authored reference-speed geometry and simulated vehicle motion is real and preserved in code.

The physics is more capable than a spline animation: it integrates independent translations and rotations, applies forces, and solves ball/angular joints and captive rail constraints. Nevertheless, rail guidance is idealized, collision handling is absent, and longer-train, full-ride and cross-machine qualification remain open. Research implementations of rail-normal/source-derived mechanics should not be confused with the production `TrackSimulation` path.

The architecture has useful existing seams. Core geometry, documents, styles and physics do not import egui or wgpu. The next useful architectural work is to extract the existing runtime coordination and establish explicit geometry/station contracts, rather than replace the workspace with a speculative engine framework.

The most actionable findings are:

1. **Confirmed correctness defect:** operations can validate and serialize in the authored station domain but be invalid on the derived physical track used for simulation. A scratch reproduction produced a 40 m rider-authored arc, a 36 m physical track, and a brake at 38–40 m accepted by document generation but rejected by physical-track validation (§4.5).
2. **Production simulation lacks an operating ride controller.** Launch/brake regions exist; stations, lifts, block reservations, dispatch and multiple runtime trains do not. This is already R5, not a new milestone proposal (§6).
3. **The per-tick cost depends unnecessarily on the whole sampled track.** Every production step rescans track samples, validates/copies state repeatedly, and performs substantial per-row work. The source establishes these costs; this audit does not claim measured speedups (§5.4).
4. **Engine lifecycle remains editor-owned.** Runtime preparation, source comparison, clocks, worker requests and presentation are under `authoring/`, and Simulator preparation obtains its physical track through a rendering-owned `InspectionScene`. R3 already addresses the controller boundary (§3).
5. **The long-term ride-platform barriers are concrete:** one coaster aggregate, train-shaped persistence, no static/kinematic bodies or world anchors, no collision world, no scene/entity hierarchy, no shared multi-ride scheduler, and no audio engine (§7, §10).

The earlier roadmap remains a sound release-readiness framework. This audit adds engine-specific detail and one reproduced cross-system defect; it does not restart R0–R10. In particular, **“no CI” and “no automated package” are obsolete findings for this branch**. R0.1 and R0.2 are implemented in source. Their hosted execution, required-check settings and merge history were not queried in this audit.

## 2. Status definitions and capability summary

“Implemented” means a working code path exists within the stated scope; it does not mean independently certified or production-ready. “Partial” means a useful subset exists with material missing behavior. “Experimental” means real research/development code or an explicitly unqualified model. “Planned” means documented intent or representable data without a working feature. “Absent” means no corresponding engine implementation was found in this checkpoint.

| Subsystem | Implemented | Partial / experimental | Planned / absent |
| --- | --- | --- | --- |
| Architecture | Seven packages; CPU domain APIs; transactional document authority | Editor-owned simulation coordinator; rendering-owned scene handoff | General ride/world runtime and multi-ride coordination absent |
| Geometry | f64 graph sections, transitions, distinct positional curves, arc-length traversal, frames, construction | Constrained rider-to-track inverse; sampled production query contract; source/dense research | General inverse heartline solve and collision/clearance model absent |
| Physics | Independent rigid-body integration and force/joint machinery | Captive centerline train playback; bounded assembly; opt-in velocity projection | Wheel contact, suspension and collision response absent |
| Operations | Persisted launch/brake commands, force limits, resistance, telemetry | Circuit seam handling; Shuttle is layout intent only | Stations, lifts, blocks, dispatch, transfers and multi-train controller absent |
| Scene/environment | Editable supports; reusable mesh instances/materials | Diagnostic imported track/support geometry | Persisted terrain/scenery/world entities, weather and park runtime absent |
| Rendering | Native wgpu viewport, PBR, instancing, material/GLB loading, relative origin | One-window inspector/editor model; coarse scene replacement | Spatial streaming/LOD/occlusion infrastructure and device reconstruction absent |
| Audio | No audio functionality | `qx-audio` contains only template arithmetic/test code | Entire audio system deferred in roadmap |
| Persistence | Schema 7; 1–6 migration paths; bounded/strict JSON; atomic project replacement | External assets and project are not one atomic transaction; full snapshot history | Autosave/recovery, migration backups and general ride/scene schema absent |
| Verification | CPU CI definitions, numerical/transactional tests, independent verifier tools | Feature-gated and ignored research/GPU suites; historical profiling | Full-ride, multi-machine and park-scale gates not established |

## 3. Architecture, dependencies and API boundaries

### 3.1 Actual package graph

Arrows below mean “depends on.” This is the local manifest graph, not a proposed reorganization.

```text
quantum-x ──► qx-rendering ──► qx-geometry ──► qx-style
     │              └─────────────────────────► qx-style
     ├──────► qx-physics ──► qx-document ──────► qx-geometry
     │              └────────────────────────► qx-geometry
     ├──────► qx-document ────────────────────► qx-style
     ├──────► qx-geometry
     └──────► qx-style

qx-audio     (standalone; unused by the application)
```

Evidence: [root workspace manifest](../../Cargo.toml), package manifests in [apps/quantum-x](../../apps/quantum-x/Cargo.toml), [qx-geometry](../../crates/qx-geometry/Cargo.toml), [qx-physics](../../crates/qx-physics/Cargo.toml), [qx-document](../../crates/qx-document/Cargo.toml), [qx-style](../../crates/qx-style/Cargo.toml), [rendering](../../crates/rendering/Cargo.toml) and [qx-audio](../../crates/qx-audio/Cargo.toml).

**What is healthy:** no workspace dependency cycle; no editor/GPU dependency in the CPU domain crates; deliberate f64-to-f32 adapters; small external dependency surface in physics; serialization stays out of `World`; rendered assets do not supply mechanical dimensions. The application is split into many modules rather than a single `main.rs` implementation.

**What is coupled:**

- `qx-geometry` contains pure numerical geometry **and** construction meshes, support solids and style/material adapters. Its dependency on `qx-style` comes from this construction layer. This is an understandable current tradeoff, but generic mathematical consumers inherit coaster construction/catalog dependencies.
- `qx-physics` consumes persisted `qx_document::train::{Train, mechanism::JointModel}` directly. `AngularJoint` stores the document's `JointModel`, while `TrackSimulation` stores document `Operations`. The domain types are UI-independent, so this is not an egui leak. It does make persistence vocabulary the runtime physics vocabulary and complicates non-train mechanisms.
- `qx-rendering` includes winit lifecycle, egui composition and coaster-specific scene/picking structures in the same package as GPU resources. `EditorSurface` exposes `SectionId`, support IDs and egui rectangles; it is an editor adapter, not a general scene-rendering API. See [workspace.rs](../../crates/rendering/src/workspace.rs), `EditorSurface` at line 47, and [scene.rs](../../crates/rendering/src/scene.rs), `InspectionScene` at line 9.

### 3.2 Missing headless application-runtime boundary

The numerical solver can run without egui. The application-level simulation session cannot yet be reused independently as a cohesive service:

- [train_playback.rs](../../apps/quantum-x/src/authoring/train_playback.rs): `Source`, `Request`, `Worker`, `State`, `initialize`, `sync`, `presentation_sample` own source invalidation, scheduling and snapshots.
- [playback_clock.rs](../../apps/quantum-x/src/authoring/playback_clock.rs): `Waiter` owns platform pacing, including a scoped Windows high-resolution timer and a portable fallback.
- [simulator.rs](../../apps/quantum-x/src/authoring/simulator.rs): `request_entry`, `poll`, `exit` operate on `Session`; input and camera-bank behavior sit beside lifecycle state.
- [session.rs](../../apps/quantum-x/src/authoring/session.rs): `scene_cached`, `Session` and the generation worker combine document inputs, physical-reference derivation, construction, materials and rendering DTOs.

`simulator::request_entry` asks `InspectionScene::train_track()` for the physical path. Physics itself does not depend on rendering, but the app's preparation flow does. A headless ride server, batch simulation, separate native Simulator window or second ride would need to reproduce or extract this coordination.

The existing epoch tags, immutable track `Arc`, bounded presentation history, source invalidation and failed-load retention are useful assets to preserve. Extracting them into the R3 controller is preferable to creating another worker or clock. A new crate is not inherently necessary; module boundaries can establish the contract first.

### 3.3 Document and revision granularity

[qx-document/src/lib.rs](../../crates/qx-document/src/lib.rs), `Project` at line 83, owns one `initial`, `sections`, `heartline`, `construction`, `style`, `operations` and support collection. `trains` are alternative definitions, not multiple active train instances or multiple rides. Simulation selection comes from `HeartlineSettings::train_configuration_id`, coupling nominal rider reference and active train selection.

`train_playback::Source::new` appropriately strips visual asset assignments before comparing mechanics, but `sync` reconstructs/clones the source for comparison during updates. Separately, Simulator `State::revision` follows the editor's generation revision. These are whole-source/editor revision contracts, not explicit geometry/mechanics/visual revisions. R3 should give a prepared simulation its own immutable source identity, with owner-directed reload behavior.

The [existing multiple-coaster proposal](../multiple-coaster-roadmap.md) already recommends a stable `CoasterId`, scoped commands, shared environment data and coaster-specific caches. Do not duplicate that work or implement its unapproved UX as part of this audit.

## 4. Geometry and mathematics

### 4.1 Implemented authoring and numerical foundations

| Capability | Evidence and assessment |
| --- | --- |
| Ordered graph sections | [authoring.rs](../../crates/qx-geometry/src/authoring.rs): `SectionId`, `Domain`, `SectionKind`, `BoundaryPolicy`, `InitialConditions`, `validate_track`. Force-vector, world-geometric, rider-local, straight, arc and native-curve sections are real representations. |
| Scalar transitions | [transition.rs](../../crates/qx-geometry/src/transition.rs): `Function`, `Transition`, `Channel`, one-sided evaluation and continuity diagnostics; polynomial, cosine, Hermite and scalar rational profiles. |
| NURBS region editing | [profile.rs](../../crates/qx-geometry/src/profile.rs): `NurbsProfile`, degree/control/knot/weight validation; [profile_transition.rs](../../crates/qx-geometry/src/profile_transition.rs) and [region_edit.rs](../../crates/qx-document/src/region_edit.rs): exact splits/conversions and explicit continuity edits. |
| Positional curves | [curve.rs](../../crates/qx-geometry/src/curve.rs): `Curve::{Bezier, BSpline, Nurbs, CatmullRom}`, `PreparedCurve`, `CurveEvaluation`, `CurveJoin`. B-spline/NURBS use homogeneous de Boor evaluation and derivative nets. They are not silently converted into Bézier authoring. |
| Representation-aware operations | [curve_edit.rs](../../crates/qx-geometry/src/curve_edit.rs): stable control IDs, moving/inserting/removing controls, weights/knots/degree, knot insertion, explicit alignment and Bézier join fitting. `SetDegree` is explicitly reinterpretation, not exact degree elevation. |
| Arc length | [arc_length.rs](../../crates/qx-geometry/src/arc_length.rs): adaptive Simpson quadrature per smooth span, compensated accumulation, bracketed Newton/bisection inversion, convergence/resource errors. |
| Shared downstream output | [solver.rs](../../crates/qx-geometry/src/solver.rs): `GeneratedTrack`, `CenterlineSample`, `SectionRange`, `SmoothRange`, connection/measurement diagnostics; [spline_track.rs](../../crates/qx-geometry/src/spline_track.rs) adapts native curves to this output. |
| Frames and precision | [math.rs](../../crates/qx-geometry/src/math.rs): project-owned f64 vectors/quaternions/frames. Graph generation integrates pose with spatial RK4; spline orientation approximates Bishop transport plus authored roll. |
| Heartlining | [heartline.rs](../../crates/qx-geometry/src/heartline.rs): `OffsetProfile`, `offset_path`, `track_from_rider`, `point_motion`, `generate_reference`; offset derivatives, rotational effects and singularity/compatibility rejection. |
| Construction | [construction.rs](../../crates/qx-geometry/src/construction.rs), `ConstructionAssets`, `build_with_assets`, `MeshLimits`; continuous sweeps and shared repeated hardware; [source_profile.rs](../../crates/qx-geometry/src/source_profile.rs), conservative exact section extraction; [construction/segmented.rs](../../crates/qx-geometry/src/construction/segmented.rs), bounded segmented construction. |

The conventions are documented: right-handed coordinates, +Y up, local tangent/normal/lateral axes +X/+Y/+Z, metres/seconds/radians. Unit names and field suffixes are generally good, though most quantities remain raw `f64` rather than compiler-distinct units. There is no need to retrofit a complete units framework before fixing cross-domain station use.

### 4.2 Accuracy contracts and limits

- Graph generation's default `max_step_m = 0.1`, rotation guard and sample budget are **resolution controls**, not a certified global error bound. `Context::step` rejects excessive per-step rotation; it does not automatically refine until a position-error target is met. Step-halving/analytic tests are therefore important evidence.
- Arc length has configurable error estimates, bounded recursion and inversion failure. Its own module correctly calls Richardson estimates diagnostic estimates rather than certified bounds. Repeated partial quadrature inside inversion can be expensive on difficult curves.
- `Channel::evaluate_side` linearly searches transitions. Generation uses this validated internal path, avoiding full validation on each internal evaluation, but still pays search/allocation work for dense scalar profiles. Public `Channel::evaluate` validates the whole channel; device curves call it during force evaluation. This deserves profiling before large operations graphs are supported.
- `NurbsProfile::control` recursively builds derivative controls during evaluation; positional `PreparedCurve` instead prepares derivative nets. A prepared scalar-channel evaluator is a possible optimization, not an audit implementation recommendation without measurements.
- World-geometric pitch/yaw deliberately reject near-vertical singularities and point to rider-local controls. This is a supported-domain limit with an actionable error, not missing handling.
- `track_from_rider` only accepts inverse geometry compatible with a supplied orientation field. A general constrained solve that derives banking/orientation from arbitrary rider geometry is absent. Variable offsets may fail for valid mathematical reasons; they must not be silently reshaped.

### 4.3 Production sampled queries versus source-derived research

`GeneratedTrack::sample_at_distance` ([solver.rs](../../crates/qx-geometry/src/solver.rs), line 156) binary-searches sampled stations, evaluates cubic Hermite position from endpoint tangents and independently slerps orientation. This is a useful shared practical query, but it does not guarantee that the derivative of the interpolated position is everywhere exactly the returned frame tangent with unit station speed. Higher derivatives can have numerical-cell seams even when authored source data is smooth.

Production guidance uses this query plus local tangent projection. `track_frame_derivative` in [physics solver.rs](../../crates/qx-physics/src/solver.rs) uses a bounded finite difference of these frames; it does not evaluate analytic derivatives of the authored source. This matters for contact normals, constraint Jacobians, J-dot and high-fidelity reaction/energy calculations.

There is already extensive work on this problem:

- [solver/prepared.rs](../../crates/qx-geometry/src/solver/prepared.rs): feature-gated source queries, explicit document/generation identity and derivative provenance. Its RK4 restart jets explicitly distinguish source-equation derivatives from derivatives of the numerical extension.
- [solver/prepared/dense.rs](../../crates/qx-geometry/src/solver/prepared/dense.rs): `ContinuousGraph`, a C3 quaternion-hodograph numerical extension, physical station reparameterization and consistent differentiated geometry for its admitted source subset.
- [M4E-A](../train-articulation-m4e-a-source-rail-adapter.md), [M4E-C](../train-articulation-m4e-c-source-b1-adapter.md), [M4F](../train-articulation-m4f.md), [M4H](../train-articulation-m4h-energy-and-performance.md): qualification, failed contracts, later continuous-source experiments and energy diagnostics.

**Status: experimental infrastructure, not production integration.** `experimental-graph-source` is off by default. `experimental-source-guidance` exposes integration-test research; its mechanical stepping implementations live under `qx-physics/tests/support/`. Enabling the feature does not replace `TrackSimulation` guidance. Further work should extend this existing research and qualification record, not start another competing rail evaluator.

### 4.4 Topology and engineering measurements

[topology.rs](../../crates/qx-geometry/src/topology.rs) separates authored `Open/Shuttle/Circuit` intent from measured closure. Closure checks endpoint position/tangent/frame against 1 mm/0.001 rad tolerances and checks join poses. `complete` performs a bounded, explicit connector search without changing prior authored sections. It is a G1/pose closure helper, **not G2 force continuity, obstacle clearance or an operations controller**.

[engineering.rs](../../crates/qx-geometry/src/engineering.rs) uses generated measurements and design speed. [heartline.rs](../../crates/qx-geometry/src/heartline.rs) retains one-sided smooth-range derivatives. These are useful geometry diagnostics, separate from runtime train loads. A geometrically valid circuit can still have curvature/twist-rate jumps and mechanically difficult motion; closure should not be promoted into a broader simulation-quality certificate.

### 4.5 Confirmed defect: operations use inconsistent authored and physical stations

**Finding E01 · High · new cross-system reproduction · recommended within R4, before R5.**

Data flow:

1. `Project::generate` validates operations against the track generated directly from authored sections ([lib.rs](../../crates/qx-document/src/lib.rs), line 157).
2. `operations_ui::show` gets `expansion::State::committed_track`, which also generates from `initial`/`sections` without deriving the rider-to-physical inverse ([operations_ui.rs](../../apps/quantum-x/src/authoring/operations_ui.rs), line 22; [expansion.rs](../../apps/quantum-x/src/authoring/expansion.rs), line 79).
3. For rider-reference authoring, `scene_cached` separately derives the physical track through `supports::physical_attachment_track` ([session.rs](../../apps/quantum-x/src/authoring/session.rs); [supports.rs](../../crates/qx-geometry/src/supports.rs), line 9).
4. Simulator preparation selects that physical path via `InspectionScene::train_track`. `TrackSimulation::with_operations` validates the same device stations against its different length ([playback.rs](../../crates/qx-physics/src/playback.rs), line 36).

The local CPU-only probe used one radius-20 m arc with authored extent 40 m, `RiderReference`, constant normal rider offset −2 m, and a brake at 38–40 m. Results:

```text
authored_length_m=40.000000000
physical_length_m=36.000000000
authored_operations_validation=Ok(())
physical_operations_validation=Err(DocumentError("device stations exceed current track; revise its start/end positions"))
project_save_serialization_succeeds=true
```

This demonstrates an actual acceptance/simulation mismatch without relying on a visual interpretation. The probe also establishes that the constrained inverse succeeds for this fixture. It does **not** assert that all rider-reference projects fail, or that a train was run in this probe.

**Recommendation:** choose and document one operations station domain, derive/validate it consistently in authoring, document preflight and runtime, and add cross-crate cases for longer/shorter physical references, device curves, circuit seams and round trips. Explicitly preserve authored device intent when reference settings change. Do not silently rescale stations as an incidental fix. The broader API should distinguish source station from physical arc station, rather than rely on identically named `f64` arguments.

Local reproduction source and output are retained under ignored `target/core-engine-audit-20261115-175304/probe/` and `station-probe.log`. They are audit diagnostics, not repository feature code.

## 5. Physics and trains

### 5.1 What actually runs

[body.rs](../../crates/qx-physics/src/body.rs) models positive finite mass, physically realizable principal moments, a world pose, linear/angular velocities, force and torque. `integrate_forces` includes gravity, force/mass and the rotational gyroscopic term; `advance` uses velocity-based position advancement and a normalized quaternion rotation update.

[solver.rs](../../crates/qx-physics/src/solver.rs), `World`, owns ball joints, angular joints and ideal guides. The outer timestep is exactly `FIXED_STEP_SECONDS = 1.0 / 240.0`. `TrackSimulation::with_operations` selects 4 substeps and 32 sequential-impulse iterations. The general `World` API accepts bounded alternative substep/iteration counts while retaining the fixed outer step.

[consist.rs](../../crates/qx-physics/src/consist.rs), `initialize`, constructs independent component bodies, COM-relative anchors, explicit guides and angular constraints from a persisted train. `assembly.rs` performs bounded initial seating; it is initial-condition construction, not a prescribed moving train. `TrackSimulation::step` then applies forces and calls the dynamic solver. Guide stations are projection hints, not animated vehicle coordinates.

Ball joints, fixed joints, hinge limits, torque motors and roll locks are real. `AngularJoint` shares angular-row geometry with velocity initialization, while independent assembly diagnostics remain available. `MechanicalProfile` transmission is representable in documents but explicitly rejected by runtime validation ([angular.rs](../../crates/qx-physics/src/angular.rs), line 27). Do not label that enum variant as implemented 4D mechanics.

### 5.2 Model fidelity and missing physical systems

| Item | Status / qualification |
| --- | --- |
| Multi-car/shared-bogie mechanisms | Experimental but substantial; arbitrary component IDs and physical connection graphs, not a fixed “one body plus two bogies” renderer assumption |
| Captive centerline guidance | Experimental production path; bilateral point constraints and one rail-referenced twist constraint per relevant running-gear body |
| Actual wheel/rail contact | Absent: no contact patches, normal complementarity, wheel spin, tread/rail surfaces, friction or contact loss/impact |
| Wheel alignment | Read-only research: [wheel_geometry.rs](../../crates/qx-physics/src/wheel_geometry.rs), `WheelGeometry`, `WheelTarget`, `diagnose`; [rail_geometry.rs](../../crates/qx-geometry/src/rail_geometry.rs), offset loci/nearest queries |
| Bogie alignment fitting | Narrow opt-in geometric research: [bogie_fitting.rs](../../crates/qx-physics/src/bogie_fitting.rs), `FitSeed::fit`; one supported leaf/free limited hinge, not a general suspension/contact solver |
| Collision detection/response | Absent: no broad phase, shape registry, narrow phase, contact manifold, restitution/friction solver or continuous collision detection |
| Suspension/compliance | No general spring/damper or compliant wheel model; a torque motor's stiffness/damping is not a suspension system |
| Couplers | Ball/angular connections and independent drawbar fixtures exist; production coupler slack, buffers and impact behavior remain unqualified |
| Aerodynamics/resistance | Ideal scalar rolling coefficient and CdA drag with configured density; no wind-relative airflow, lateral wind, temperature or weather coupling |
| Rider telemetry | Train mass-weighted specific force exists; no production per-seat accelerometer/mount catalog |

The Baumgarte stabilization in `scalar_impulse` corrects 20% of positional error per substep. Its comment explicitly states that it introduces artificial energy and is not a contact model. Bilateral constraints can exert pulling reactions. These are important declared assumptions, not evidence that the rigid-body solver is fake.

### 5.3 Initialization and determinism

The default constructor seeds each body's sampled tangent translation with zero initial angular speed. On curved/articulated configurations this can violate the assembled mechanism's velocity constraints, producing a correction transient. [velocity_initialization.rs](../../crates/qx-physics/src/velocity_initialization.rs) contains the M4J alternative: bounded sparse mass-metric projection with equality/inequality rows and an explicit lead-guide speed constraint. It is opt-in via `QX_EXPERIMENTAL_TRAIN_VELOCITY=compatible-lead`, not the default semantics.

The M4J initializer is limited to 64 bodies/1024 rows/20,000 sweeps/10,000,000 row updates, with 1e-9 row residual and 1e-8 stationarity tolerances. Document limits are substantially larger. A valid saved train is not necessarily within a qualified real-time or initialization envelope.

Fixed-step ordering, transactional rollback, exact reset snapshots and same-input replay tests are good foundations. The Windows worker schedules against monotonic deadlines, catches up with at most eight steps per wake and records discarded wall-clock debt without enlarging/skipping numerical steps. Presentation interpolation is separate from stepping.

Remaining determinism work: tick-stamped external events, deterministic multi-ride ordering, stored golden states and cross-machine/compiler/target comparisons. Existing in-process equality tests and historical same-machine native replay do not establish cross-machine bit identity.

### 5.4 Performance risks visible in the production path

**Finding E02 · High · source-confirmed work; performance impact not remeasured · R4.**

- `TrackSimulation::step` clones `world`, guides and attitudes for rollback. `World::step_on_track_topology` then clones those candidates again before solving. Preserve atomic failure behavior, but measure the duplicate copying before replacing it with a transaction/scratch-buffer strategy.
- Every `step_on_track_topology` rescans all track samples for finite positions and ordered stations. At 240 Hz this is O(track sample count) work per train, independent of local guide count. The immutable track is already shared by `Arc`; validation could eventually be associated with that exact immutable generation.
- `step_inner` allocates diagnostic/impulse vectors, solves all 4 × 32 iterations, samples guide targets inside the iteration loop, and repeatedly validates body/joint data. There is no convergence exit or production warm-start cache. Any change needs unchanged regression tolerances, deterministic behavior and energy comparison.
- `consist::snapshot` repeatedly searches component IDs for connections; projected station/snapshot updates perform repeated scans of guides. These are bounded but increasingly expensive with longer mechanisms.
- Each device visits bodies, and acceleration curves use public `Channel::evaluate`, which validates the whole channel. Cost grows with devices × bodies × profile complexity; simple launch tests do not characterize the upper limit of 1024 devices.

Historical **production-path** observations in [M4J §Cost and bounded failure](../train-articulation-m4j-native-integration.md): straight 16-car/63-body stepping mean 7.193 ms, maximum 12.784 ms; the 16-car R20 velocity projection failed after 20,000 sweeps. These exceed or fail the relevant 4.167 ms/initialization targets on that recorded setup. They were not rerun here and are not measurements of this audit's debug test execution.

The M4H energy drift numbers quoted by the roadmap come from a separate source-guidance research harness. They are evidence of numerical risk and existing diagnostic work, **not a freshly measured defect value for the current production captive-centerline solver**. R4 needs an energy/work ledger around the actual shipped path and its declared train envelope.

## 6. Ride operations

[qx-document/src/operations.rs](../../crates/qx-document/src/operations.rs), `DeviceKind` at line 50, has exactly `Launch` and `Brake`. `TrackDevice` persists stable ID, interval, direction, acceleration profile and force cap. Profiles support constants, legacy keys and Transition Editor curves. `validate_acceleration_curve` uses analytic/control-hull/subdivision bounds rather than assuming sampled values prove bounded acceleration.

[qx-physics/src/devices.rs](../../crates/qx-physics/src/devices.rs), `forces`, applies ideal longitudinal body-COM forces in occupied half-open intervals. Brakes oppose current motion, passive forces are capped to avoid numerically driving through rest, and total device force limits are observable in telemetry. Devices do not assign velocity. [tests/devices.rs](../../crates/qx-physics/tests/devices.rs) checks actual multicar acceleration, caps, reverse braking, profiles and seam traversal.

**Partial semantics:** occupancy is by component/body station, not an axle envelope, train nose/tail occupancy interval or block reservation. Force commands are sampled once per outer 1/240 s step and held during internal substeps. A brake at rest is not an automatic station hold or anti-rollback mechanism. Shuttle classification allows an open path; it does not reverse a train or create end stops. Open-end escape fails transactionally.

**Absent operations:** stations and berthing targets; boarding/restraints/gates; dispatch sequencing; chain/cable lifts and anti-rollback; tire transport; target-speed launch control; block occupancy/reservation/interlocks; transfer/switch track routing; emergency-stop/fault/reset states; multiple live trains and an operator event log. Overlapping launch/brake regions sum forces; there is no higher-level arbitration policy.

**Finding E03 · High · known roadmap gap · R5.** Preserve the distinction between physical actuators and an operations controller. The roadmap's deterministic controller, tick-stamped operator events, single-train complete cycle and gated multi-train block simulation remain the right next operating slice. These are coordinated engine requirements, not justification to add disconnected device enum variants without runtime control, persistence and tests.

## 7. Scene, environment and park-scale readiness

### 7.1 Existing reusable pieces

- [supports.rs](../../crates/qx-geometry/src/supports.rs): editable `Structure`/`Node`/`Member`, track-relative attachments, fixed-elevation foundations, timber bents/lattice generation, validation and renderable solids. This is useful construction, not structural mechanics or terrain-aware footings.
- [qx-style/src/materials.rs](../../crates/qx-style/src/materials.rs) and [rendering/material_library.rs](../../crates/rendering/src/material_library.rs): data-driven material definitions, bounded loading, texture sharing and recoverable missing-map diagnostics.
- [asset.rs](../../crates/rendering/src/asset.rs): static GLB hierarchies and `AssetInstance` transforms can render reusable objects. The application currently uses this mainly for train/construction assets; this is not a persisted general scenery placement workflow.
- [diagnostic_import](../../crates/qx-geometry/src/diagnostic_import/mod.rs): CSV track frames and support XML with explicit size/count/numeric bounds. Imported footer `height_above_terrain_m` is metadata, not a terrain surface. The diagnostic application's own text says there are no Regions, terrain or vehicle simulation ([surface.rs](../../apps/quantum-x/src/diagnostic_import/surface.rs)).

### 7.2 Absent engine systems

No persisted terrain heightfield/mesh, terrain query service, water system, scenery/entity hierarchy, general object transform/component registry, shared collision world, spatial scene index, streaming cells, world lighting state, weather field or environmental scheduler was found. `rendering/environment.rs` generates procedural sky radiance/IBL textures for material lighting; it is **not** weather, atmosphere dynamics or a visible world/sky system.

`Project` has no ride collection or shared world aggregate. `InspectionScene` contains one generated track, its construction/reference and labels; it has no generic ride/scene identity model. Park-scale shared transforms, references, persistence and simultaneous simulation are not merely unfinished UI.

**Finding E04 · Strategic blocker · existing post-1.0 scope plus future design.** Reuse the multiple-coaster proposal when broader scope is approved. Establish stable ride/object identities, a shared scene coordinate contract and scoped snapshot/cache identities before adding park collections. A park runtime should coordinate fixed-step ride instances and slower environmental/operational updates explicitly, with deterministic event ordering. This does not require advancing scenery or weather at 240 Hz, nor adding an ECS before a concrete use case.

Pedestrian, queue, pathfinding or economic gameplay requirements are not defined by this repository. They require owner scope/design rather than being silently inferred from “park simulation.”

## 8. Rendering and audio

### 8.1 Implemented rendering infrastructure

The wgpu adapter/resource boundary is real: `Gpu` owns instance, surface, device, queue, pipelines, depth and egui renderer. Resize/minimize paths avoid zero-sized configuration. Timeout/occlusion defer drawing; Outdated reconfigures; Lost recreates the surface. `wait_idle` has a bounded wait. These are substantive lifecycle paths in [gpu.rs](../../crates/rendering/src/gpu.rs), not a viewport mockup.

[mesh_gpu.rs](../../crates/rendering/src/mesh_gpu.rs) reuses buffers and skips reupload when mesh/material `Arc` identity and origin match. Repeated hardware is instanced. [asset_gpu.rs](../../crates/rendering/src/asset_gpu.rs) shares static model/texture resources and updates moving placements separately. The renderer subtracts an f64 scene origin before GPU conversion, preserving more useful precision than directly casting world positions. This is a good basis for larger scenes, though it is not a streaming/floating-origin world system.

PBR has shared WGSL, normal/scalar maps, mip generation and procedural prefiltered IBL/BRDF resources. Materials and GLBs have explicit limits and unsupported-feature errors. Static GLB import rejects animations, skins, morph targets, external texture URIs, unsupported attributes/extensions and alpha blending; opaque/masked rigid assets are the supported contract. Those restrictions matter for animated non-coaster rides and scenery.

### 8.2 Risks and missing general scene features

**Finding E05 · Medium now / High at park scale · extend R7 when scope grows.**

- `Workspace` owns one window/GPU/context/input state; R3 already calls for shared-device, per-window surfaces. Navigation banks are camera banks, not separate native windows.
- `PowerPreference::LowPower` is hard-coded ([gpu.rs](../../crates/rendering/src/gpu.rs), line 44). Device loss reports an error and exits the app path; it does not rebuild resources. R1 already owns this reliability work.
- Core pipelines/depth are single-sample, with no current MSAA, directional shadows or general light collection. R7 already covers the applicable baseline; avoid a new PBR milestone that ignores physics and data safety.
- There is no frustum/occlusion scene manager, LOD or streaming layer. Mesh draw batches iterate visible roles/components, rather than a spatially culled world. `scene::pick` scans track segments; `pick_support` walks candidates/triangles after bounds checks. Large support/track counts can make both rendering and picking expensive.
- Every support member becomes a separate construction `Component` in `supports::append_mesh` and a GPU batch. The document permits 32,768 aggregate support members. That allowed input can imply tens of thousands of batches; it is not a verified real-time scene size. Grouping/instancing must preserve member identity and selection.
- Whole-track geometry/construction changes replace derived outputs and trigger aggregate GPU preparation. Cached material-only changes avoid solving the path, but there is no general dirty-section/chunk regeneration scheme.
- GPU upload and texture/resource creation still occur on the render path. CPU worker generation prevents much blocking, but a large valid scene can still stall while uploading. CPU timers do not replace GPU timestamps, visible-window frame distributions or memory measurements.

No fresh GPU run or visible-window FPS measurement was performed for this audit. A passing CPU rendering-import test is not evidence of adapter recovery, shading correctness, refresh-rate behavior or 60 FPS.

### 8.3 Audio status

**Absent.** [qx-audio/src/lib.rs](../../crates/qx-audio/src/lib.rs) contains only `add(u64, u64)` and the template `2 + 2` test. The app does not depend on it. There is no device backend, mixer, voice lifecycle, spatialization, listener, simulation-event bridge or asset decoder.

The roadmap explicitly defers audio. When authorized, a useful initial design would consume timestamped ride/device events and physical source/listener poses through an adapter, keeping mixing/device I/O off the physics and rendering threads. This is a proposed boundary only; no audio implementation was started.

## 9. Persistence, compatibility and integrity

### 9.1 Implemented safeguards

[persistence.rs](../../crates/qx-document/src/persistence.rs) has a 16 MiB project limit, read growth check, explicit schema dispatch and strict deserialization. Current schema is 7; versions 1–6 follow explicit acceptance/default/migration rules, including preserving legacy transition shapes when converting Hermite regions. Unsupported versions and unknown fields fail rather than disappear during a read/write cycle. `serde_json` enables `float_roundtrip` in the root manifest.

`save` validates/serializes before publication, creates a unique sibling temporary file with `create_new`, writes/syncs/closes it and renames over the destination. Failure cleanup retains the old project. Its comment correctly limits power-loss/directory-entry durability claims. [tests/workflow.rs](../../crates/qx-document/tests/workflow.rs) checks overwrite, exact reopen/regeneration, malformed/versioned input and retaining the old file after an invalid save.

[command.rs](../../crates/qx-document/src/command.rs), `Editor::execute`, clones a candidate, applies a bounded command batch, validates and atomically publishes/history-records only if changed. Undo/redo are complete document snapshots. Renderer/GPU handles, live world state and editor docking are not serialized into the project.

Train assets use validated project-relative paths, separate mounting transforms, asynchronous import and bounded decoded/model caches. `train_assets::State::package` refuses to replace different bytes, checks destination containment and publishes asset files before the atomic project write. Missing references are preserved and surfaced as warnings; this is deliberate recoverable behavior.

### 9.2 Remaining data-safety and scalability issues

**Finding E06 · High · mostly already R2.**

- No project autosave, recovery journal, crash recovery, migration backup or saved-with application metadata. Atomic explicit save does not protect unsaved edits.
- A project plus its external assets is not one transaction. A failure can leave orphaned copied assets; a successful project save may still carry explicitly warned missing references. Moving a `.qx.json` alone is not a self-contained transfer. Preserve references and give users the owner-designed bundling/recovery behavior that R2 calls for.
- Most older-schema compatibility tests construct/mutate fixtures in test code. There is useful migration coverage, including a checked-in schema-1 example, but no organized immutable corpus covering every historical schema and important variant. R0/R2 already propose this corpus.
- Document validation is layered: `Project::validate` validates authoring structures, while explicit generation/`ValidateConnections` checks geometry-dependent conditions. Not every public `Editor::execute` caller automatically runs generation. This contract needs to be clear for future headless/import clients; E01 shows why “serialized successfully” must not be treated as “ready for physics.”
- History is bounded by count, not bytes. The app selects 64 full snapshots; accepted documents can contain large profiles, trains and supports. Full document clones, worker caches, generated tracks/reference points and GPU staging accumulate separate memory costs. No whole-session memory budget is established.
- Operations store absolute stations, while supports use section ID/fraction attachments. Both can be deliberate authoring choices, but they respond differently when upstream geometry changes. Future migration/undo/ride collections must preserve those explicit semantics rather than silently rebinding devices.

**Finding E07 · Medium · source-confirmed interactive-path work.** `Session::execute` appends synchronous `Command::ValidateConnections` for geometry/operations/support commands before publishing. That command regenerates the whole path and checks support endpoints. `expansion::committed_track` can also generate synchronously on a cache miss from `operations_ui::show`. Saving packages assets and serializes/syncs the project on the calling thread. The worker architecture therefore does not mean all expensive work is off the UI thread. Measure large-commit/save latency before adopting a prepared-candidate/async publication design that retains atomicity and stale-revision checks.

## 10. Barriers to additional amusement rides

The reusable `World` is a starting point, but adding a ride catalog entry or mounting a different GLB cannot supply a new ride's mechanics.

| Barrier | Current evidence | Consequence / future requirement |
| --- | --- | --- |
| Every body is dynamic with positive finite mass | `Body`, `MassProperties::validate`, `World` | No first-class fixed foundation/kinematic actuator or world-anchored joint. A grounded flat-ride mechanism needs these concepts. |
| Joint vocabulary is narrow | `JointModel::{Ball, Fixed, Hinge, RollLock}` | No prismatic/distance/spring/gear/cable constraints, no continuous multi-revolution coordinate; `AngularJoint::angle_rad` wraps a principal angle. Spinning rides need unwrapped drive coordinates and appropriate joints. |
| Persisted physical objects are `Train`/`Car`/track placements | `Train::validate` requires a nonempty car list and unique car-body components | A pendulum, drop tower, wheel or rotating platform should not be disguised as a coaster train. |
| Playback requires a track guide | `TrackSimulation::with_operations`, `guides.is_empty()` rejection | Generic rigid-body `Simulation` exists, but the app/runtime path assumes track-based vehicles. Other rides need mechanism-specific preparation over the shared world. |
| Operations are longitudinal station regions | `TrackDevice`, `devices::forces` | Rotational actuators, travel limits, lift holds and mechanism controllers need their own physical commands and state machines. |
| One coaster and one selected runtime train | `Project`, `Source::new`, worker's `Option<TrackSimulation>` | No ride-instance registry, instance identities or park scheduling; alternative definitions do not provide simultaneous operation. |
| No collision/environment world | No engine collision/terrain API; support mesh is rendering construction | Cannot support shared obstacle/contact interactions merely by composing rendered objects. |
| Static-only model import and absent audio | `asset::import_glb`; template `qx-audio` | Mechanical rigid components can be instanced, but skeletal/animated scenery, sound events and listener/source lifecycles remain separate missing systems. |

**Finding E08 · Strategic blocker · after current release gates, subject to owner scope.** A sensible first generalization test is one explicitly chosen non-coaster mechanism built from the existing rigid-body kernel, with fixed foundations, a measurable actuator and deterministic lifecycle. Use it to identify the minimum reusable mechanism/domain interfaces. Do not first build a universal ride framework, convert every ride into a rail, or replace the human-directed graph/timeline coaster workflow.

## 11. Testing, performance evidence and verification gaps

### 11.1 Coverage that exists

- **Geometry:** analytic lines/arcs, positional curve derivatives, rational curves, knots/weights/invalid input, arc-length inversion, continuity, representation-aware edits, source refinement, heartline rigid-point equations, construction/support validation and deterministic generation. See `qx-geometry/tests/{curves,generation,profiles,heartline,engineering_topology,rail_geometry,segmented_construction}.rs` and feature-gated graph tests.
- **Documents:** transactional rejection, undo/redo, save/reopen, migration, styles independent of geometry, train/asset definitions, support attachment and scalar-region precision. See `qx-document/tests/`.
- **Physics:** free fall/torque, momentum, guide constraints, incline motion, multicar/shared mechanisms, assembly residuals/convergence, roll stability, motors/limits, devices, reset/replay and velocity projection. See `qx-physics/tests/`; source/J/J-dot/energy research is extensive but separately scoped.
- **Application:** actual egui event-driven authoring and draft workflows plus headless CLI exports, not just helper unit tests. GPU/native smoke is explicitly gated/ignored.
- **Rendering/content:** malformed GLB range/index/hierarchy/features, bounded maps, transform/material behavior, exact Box Spined source extraction and construction alignment.
- **Independent tools:** Python verifiers reconstruct transition, geometry, force, energy and constraint quantities from exported evidence. They are stronger evidence than tests that merely repeat implementation arithmetic, but are not all part of normal CI.

### 11.2 Checks executed for this audit

All checks below completed on the audited native Windows checkout. Logs are local ignored artifacts under `target/core-engine-audit-20261115-175304/`. No tests were weakened or source code fixed during the audit.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --locked` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked --no-fail-fast` | PASS: **530 passed, 0 failed, 16 ignored**, across 65 test/doctest groups |
| `cargo clippy -p qx-geometry -p qx-physics --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo test -p qx-geometry -p qx-physics --all-features --release --locked --no-fail-fast` | PASS: **270 passed, 0 failed, 28 ignored**, across 42 test/doctest groups |
| Local station-domain probe under git-ignored `target/`: `cargo run --manifest-path target/core-engine-audit-20261115-175304/probe/Cargo.toml --offline` | PASS execution; reproduced E01 |

The two Cargo test runs cover overlapping tests in different configurations; their counts must not be added as unique coverage. Runtime timings from these audit runs are not real-time benchmarks, especially while other verification is running.

**Not run:** whole-workspace Release build/package, Python export verifiers, ignored long research/energy/timing controls, native hidden-window GPU smokes, visible native interaction, device-loss injection, cross-machine replay or park-scale profiling. No mouse/keyboard/window control was used. The tests exercised here do not establish native UX quality or physical ride safety.

Owner-executable manual follow-up, **not performed in this audit**:

```powershell
cargo run --release -p quantum-x -- edit examples/mixed-authoring.qx.json
```

1. Save As to a new project location before changing the supplied fixture. Edit a graph region, Apply, undo/redo, save and reopen; compare the authored values and geometry.
2. Through existing development tools, associate a development train and initialize playback. Check Play/Pause/Reset and Simulator entry/return, including reported initialization errors. This remains experimental fixture testing.
3. Resize/minimize/restore the window and exercise selection with supports/train visible. Record visible frame timings separately if making performance claims. Passing these actions is not a device-loss recovery test.

### 11.3 Missing gates and limitations of current CI

**Finding E09 · High · refine R0/R4, do not recreate CI.** [.github/workflows/ci.yml](../../.github/workflows/ci.yml) now defines pinned-toolchain Windows format, Clippy/debug tests and Release/package jobs. However, the test command does not enable `experimental-source-guidance`, so default CI does not exercise that feature-gated research. `--all-targets` does not mean `--all-features`. Ignored Release controls and GPU smokes remain outside the default run, by design. No percentage coverage was calculated.

Recommended additional gates, assigned to existing work:

| Gap | Where to add it | Acceptance evidence |
| --- | --- | --- |
| E01 source/physical station mismatch | R4 correctness + R5 preparation | Rider-reference device authoring/preflight/runtime/round-trip cases in both length directions |
| Feature-gated research regressions | R0 separate bounded job | Explicit feature/configuration matrix; long research controls on an appropriate scheduled/manual tier |
| Production full-ride energy/work | R4 | 60–180 s trajectories with gravity/device/resistance/constraint ledger, residuals and accepted bounds |
| Timestep/substep convergence and rotating bodies | R4 | Analytic torque-free/forced rotation, bank variation, guide/frame seams and mass/inertia ratios; preserve existing tolerances |
| Long-train/runtime envelope | R4/R6 | Initialization and p50/p95/p99/max step distributions per admitted car/curvature/speed range; reject or clearly delimit unqualified configurations |
| Persistent determinism references | R0/R4 | Tick-specific golden state/event fixtures, separate same-machine bit identity and cross-machine tolerance/identity verdicts |
| Operations invariants | R5 | Stateful/property-based dispatch, blocks, rollback/fault/reset and repeated complete ride cycles |
| Historical/corrupt inputs | R0/R2 | One fixture per schema plus variants; truncated/unknown/nonfinite/extreme-count and asset-containment cases; no panics/data erasure |
| Large scene and commit/save latency | R4/R7/R2 | Full-circuit supports, picking, upload cost, peak memory/history cost and interruptible/prepared publication behavior |
| GPU lifecycle | R1/R3/R7 existing tiers | Resize/minimize/occlusion, actual device loss and multi-window lifecycle; recorded target-machine evidence |

There are many hand-authored adversarial tests, but no dedicated fuzz/property-testing infrastructure or general benchmark regression job was found. `apps/quantum-x/src/benchmark.rs` provides useful long-spline generation observations; it measures sample/graph payload and explicitly excludes preparation, GPU and history. It is not a whole-session or park-scale benchmark.

## 12. Prioritized recommendations and roadmap reconciliation

### 12.1 Recommendation register

Priorities here describe ordering and consequence, not authorization. **P0:** establish release-critical correctness/evidence before expanded claims. **P1:** remove important scaling/reuse limitations in the relevant pass. **P2:** future platform expansion after owner scope/design. No proposed milestone was started.

| Priority | Recommendation | Finding / existing roadmap | Reason |
| --- | --- | --- | --- |
| P0 | Resolve the operations station-domain contract and reproduce it in cross-crate tests | E01; add to R4, prerequisite for R5 | A document can currently pass generation/save checks and fail on its actual physical simulation path |
| P0 | Qualify the actual production solver, train envelope and energy behavior | E02; existing R4/R6 | Real independent dynamics exist, but longer trains and full rides are not broadly qualified |
| P0 | Preserve recovery/version integrity while extending operations schemas | E06; existing R2 | Atomic explicit saving leaves unsaved-work and migration risks |
| P0 | Finish one deterministic operating ride cycle before broad device/ride expansion | E03; existing R5 | Force regions alone do not provide a usable operational simulation |
| P0 | Complete controller extraction with immutable source identity and replay equivalence | §3; existing R3 | Enables the owner-required Simulator window and future headless/multiple instances without duplicating workers |
| P1 | Profile immutable-track validation, nested copies, constraint rows and device curves | E02; R4 | Identified production costs grow with track/train/device size; optimize from measured attribution |
| P1 | Add explicit experimental-feature and schema-fixture verification tiers | E09; R0/R2/R4 | Existing default CI does not execute the complete research/compatibility evidence |
| P1 | Measure/limit synchronous commits, save work, whole-snapshot memory and support batching | E05/E07; R2/R4/R7 | Accepted resource counts are not interactive performance guarantees |
| P1 | Advance source/rail derivatives only through existing qualification work | §4.3; R4 research then later contact fidelity | Avoid duplicate solvers and accidental promotion of test harness mechanics |
| P2 | Establish shared ride/world identities and generic mechanism preparation | E04/E08; multiple-coaster proposal and later platform design | Required for additional rides, shared environment and simultaneous operation |
| P2 | Implement collision/contact, terrain/scenery, weather and audio as separately validated systems | §5/§7/§8; existing post-1.0 scope | Large missing subsystems, not catalog or rendering-only features |

### 12.2 What should not be duplicated

| Existing roadmap item | Current audit disposition |
| --- | --- |
| R0 verification infrastructure | R0.1 CPU checks and R0.2 verified package automation exist on this branch. Continue remaining golden/corpus/GPU-tier work; do not create another CI baseline. Hosted check success/branch protection was not verified here. |
| R1 Windows reliability | Device-loss/adapter/error paths remain relevant. This audit supplies engine consequences; it does not repeat packaging aesthetics or start shell changes. |
| R2 data safety | Still needed. Coordinate station/reference schema contracts and future ride IDs with compatibility/recovery work. |
| R3 controller and separate Simulator | Still needed. Current M4K mode is implemented in one window; older README/M4J text describing mode work as future is historical/stale. Preserve one controller/worker and explicit reload isolation. |
| R4 physics validation/performance | Highest engine risk. Add E01, immutable-track/hot-loop attribution and clear separation of production versus research energy evidence. |
| R5 ride operations | Still needed exactly as scoped; no station/lift/block implementation was found. |
| R6 production content | Affects mechanics validation as well as assets. Box Spined visual availability is not a validated production train or mechanical compatibility guarantee. |
| R7 graphics baseline | Reuse existing instancing/material/resource work. Add support-batch/picking/memory measurements before park-scale renderer abstractions. |
| R8–R10 UX/release/RC | Remain roadmap work outside this audit's main focus. No release-readiness approval is implied. |
| Deferred post-1.0 scope | Contact, collisions, terrain, scenery, weather, audio and multi-coaster support remain deferred there. The broader requested platform ambition needs a later approved roadmap extension, not an implicit expansion of current R0–R10. |

The earlier roadmap's capability inventory and README contain snapshot-era statements. Resolve precedence using the actual implementation and the latest relevant report: M4K/1 mode/timing are present, R0.1/2 CI/package automation are present, and default production stepping still uses sampled centerline guidance. Do not infer implementation from a planned catalog entry or from a past proposal's title.

### 12.3 Proposed next coherent milestones, for owner approval only

1. **R0 remainder + R4 correctness baseline.** Record a bounded feature/test matrix, production golden fixtures and immutable schema corpus; resolve E01's domain contract and test it; measure the production solver's full-ride energy and scaling. Preserve existing research suites and tolerances. Completion evidence must distinguish production, opt-in initializer and offline source-guidance results.
2. **R3 reusable simulation controller, with R2 data-safety work coordinated.** Extract the existing worker/source/pacing/presentation lifecycle, prepare an immutable physical-track/mechanism snapshot, and verify bit-identical replay against the current path. Implement the owner-required separate window only under its approved composition/interaction decisions. Preserve failed-reload recovery and independent editor saving/history.
3. **R5 complete single-train operations.** Station/dispatch/lift/anti-rollback/brake/return cycle, tick events, persisted operating settings and deterministic controller tests. Use force-limited physical actuators, no velocity assignment. Gate multiple trains on the R4 budget and add block occupancy/reservation invariants before admitting them.
4. **Later approved ride-platform foundation.** Build on the existing multiple-coaster proposal; separate ride definitions from runtime instances and shared environment. Validate one owner-chosen non-coaster mechanism with static anchors/appropriate actuators. Then scope collision, scenery/terrain, environmental fields and audio individually. This is a new future direction requiring approval, not part of current audit completion.

These are integrations of existing roadmap work, plus a clearly separated later platform proposal. They do not authorize a new visual identity, authoring workflow, generated coaster layout or generic engine rewrite.

## 13. Completion and preservation record

- **Delivered:** this audit report, subsystem maturity inventory, evidence paths/symbols, reproduced E01 diagnostic, prioritized recommendations and roadmap-aligned milestone proposals.
- **Production changes:** none. No fixes, dependencies, schemas or tests in production/package directories were modified.
- **UX status:** no approved UX behavior was changed; no temporary developer UI was introduced. Future Simulator, operations, train setup, ride selection and park workflows remain subject to the owner-directed designs/roadmap.
- **Desktop verification:** not manually verified; no active-desktop control or GUI launch was used.
- **Git:** existing branch and commits preserved. No checkout, branch creation, fetch, merge, reset, commit, push or PR mutation was performed. The audit report is a local uncommitted addition; this is an audit-only request, not an authorized development pass.
- **Limits:** findings describe the audited local commit and local verification, not unqueried remote state, future branches or certified physical/release readiness.
