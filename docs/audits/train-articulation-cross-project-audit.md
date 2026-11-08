# Train articulation, vehicle physics, assets and ride cameras: cross-project audit

Audit date: **2026-11-01**. Scope: the actual local working trees of QUANTUM Dynamics/Rust, C++ QUANTUM and C# QuantumCoasterWorksLib. This is an engineering audit, not an implementation pass. The only intended repository addition is this report. No code, models, assets, authored layouts or production schemas were changed; no Git publishing actions were performed.

## 1. Executive summary

**Preserve Rust's component graph and independent rigid-body foundation. Improve its geometric assembly and camera integration together.** Neither reference project is an all-round replacement for the Rust systems.

- Rust already persists arbitrary component IDs, variable car dimensions/masses, component-local mounts, mass centers, principal inertia, shared running-gear references, and ball/fixed/hinge connections. These are substantially more flexible than C#'s homogeneous, two-bogie consist. Rust's current project schema is **6**, despite several retained comments referring to Schema 5. [R1, R2]
- Rust really integrates independent bodies and angular velocities. It has equal-and-opposite joint impulses, free hinges, bounded torque motors, limits, diagnostics and transactional failed steps at **1/240 s**. These are experimental mechanisms, not a completed coaster vehicle implementation. **There is no wheel/rail contact solver in the inspected Rust source.** Its track guides are bilateral, frictionless centerline constraints. [R4–R6]
- The immediate articulation weakness is assembly: explicit track placements evaluate each component independently; legacy placement adjusts body orientation from two anchors without solving the bogies' arc-distance separation. Neither path closes a whole train's couplers geometrically. Tight curvature can therefore leave initial residuals that prevent playback or produce stabilization forces. [R3]
- C++ supplies the strongest reference for rigid two-pivot placement and consecutive-car coupling: bounded station searches, rigid chord-length closure, bank-aware frames, explicit failures, circuit/reverse handling and detailed regression tests. **Its regular train dynamics is reduced-coordinate dynamics, not independently articulated rigid bodies.** Adapt the geometry and verification ideas without replacing Rust's independent-body solver with prescribed track poses. [Q1–Q3]
- C# supplies useful batch sampling, frame construction, immutable pose snapshots, offset camera math and characterization tests. Its car placement is station based, and articulated bodies use the bogie chord and midpoint; it does **not** solve rigid bogie spacing or physical inter-car coupling. The named `TrainBogieTransformSolver` exists, but the inspected public provider uses its own `SampleTrainWithBogies` batching path; a repository search found no call to that helper. [C1–C4]
- Rust already renders independently moving components from inspection or simulated snapshots, with project-relative GLBs, pivots, separate visual corrections, shared mesh instances and a plain gray-box fallback. Keep this architecture and the owner's original assets. A finished train model or finalized component taxonomy is unnecessary for rider POV. [R7–R10]
- Rust's camera remains an orbit/preset camera with world-up and a pitch clamp. Framing a train or selected car is not a follow camera. C#'s camera builder is a good mathematical reference; its Preview Viewer follow camera is a centerline preview with frame-dependent smoothing. C++'s “Track camera” is a static, horizontal section-start preset. None establishes a finished physical rider-camera implementation to copy wholesale. [R11, C5, C6, Q7]

**Recommended next assignment:** integrate rigid geometric assembly/coupler closure, inspection scrubbing and body-attached rider/chase cameras using the existing gray boxes, then verify the same pose contract through experimental playback. Follow with physical contact/suspension work and specialized mechanisms as separate coherent assignments. No rewrite, artwork pass or wholesale schema redesign is justified by this audit.

## 2. Repositories and revisions inspected

| Project / source root | Branch | HEAD | Initial working tree |
| --- | --- | --- | --- |
| Rust: `C:\Dev8\QUANTUM-X` | `codex/m1-track-pbr-materials` | `493ac03434d96885e2c113995e471759fafc4d25` | 42 modified tracked files; 41 untracked status entries containing 67 files |
| C++: `C:\Dev8\QUANTUM` | `codex/precision-track-authoring-pass4-4` | `6efcf29465ddb4f084a1d3329b2070cf73da37f9` | 7 modified tracked files; 10 untracked status entries containing 30 files |
| C#: `C:\Dev8\QuantumCoasterWorksLib` | `feature/m169-track-workspace` | `a4ad4663d3b7ab8cf20748c9442fd19bb9d166ec` | 7 modified tracked files; 3 untracked test files |

Counts exclude ignored build products. A status entry may represent a directory, hence the separate untracked-file counts. All source inspection used filesystem contents, including uncommitted files, rather than `git show HEAD` or remote `main`. The C# local checkout was found immediately under `C:\Dev8`; no clone, fetch or remote revision substitution was needed. Its supplied upstream reference is [Coasterpete/QuantumCoasterWorksLib](https://github.com/Coasterpete/QuantumCoasterWorksLib).

Relevant Rust changes include `train_asset_ui.rs`, `panels.rs`, `layout_ui.rs`, `session.rs`, `asset.rs`, `asset_gpu.rs`, shaders and untracked material infrastructure. The audit includes the current PBR importer/material path and development-tool gating. The numerical train/consist files are present in the working tree and were inspected directly. C++ dirty work is chiefly audio/Application/tests and documentation; C# dirty work is editor straight-length interaction plus M169 regression tests. These changes were preserved.

The root `AGENTS.md` of each project was read. Rust's existing owner UX direction in `docs/ux/owner-mockups.md` was also read: retain Configuration/Preview/Summary and optional development controls; later owner palette decisions supersede older generic styling guidance. This audit authorizes no UX or appearance change.

### Source reference key

Paths below are relative to the indicated source root; links use the local sibling checkouts. Symbols distinguish the actual code from documentation claims.

| Ref | Source and principal symbols inspected |
| --- | --- |
| R1 | [`crates/qx-document/src/train.rs`](../../crates/qx-document/src/train.rs): `Asset` (line 14), `Component` (41), `Car` (60), `Connection` (69), `Train::validate` (139); `src/lib.rs::SCHEMA_VERSION`, `src/persistence.rs::from_json` |
| R2 | [`crates/qx-document/src/train/mechanism.rs`](../../crates/qx-document/src/train/mechanism.rs): `LocalFrame`, `Placement`, `JointModel`, `RotationDrive`; [`train/fixtures.rs`](../../crates/qx-document/src/train/fixtures.rs): `development_consist` |
| R3 | [`crates/qx-physics/src/consist.rs`](../../crates/qx-physics/src/consist.rs): `inspect` (45), `initialize` (246), `snapshot` (361); [`inspection.rs`](../../crates/qx-physics/src/inspection.rs): reference-only `inspect` |
| R4 | [`crates/qx-physics/src/body.rs`](../../crates/qx-physics/src/body.rs): `MassProperties`, `Body::integrate_forces`, `advance`, `impulse`; [`solver.rs`](../../crates/qx-physics/src/solver.rs): `World::step`, `step_on_track`, `step_inner`, `scalar_impulse`, `project_station` |
| R5 | [`crates/qx-physics/src/angular.rs`](../../crates/qx-physics/src/angular.rs): `AngularJoint::{validate,angle_rad,motor,solve,error}` |
| R6 | [`crates/qx-physics/src/playback.rs`](../../crates/qx-physics/src/playback.rs): `TrackSimulation`, `Simulation`; [`train.rs`](../../crates/qx-physics/src/train.rs): older in-memory `TrainDefinition::instantiate` |
| R7 | [`apps/quantum-x/src/authoring/train_playback.rs`](../../apps/quantum-x/src/authoring/train_playback.rs): `Source::new`, `Worker::new`, `State::{initialize,sync}`; `train_playback_ui.rs::controls` |
| R8 | [`apps/quantum-x/src/authoring/train_assets.rs`](../../apps/quantum-x/src/authoring/train_assets.rs): `State::{poll,package}`, `transform` (389), `Session::assign_train_asset`; `train_asset_ui.rs` |
| R9 | [`crates/rendering/src/asset.rs`](../../crates/rendering/src/asset.rs): `import_glb` (152), `Model::development_box` (725); [`asset_gpu.rs`](../../crates/rendering/src/asset_gpu.rs): `AssetGpu::{upload,camera,draw}`; `material_gpu.rs` |
| R10 | [`apps/quantum-x/src/authoring/train_preview.rs`](../../apps/quantum-x/src/authoring/train_preview.rs): `populate`, `lines`; `panels.rs` around lines 141–180; `layout_ui.rs::train_inspection` |
| R11 | [`crates/rendering/src/camera.rs`](../../crates/rendering/src/camera.rs): `Camera::{fit,eye,orbit,matrix_f64}`; [`workspace.rs`](../../crates/rendering/src/workspace.rs): `ViewCommand`, `EditorSurface`, per-profile navigation |
| R12 | [`crates/qx-geometry/src/solver.rs`](../../crates/qx-geometry/src/solver.rs): `GeneratedTrack::sample_at_distance` (150); [`crates/qx-style/src/compatibility.rs`](../../crates/qx-style/src/compatibility.rs): `VehicleCompatibility::assess` |
| C1 | [`Quantum.Track/TrainCarTransformProvider.cs`](../../../QuantumCoasterWorksLib/Quantum.Track/TrainCarTransformProvider.cs): `EvaluateTrainPose`, `EvaluateTrainWithBogies`, `SampleTrainWithBogies`, `CreateRuntimeSnapshotEvaluator`, `BuildBodyDistances` |
| C2 | [`Quantum.Track/Internal/TrainBogieTransformSolver.cs`](../../../QuantumCoasterWorksLib/Quantum.Track/Internal/TrainBogieTransformSolver.cs), `TrainArticulationFrameSolver.cs`, `TrainPoseAssembler.cs`, `TrainCarBodySampler.cs` |
| C3 | [`Quantum.Track/TrainConsistDefinition.cs`](../../../QuantumCoasterWorksLib/Quantum.Track/TrainConsistDefinition.cs), `TrainCarGeometry.cs`, `TrainBogieLayout.cs`, `TrainPoseResult.cs` |
| C4 | [`Quantum.Track/Internal/TrainWheelTransformLayoutSolver.cs`](../../../QuantumCoasterWorksLib/Quantum.Track/Internal/TrainWheelTransformLayoutSolver.cs): `BuildWheelTransforms`; `Quantum.Track/TrainWheelLayout.cs`, `WheelTransform.cs`; `Assets/Scripts/QuantumVisualizer/BackendTrainPipelineGizmoVisualizer.cs` wheel-offset application around line 1197 |
| C5 | [`Quantum.Track/CameraFrameBuilder.cs`](../../../QuantumCoasterWorksLib/Quantum.Track/CameraFrameBuilder.cs): `BuildRideCamera`, `BuildTargetCamera`, `BuildBRollCamera`, `BuildFlyViewCamera`; `CameraTransform.cs` |
| C6 | [`Quantum.PreviewViewer/wwwroot/app.js`](../../../QuantumCoasterWorksLib/Quantum.PreviewViewer/wwwroot/app.js): `setDistance` (1110), `updateDynamicObjects` (1121), `updateFollowCamera` (1139), `tick` (1779), `advancePlayback` (1794) |
| C7 | [`Quantum.Physics/TrainStepLoop.cs`](../../../QuantumCoasterWorksLib/Quantum.Physics/TrainStepLoop.cs), `TrainFollowerState.cs`: legacy/projected acceleration modes, scalar speed/distance integration |
| C8 | [`Quantum.PreviewViewer/Program.cs`](../../../QuantumCoasterWorksLib/Quantum.PreviewViewer/Program.cs): `ResolveTrainBodyStyleRole`, `ResolveConfiguredStyleRole`; `wwwroot/app.js::{createTrainVisualGroup,loadStyleAsset,createAssetVisualGroup}` |
| C9 | [`Quantum.IO/TrackLayout/V2/TrackLayoutPackageV2Dtos.cs`](../../../QuantumCoasterWorksLib/Quantum.IO/TrackLayout/V2/TrackLayoutPackageV2Dtos.cs); `Quantum.IO/TrainPose/V1/*`; `Quantum.Editor.Avalonia/Services/Trains/TrainConsistEditorSession.cs`, `Services/Documents/TrackDocumentFileService.cs` |
| Q1 | [`core/include/quantum/physics/TrainDefinition.hpp`](../../../QUANTUM/core/include/quantum/physics/TrainDefinition.hpp); `core/include/quantum/coaster/TrainConfiguration.hpp`; `core/src/TrainConfiguration.cpp`, `TrainConfigurationJson.cpp` |
| Q2 | [`core/src/physics/CarPose.cpp`](../../../QUANTUM/core/src/physics/CarPose.cpp): `bodyFrameFromBogies` (349), `rigidBogieTolerance` (503), `solveBogieStations` (520), two-bogie rejection around line 805; `core/include/quantum/physics/CarPose.hpp` |
| Q3 | [`core/src/physics/TrainPhysics.cpp`](../../../QUANTUM/core/src/physics/TrainPhysics.cpp): connector candidate/refinement around lines 770–1335, `solveTrainPose`, kinematic/effective-mass evaluation around 2780–2900, `stepTrain` (4859); corresponding `TrainPhysics.hpp` |
| Q4 | [`core/src/physics/DynamicContactPhysics.cpp`](../../../QUANTUM/core/src/physics/DynamicContactPhysics.cpp): aggregate contact restrictions around 935–1000, `solveLcp`, `solvePgs`, `stepDynamicContact`, `stepDynamicContactMultiCar`; corresponding header |
| Q5 | [`engine/include/quantum/physics/RigidBodyWorld.hpp`](../../../QUANTUM/engine/include/quantum/physics/RigidBodyWorld.hpp), `engine/src/physics/RigidBodyWorld.cpp`: Jolt-backed boxes, hinge/slider APIs, motors, `stepFixed` |
| Q6 | [`editor/src/TrainMeshPresentation.cpp`](../../../QUANTUM/editor/src/TrainMeshPresentation.cpp), corresponding header: `TrainVisualConsist`, `TrainVisualPrototype`, `updateTrainMeshInstances`; `RigidBodyMechanismProof.cpp/.hpp::RigidBodyMeshBinding` |
| Q7 | [`editor/src/ViewportCamera.cpp`](../../../QUANTUM/editor/src/ViewportCamera.cpp), corresponding header; `editor/src/EditorUi.cpp::applyTrackViewPreset` (8477) |
| Q8 | [`editor/src/SimulationPreview.cpp`](../../../QUANTUM/editor/src/SimulationPreview.cpp): `interpolateTrainPreviewPose` (262), fixed-step accumulation and render pose around lines 559–860 |
| Q9 | [`engine/src/StaticMeshAssets.cpp`](../../../QUANTUM/engine/src/StaticMeshAssets.cpp), corresponding header: `loadStaticMeshGlb`, `gltfVectorToQuantum`, `StaticMeshAssetCache`, `StaticMeshGpuHandleCache` |

## 3. Detailed feature-parity matrix

Statuses describe the named capability, not production readiness: **Implemented** = an actual implementation exists; **Partial** = important parts absent; **Experimental** = operating prototype with restricted scope; **Representable only** = data can express intent without a functioning solver; **Missing** = absent from the inspected path; **Not verified** = evidence insufficient for the stronger claim. Tests and family validation remain separate below.

Priority: **P1** next integrated articulation/camera assignment; **P2** contact/suspension and supporting numerical work; **P3** specialized mechanisms or demonstrated later need. “Preserve” means no replacement recommended.

| Feature | C# | C++ | Rust | Evidence | Limitation / improvement needed / priority |
| --- | --- | --- | --- | --- | --- |
| Ordered individual cars | Implemented: homogeneous indexed consist | Implemented: ordered authored cars, stable numeric IDs | Implemented: ordered `Car` references to stable string component IDs | C3; Q1; R1 | Preserve Rust's graph; improve assembly rather than replace its model. P1 |
| Lead/middle/rear roles | Partial: preview role resolver; same physical geometry | Implemented: lead/middle/rear/zero/trailer/custom metadata and visuals | Partial: order and per-component assets express distinct cars; no explicit role field | C8; Q1, Q6; R1, R8 | Explicit finalized role taxonomy is unnecessary now. Preserve arbitrary IDs. P3 |
| Heterogeneous lengths/masses | Missing in homogeneous transform definition; no per-car mass there | Implemented per-car dimensions, dry mass/loadout | Implemented per-component dimensions/mass/inertia | C3, C7; Q1; R1 | Available data does not guarantee correct curve assembly. P1 geometry |
| Body/chassis/passenger separation | Missing general component graph | Partial: primary car + bogie/contact data; separate general-body proof | Implemented representation; Experimental dynamics through mounted components and joints | C3; Q1, Q5; R1, R2, R5 | Rust `Mounted` establishes initialization only, not an implicit weld. Preserve |
| Arbitrary bogie counts | Missing: named front/rear pair | Representable only beyond two: vector storage, pose solver rejects other counts | Implemented variable associations; Partial placement beyond simple cases | C1–C3; Q1, Q2; R1, R3 | Rust must not inherit C++'s global two-bogie restriction. P1 |
| Shared bogies | Missing shared identity/topology | Missing in ordinary value-owned car bogies; general joints are separate | Implemented representation and basic shared-component world test; Partial real-track assembly | C3; Q1, Q5; R1, R3, R6 | A shared reference alone adds no physical joint. P1 supported-topology contract |
| Wheels/axles | Partial: count/radius/width/axle-spacing plus offset records | Partial: authored contact positions/normals/roles; not a complete axle/wheel system | Representable only as components/reference points; no wheel radius/contact law fields | C4; Q1, Q4; R1 | Neither wheel markers nor contact-role names establish rolling dynamics. P2 |
| Mass center/inertia | Missing per-body mass/inertia in transform pipeline | Implemented full symmetric authored tensor and loaded COM/inertia | Implemented COM and diagonal principal moments in component axes | C3, C7; Q1; R1, R4 | Rust lacks independent inertia-axis orientation/full tensor field; only extend for a measured need. P2/P3 |
| Local origins, mounts, pivots | Partial: frame-relative offsets; preview fit/rotation/offset | Implemented physical pivots/hitches and separate visual alignment | Implemented separate component origin, COM, mount frame, joint anchors, asset pivot | C4, C8; Q1, Q6; R1–R3, R8 | Keep visual transforms out of mechanics. Preserve |
| Arbitrary references and persistence | Partial: indices, pose export; track package excludes train composition | Implemented ordered consist JSON, stable car IDs; no general persisted train graph | Implemented validated IDs, mount-cycle rejection, train transactions and schema migration | C3, C9; Q1; R1, R2 | Unknown fields rejected, not silently erased. No new schema required for first camera pass. P1 preserve |
| Rigid body fitted over hills/curves | Partial: midpoint/chord frame | Implemented two-pivot rigid closure with bank-aware body frame | Partial: explicit components independently sampled; legacy rotation/centering approximation | C1, C2; Q2; R3 | Confirmed Rust assembly gap. P1 |
| Chord versus arc-distance correction | Missing: bogie stations stay at ±spacing/2 | Implemented bounded station-spacing root solve | Missing in current consist inspection | C1, C2; Q2; R3 | Do not treat arc spacing as rigid wheelbase. P1 |
| Consecutive-car/coupler closure | Missing: fixed center-station spacing | Implemented rigid connector length and nearest local root | Partial: actual anchor-coincidence joints during dynamics; no geometric train closure | C1; Q3; R3–R5 | Rust ball coupler has no distinct length parameter; anchor offsets or an explicit link body can express length. P1 |
| Banking, inversions, continuity | Partial: transported/banked frames, chord basis with fallbacks | Implemented within supported two-bogie kinematics | Partial: quaternion track frames and dynamic orientation; assembly robustness incomplete | C1, C2; Q2; R3, R12 | No end-to-end full-layout guarantee in Rust. P1 |
| Independent body translation/rotation | Missing: scalar track follower and computed poses | Experimental isolated Jolt mechanism; regular train is reduced-coordinate | Experimental: per-component rigid body with gyroscopic term and torque | C7; Q3, Q5; R4 | Preserve Rust. Do not replace dynamics with geometric placement. P1 integration |
| Ball/fixed/revolute joints | Missing physical mechanisms | Partial: exposed Jolt hinge/slider; regular train connectors are geometric constraints | Experimental ball + fixed + hinge with component-local frames | C3, C7; Q3, Q5; R2, R5 | C++ Jolt wrapper does not expose a general ball/fixed train-graph API. Preserve Rust |
| Limits and motors | Missing physical limits/motors | Experimental bounded velocity hinge/slider motors and slider travel limits | Experimental unilateral hinge limits, bounded spring/damper target-angle torque motor | Q5; R2, R5 | Different motor semantics; Rust principal-angle targets/limits stay inside ±pi. P2/P3 |
| Mechanically controlled 4D | Missing | Not verified as track-driven mechanical transmission; driven-arm proof is insufficient | Representable only `MechanicalProfile`; dynamics explicitly rejects it | Q5, Q6; R2, R5 | Free hinge, motor and track-driven transmission are three distinct capabilities. P3 |
| Wheel/rail unilateral contact | Missing | Experimental planar reduced contact; regular reaction/feasibility diagnostics are separate | Missing: bilateral centerline guidance only | C7; Q1, Q3, Q4; R4 | Actual 3D wheel/running/guide/upstop contact is P2; do not relabel guides |
| Suspension/compliance | Missing physical suspension | Partial: transverse contact DOFs and slider mechanism, not a validated suspension system | Missing spring/damper suspension/contact model | Q4, Q5; R2, R4 | Ball joints named “suspension” in fixtures are not suspension physics. P2 |
| Gravity/resistance/forces | Partial scalar gravity/drag/resistance; problematic legacy mode available | Implemented reduced distributed gravity, rotational effective mass, resistance and applied-force projection | Partial: gravity, arbitrary body forces/torques; no integrated resistance/aerodynamics | C7; Q1, Q3; R4 | Adapt physically dimensioned force models later; no arbitrary curvature speed multipliers. P2 |
| Reactions and conservation | Partial follower diagnostics, no physical joint loads | Implemented reduced load/reaction recovery with availability checks; Experimental contact impulses | Experimental impulse-derived joint/guide forces and angular torque; stabilization adds artificial energy | Q3, Q4; R4, R5 | Distinguish constraint forces from wheel loads. Expand conservation envelopes. P2 |
| Fixed stepping and failure | Implemented configurable fixed loop, not mandated 240 Hz | Implemented default 240 Hz; separate solver failure contracts | Implemented 240 Hz, substeps, clone/commit rollback of bodies and guide hints | C7; Q4, Q5, Q8; R4, R6, R7 | Jolt update throws without a transactional state restore in the wrapper. Preserve Rust rollback |
| Simulation/render interpolation | Missing independent-body interpolation | Implemented station interpolation plus fresh constrained pose solve | Missing: latest worker snapshot only | Q8; R7, R10 | Add timestamped previous/current body poses for smooth attached cameras. P1 |
| Heterogeneous visual models | Implemented lead/middle/rear styles in debug viewer | Implemented ordered visual prototypes and authored per-car assets | Implemented per-component GLBs independent of car order | C8; Q1, Q6; R8–R10 | Rust already has the more general component placement surface. Preserve |
| Independently moving rendered parts | Partial: static node hierarchies and sampled debug boxes | Implemented car shells plus separate rigid-body proof bindings | Implemented per-component pose instances; Experimental mechanical correctness | C6, C8; Q6; R3, R10 | An animated GLB skeleton is not required; a single rigid GLB cannot articulate internal nodes. P1 preserve |
| Mesh reuse/materials | Implemented cached glTF scenes/clones | Implemented CPU/GPU cache-based visual instances | Implemented shared `Arc<Model>`, GPU instancing, current PBR material path | C8; Q6; R8, R9 | Per-instance train recoloring is not established by a track-material editor. No artwork needed |
| Save/reopen/missing assets | Partial: style catalog + snapshot/pose formats, not a complete train project | Implemented visual asset/alignment in ordered consist; runtime asset cache | Implemented project-relative assignments, packaging, warnings and fallback boxes | C9; Q1; R8–R10 | Rust save/undo packaging has existing integration tests; preserve |
| Rider/local-offset camera math | Implemented library builder; no physical car source | Partial: editor presets/free navigation, no inspected body attachment | Missing attached-pose camera path | C5; Q7; R11 | Small renderer-neutral pose adapter can use gray boxes. P1 |
| Live follow/chase/scrub | Partial: centerline preview follow and distance scrub | Partial: static track preset; simulation interpolation separately available | Partial: inspection station scrub and train framing; no live following | C6; Q7, Q8; R7, R10, R11 | Integrate camera attachment and pose timing, keep preview/dynamics labels distinct. P1 |

## 4. Kinematic articulation: what to adapt from C#

### Actual pipeline and its limits

`TrainCarTransformProvider.EvaluateTrainPose` evaluates articulated cars with wheels, then `TrainPoseAssembler.BuildPoseResult` snapshots the result. The active `EvaluateTrainWithBogies` path builds all body/front/rear stations in `SampleTrainWithBogies` and queries them as a batch. The profile-backed variant batches through `BankingProfileSampler`. `TrainBogieTransformSolver.SolveBogies` implements similar station sampling but is not called by the inspected provider. Porting both would duplicate behavior. [C1, C2]

For car `i`, body station is `s_lead - i * CarSpacing`; bogies are at that station ± `BogieSpacing/2`. Body position becomes the midpoint of bogie positions, forward becomes front-minus-rear, and roll is chosen from projected average normals. Fallbacks try the original body's normal/binormal, then arbitrary perpendicular axes. This improves over orienting a car from only the center tangent on hills and bends. It does **not** ensure that the two world-space bogie pivots remain the authored rigid separation. [C1–C3]

An analytic circle illustrates the distinction: arc separation `d` on radius `R` gives chord `2R sin(d/(2R))`, generally shorter than `d`. Midpoint/chord orientation alone cannot make a rigid body with pivot separation `d` meet both bogies. This is a mathematical explanation, not a measured Rust test result.

`TrainWheelTransformLayoutSolver` accepts a variable count but builds paired wheels on sequential axles: `axleIndex=i/2`, `axleCount=(count+1)/2`. It alternates `LocalOffsetY=±WheelWidth/2`, uses zero Z and fixed axle spacing. Wheel frames/matrices remain equal to their bogie's frame/matrix; offsets are separate records and no rolling spin is solved. The legacy gizmo consumer applies Y along `frame.Normal`, not the binormal. Consequently that layout must not be imported as a universal wheel arrangement or assumed to be a physically correct lateral rail gauge. [C4]

Other restrictions: all cars share geometry/layout; `BogieSpacing <= CarLength`; train transform stations outside `[0,length]` throw; no shared-bogie identity, physical coupler closure or general passenger-body graph. A follower's optional looping is not evidence that the train transform pipeline crosses a circuit seam. Several nominally double matrices are constructed through float `ToMatrix4x4()` and converted back; Rust should retain f64 through domain transforms. [C1–C4, C7]

C# physics is a scalar follower, independent of that pose assembler: `TrainFollowerState.Update` computes distance from speed/acceleration, updates speed and then samples the curve. Gravity projects onto the track tangent; linear/quadratic drag and a constant acceleration-like rolling term oppose speed. The default `LegacyNormalComponent` mode additionally kicks longitudinal speed using a projected **normal** acceleration component. `TangentialProjected` uses the tangent component and can optionally add a curvature-normal speed influence (disabled by default). Those preview approximations do not establish physically correct normal-force work, body inertia, joint reactions or momentum conservation. Do not adapt either normal-to-speed shortcut into Rust vehicle dynamics. The C# loop's fixed delta is caller-configurable, and its follower clamps/wraps station; it has no equivalent independent-body transactional rollback mechanism. [C7]

### Useful ideas

1. **One immutable pose snapshot for consumers.** Keep Rust inspection/render/camera consumers on the same solved component poses and IDs. Include clear provenance: geometric preview versus dynamic snapshot.
2. **Bank-aware orthonormal frame construction.** Adapt projected-up/ordered-fallback tests, while reporting impossible/degenerate geometry rather than silently claiming success from a fallback.
3. **Batch sampling against one immutable track revision.** Avoid repeated rebuilds and inconsistent frames within one train query. Preserve Rust's existing generated-track snapshot rather than importing C# runtime infrastructure.
4. **Frame-local camera offsets and target-camera basis.** Adapt `BuildRideCamera`/`BuildTargetCamera` semantics to existing Rust f64 vectors/quaternions, with explicit renderer matrix conversion.
5. **Regression contracts.** Adapt midpoint/chord distinction, hierarchy consistency, duplicate-station ordering, banking-profile independence and mutation-safe snapshots. Do not copy inaccurate spacing as a golden physical requirement.

## 5. C++ algorithms and numerical behavior worth adapting

### Rigid car placement and coupling

`solveBogieStations` solves the world-space pivot distance, rather than assuming arc separation equals wheelbase. It starts from nominal longitudinal stations, expands a local bracket, then uses safeguarded secant/bisection refinement (bounded to 64 iterations). The closed-circuit search is bounded to avoid choosing a corresponding root on a different lap. Tolerances combine a physical distance tolerance with coordinate-scale machine precision. A continuation hint is accepted only for the same track/path/direction/geometry and sufficiently nearby station, with the bracketed solve retained as fallback. [Q2]

`bodyFrameFromBogies` maps the authored local pivot direction to the solved world chord and uses both banked up vectors, followed by deterministic fallbacks. It accommodates offset/asymmetric local pivots rather than assuming body origin equals midpoint. The ordinary solver still explicitly rejects any bogie count other than two and requires longitudinal separation; its by-value bogie storage does not provide shared physical identity. Adapt this as a supported numerical case inside Rust's flexible graph, not as a new document-wide restriction. [Q1, Q2]

`TrainPhysics` searches the next car's reference station using the previous car's rear hitch and candidate front hitch. It distinguishes nonzero connector length from coincident endpoints, selects a nearby local root, checks closure to `connectorLengthToleranceMeters=1e-8`, handles zero-length/tangential-root cases and rejects impossible solutions. Open-track searches account for the full bogie envelope, not only the lead station. These are directly relevant to Rust's current fixed-offset inspection. [Q3]

### What the physics actually is

The regular C++ train has one longitudinal generalized coordinate. All car poses depend on that coordinate through the rigid assembly solve. Its force model is more substantial than constant-speed spline animation: it derives COM and angular pose Jacobians, sums translational and rotational effective mass, projects gravity and applied forces by virtual work, and accounts for resistance. However, car bodies do not independently depart from the constrained pose manifold. Reduced connector/reaction recovery and contact feasibility diagnostics are not free wheel/rail motion. [Q3]

The separate `DynamicContactPhysics` path introduces longitudinal plus front/rear vertical coordinates: three DOFs for one car, `1+2N` for multiple cars. It evaluates gaps, contact Jacobians and a coupled mass matrix, with nonnegative contact impulses, complementarity checks and plastic impact behavior. Small systems enumerate active sets; larger systems use bounded deterministic projected Gauss–Seidel. Retries subdivide the fixed tick. Inputs are const; the single-car failure result restores its input state, and callers must not commit an unavailable result from either API. [Q4]

Its restrictions matter: planar vertical track motion, exactly two bogies per car, symmetric contact groups, vertical contact normals, no guide contacts, and explicit unsupported force/resistance cases. It is **not** general 3D wheel/rail contact over banked curves/inversions, and it does not model wheel rotation or a complete suspension. Searches found no `stepDynamicContact*` call in the inspected editor/engine source: treat it as a core experimental path, not the normal editor playback implementation. [Q4]

The Jolt-backed `RigidBodyWorld` is another separate subsystem: independent box bodies, collisions, hinges, velocity motors, sliders with limits/motors and a fixed host tick. Its exposed API does not implement the regular coaster train as that body/joint system. The driven-arm/passive-gondola proof verifies a mechanism, not a track-driven controlled-4D train. It converts relevant values to Jolt float types; its `stepFixed` throws on an update error without rolling the mutated world back. Rust's transactional and f64 behavior should be preserved. [Q5, Q6]

### Adaptation priorities

Use C++'s bounded closure searches, dimensioned diagnostics, circuit branch rules and regression scenarios first. Later use its contact feasibility/failure classifications and convergence tests as numerical references. Do not transplant the entire reduced-coordinate engine, Jolt wrapper or renderer into Rust. Source licensing was not reviewed for code reuse; these recommendations concern independently implemented algorithms and tests, not verbatim source copying.

## 6. Rust systems to preserve and confirmed gaps

### Preserve

- **Document authority and validation:** unique arbitrary string IDs, explicit local anchors, mount-cycle checks, strict fields, bounded counts, transactional `SetTrains`, undo/redo and versioned persistence. `Car.bogie_ids` is a grouping/reference list; mechanics come from `connections`, so zero/one/many/shared associations do not imply a different physics engine. [R1, R2]
- **Independent dynamics:** SI/f64 body state, mass-center integration, gyroscopic term `I^-1(torque - omega × I omega)`, normalized quaternion integration, pairwise impulses, hinge/fixed angular constraints and finite torque motors. `consist::snapshot` correctly translates simulated COM poses back to component origins. [R3–R5]
- **Failure honesty:** initialization rejects joint/guide errors above 0.05 m and angular errors above 0.05 rad; unsupported mechanical profiles are explicitly rejected; steps restore bodies, guide hints and clock on failure. Preserve these safeguards while improving the initial solve. [R3, R5, R6]
- **Separation from visuals:** playback source comparison removes asset assignments, so swapping GLBs/pivots does not restart mechanics. Mass/inertia are authored data, not inferred from GLB bounds. `Mounted` and asset correction are distinct operations. [R1, R7, R8]
- **Existing rendering/assets:** per-component rigid instances, shared model resources, missing-asset warnings and gray fallback boxes; packaging refuses to overwrite different asset bytes and guards project-relative destinations. Current importer/material work supports more than the older basic material path. [R8–R10]
- **Existing track representation:** `GeneratedTrack::sample_at_distance` uses arc-distance lookup, Hermite position interpolation and shortest quaternion slerp. Its generated frame orientation is an appropriate geometric query contract; do not build another independent spline-placement system. [R12]

### Confirmed missing or incomplete functionality

1. **Reliable rigid initial assembly across full layouts.** Explicit placements skip the legacy body-fitting loop entirely. Legacy fitting rotates from the first two anchors and averages translations across available anchors; it does not adjust stations or satisfy arbitrary graph closure. Shared legacy gear is initially placed from the first referencing car. [R3]
2. **A geometric inter-car closure solve.** Fixed `Car.offset_m` values seed placement, not a solved physical coupler length. Ball joints enforce coincident anchor points dynamically; no dedicated distance coupler, slack, break threshold or ball-joint angular cone is implemented. Fixed/hinge connections or an explicit link body can express some mechanisms already. [R1–R5]
3. **An attached camera pose path.** `ViewCommand` supplies fitting and axis presets, not rider/chase attachment. Current camera state cannot preserve arbitrary bank through vertical/inverted motion. [R11]
4. **Previous/current dynamic snapshots with presentation timestamps.** The worker publishes the latest pose only. There is no independent-body render interpolation or physical timeline seek/replay cache. [R7, R10]
5. **Actual contact, wheels and suspension.** Wheel references are points, guide reactions can push or pull without engagement, and no spring/damper rail-contact law, wheel radius/spin/traction or collision system is present in `qx-physics`. [R1, R4]
6. **Mechanical transmission.** `MechanicalProfile` carries keys/domain/transmission ID, but the ID has no implemented transmission object/solver behind it. Motorized seat angle control is not track-relative mechanical transmission. [R2, R5]
7. **Family-specific implementation validation.** Generic fixtures exercise ingredients; no inspected Rust fixture proves a manufacturer's train/family mechanics. Compatibility tags express declarations, not geometry or dynamics certification. [R12]

## 7. Rust coaster-family compatibility

These four columns are intentionally independent. “Representable” means generic bodies, mounts, dimensions, references and joints can describe the broad arrangement; it does not mean the schema contains every necessary future contact/suspension/transmission parameter. “Experimental” numerical support below means the shared body/joint foundation only. No family needs a separate physics engine merely because seats are above/below/beside the rail.

| Family | 1. Document representation | 2. Existing numerical/physical support | 3. Development fixtures actually present | 4. Actual family implementation validated |
| --- | --- | --- | --- | --- |
| Sit-down | Representable only: above-track bodies/passengers | Experimental generic rigid bodies and captive guides; no contact | Conventional arrangement; two/three-car tests | Not verified |
| Floorless | Representable only: body/passenger geometry can differ | Experimental same mechanisms; floor geometry has no special solver meaning | No dedicated fixture | Not verified |
| Dive | Representable only: wide/long heterogeneous components | Experimental; no dedicated holding-brake/operational or contact behavior | No dedicated fixture | Not verified |
| Inverted | Representable only: below-track mounts | Experimental generic body/guide dynamics | `BelowTrack`; banked/inverted reference and curved-consist tests, not a family model | Not verified |
| Flying | Representable only: oriented passenger mount, fixed/hinged assembly | Experimental joints; no validated tilt/locking/load behavior | RotatingSeats exercises related ingredients, not flying conversion | Not verified |
| Suspended/swinging | Representable only: below-track assembly + hinge | Experimental arbitrary-axis free hinge/limits; no validated rail carriage/suspension | BelowTrack and rotating passenger fixtures separately; no full suspended vehicle | Not verified |
| Wing | Representable only: lateral passenger mounts | Experimental general body/joint motion; no validated wing loads/contact | No dedicated fixture | Not verified |
| Stand-up | Representable only: passenger assembly/mount | Experimental rigid ingredients; no rider/restraint biomechanics | No dedicated fixture | Not verified |
| Motorcycle | Representable only: component dimensions and rider mount | Experimental rigid ingredients | No dedicated fixture | Not verified |
| Single-rail/monorail | Representable only: references do not prescribe two rails | Experimental centerline guidance; no rail-section-specific engagement | Generic two-longitudinal-guide fixture is rail-count independent, not monorail validation | Not verified |
| Hybrid | Representable only: train plus declared track compatibility | Experimental generic vehicle physics; track construction/material is separate | Generic consist tests; no hybrid vehicle/contact fixture | Not verified |
| Freely spinning | Representable only: passenger/body on free hinge | Experimental free hinge and arbitrary joint-axis tests | RotatingSeats swings around joint-local Z; not a tested full freely spinning coaster | Not verified |
| Mechanically controlled true 4D | Representable only at intent/profile level; full transmission unspecified | Missing transmission; free hinge/motor available separately | MechanicalProfile round-trip/explicit-rejection test; no track-driven transmission | Not verified |
| Custom articulated | Representable only within component/joint types and bounds | Experimental arbitrary graph, ball/fixed/hinge mechanisms; no general contact | Shared-component test, mounted chain, arbitrary axes and motor/limit tests | Not verified |

Evidence: [R1–R6, R12], `crates/qx-document/tests/vehicle_workflow.rs::{arbitrary_car_counts_roles_and_shared_running_gear_are_representable,compatibility_is_an_extensible_declaration_not_a_validation_claim}`, and `crates/qx-physics/tests/{vehicles,dynamics,consist,track_guidance}.rs`.

The shared-component dynamics fixture connects two colocated bodies to one bogie and steps the world. It proves identity resolution/basic topology, not that a shared bogie can negotiate a complete coaster. `different_arrangements_replay_reset_and_render_origins_use_mass_centers` runs three-car generic arrangements on a straight track; do not promote that to a family validation claim.

## 8. Train GLBs, bindings and original assets

Rust already allows each component of each car to choose a different GLB. It can represent lead/intermediate/rear visual differences without a finalized role enum. A component's visual correction is:

`world_point = component_origin + R_component * (translation + R_asset * (scale * (model_point - pivot)))`.

The simulated body pose is at COM; `consist::snapshot` converts it to component origin before rendering. Thus body COM, physical joint/mount, visual origin and model pivot are not conflated. Changing a visual assignment does not alter the authored centerline or mass properties. [R1, R3, R8]

`AssetGpu::upload` caches resources by shared `Arc<Model>` identity and batches instance matrices; primitives/node transforms and materials remain reusable. All nodes within one imported static GLB follow that component's rigid transform. The importer explicitly rejects animation/skinning, so multiple physically moving parts currently require separate component assets. This is sufficient for gray-box bodies, bogies and passenger assemblies and is preferable to pretending an internal GLB node is already mechanically bound. [R9, R10]

The current working tree imports metallic/roughness, normal, occlusion and emissive data through the shared material path, with import validation and tests. It rejects unsupported extensions and blended transparency. Material import is implemented; correct appearance/performance on a native viewport and arbitrary per-instance train material overrides were not verified in this audit. Track recoloring controls do not by themselves prove a train-material editing workflow. [R9]

C++ `TrainVisualConsist` orders one `TrainVisualPrototype` per render-pose car, with independent asset identifiers/local transforms and count validation. Its general rigid-body proof binds multiple moving bodies independently. Rust's per-component asset assignments already cover the key separation and exceed the one-shell-per-car presentation restriction; reuse the count/identity/transform regression principles, not the restriction. [Q6]

C++ `StaticMeshAssets` validates binary glTF, applies node transforms to imported geometry, uses an inverse-transpose normal transform and handles mirrored occurrences. `gltfVectorToQuantum` performs an explicit proper rotation into the C++ coordinate convention. CPU assets and GPU handles have separate caches. Static import rejects animations/skins; materials retain supported PBR factors/embedded textures but reject transparent/masked and emissive material features in the inspected path. Train JSON schema 2 preserves per-car visual identifiers and translation/rotation alongside independent physical data, with a 32-car deserialization limit. The shared asset system exposes missing/invalid/unsupported status and diagnostic fallback for track components; equivalent complete train missing-asset UX was not separately verified. [Q1, Q9]

C# Preview Viewer loads/caches glTF scenes, clones them, resolves lead/middle/rear style roles and falls back to debug boxes. Its fit-to-box/centering policy is a viewer convenience, not a physical pivot contract. The backend `TrainPose` export and track-layout package are separate; the inspected track Save/Open service serializes Track Layout Package V2, which contains no train composition. Do not infer full project train persistence from successful pose export. [C8, C9]

**Asset policy:** retain original neutral-gray placeholders and all owner files. `Model::development_box` is already plain gray (`[0.35,0.35,0.35,1]`); optional colored diagnostic lines are reference overlays, not finished train artwork. No new artwork, production IDs, catalogs or component taxonomy are proposed as prerequisites.

## 9. Numerical and architectural risks

| Finding and source-level evidence | Consequence | Recommended response |
| --- | --- | --- |
| **P1: two different assembly policies**, explicit Track/Mounted placement versus implicit legacy fitting; additional reference-only `inspection::inspect` [R3] | Apparently similar trains can produce different rigid-fit behavior; extra anchors need not close | One shared assembly result/diagnostic contract. Keep simple reference inspection explicitly reference-only; retain compatibility adapter rather than rewriting documents |
| **P1: no station closure**, followed by a 0.05 m initialization rejection [R3] | A valid authored train can fail on tighter curvature; a barely accepted mismatch produces corrective impulses | Solve rigid geometry first; report impossible assembly and residuals. Never change authored track, wheelbase or couplers silently |
| **P1: orbit-only camera and latest-only snapshots** [R7, R11] | POV cannot preserve roll; following a worker snapshot can visibly step | Full orientation camera pose plus timestamped previous/current body poses, tested independently of native UI |
| **P1/P2: open-only track query/projection**: sample range is `[0,length]`; projection clamps; endpoints reject escape [R4, R12] | A geometrically closed loop does not automatically behave as a circuit; no continuous lap identity | Add explicit validated topology/branch handling at the runtime boundary. Do not wrap arbitrary open tracks or infer closure merely from appearance |
| **P2: local projection has 12 iterations and rejects >2 m tangent correction** [R4] | Safe rejection instead of branch jumps, but large seek/long gear offsets/high speed can exhaust local tracking | Preserve bounded local hints; distinguish reinitialization/seek from step projection; test hairpins/crossings, long gear and failed projection. No universal robustness claim |
| **P2: Baumgarte factor 0.2 per substep**, sequential impulses, no general large-residual failure threshold after a finite step [R4, R5] | Artificial energy and order/iteration sensitivity; a finite but poorly constrained result can be committed | Quantify drift/residual envelopes, conditioning and mass-ratio sensitivity; decide acceptance thresholds from fixtures. Do not call guide reaction a measured wheel load |
| **P2: two centerline guide points do not constrain roll** [R2, R4] | A running-gear body's rotation about the guide line remains free unless joints constrain it; banked track frames alone do not supply physical rail roll engagement | Distinguish free motion from an assembly defect; verify roll transfer through declared joints and later real contact |
| **P2: repeated validation, cloning and track scans** in each fixed step; repeated sample lookup inside constraint iterations [R4, R7] | Potential costs grow with sample count × guides × iterations × substeps; no benchmark proves a bottleneck yet | Measure before caching validated immutable track state/bases. Preserve rollback and safe public APIs |
| **P2/P3: old in-memory `qx_physics::train::TrainDefinition` plus persisted document adapter** [R3, R6] | Two definition entry paths could diverge; currently they serve distinct tests/adapters | Share conversion/invariants where demonstrated useful; no audit evidence demands deleting either system |
| **P3: component kind and car-order restrictions** [R1] | Five coarse kinds, positive masses/dimensions, unique CarBody references, strictly increasing offsets and finite size limits are real boundaries; arbitrary graph is not arbitrary everything | Preserve existing format for P1. Revisit static parts, per-car stable identity independent of body ID or richer physical data only when required |
| **P1/P3: identity/order alignment**: preview zips components with poses; snapshots use body order [R3, R10] | A new reordered assembly result could silently bind wrong geometry if it breaks the invariant | Return definition order or explicit checked IDs; test arbitrary ID names, shuffled definitions and heterogeneous visuals |
| **P1: inspection errors can be visually quiet**: `panels.rs` populates only on `Ok`; residual details are development disclosures [R10] | A failed station may leave no train; a future follow camera must not retain a stale attachment unnoticed | Surface concise assembly/attachment failure feedback, preserve last valid state deliberately, keep technical detail optional |
| **P1: coordinate differences**: C++ +X forward/+Y lateral/+Z up; Rust and C# frame offsets use +X forward/+Y up/+Z lateral [Q1, R1, C5] | Blind swaps can reflect handedness or invert banking/hinge axes | Convert complete orthonormal bases and rotations with an explicit adapter; test determinant, axis signs and quaternion equivalence. Do not just exchange Y/Z components |

No evidence supports the claim that Rust physics is improperly dependent on GLB geometry. The stripping of asset data from playback sources and independent physical properties are positive evidence of separation. Likewise, no global requirement for exactly two bogies or fixed wheel counts exists in the Rust document model.

## 10. Ride-camera integration opportunities

| Requested behavior | Actual reference support | Rust integration opportunity / remaining work |
| --- | --- | --- |
| Rider POV / train-attached | C# `BuildRideCamera` uses local offset and frame axes; not a physical car binding | Transform eye and view axes by a chosen solved component pose. Use simulated pose during dynamics and geometric pose during inspection, visibly distinguishing the source |
| Front/rear row POV | Neither a row selector nor saved seat layout is established by the inspected camera builders | Select lead/rear car or a passenger assembly plus an adjustable local eye offset. Row naming/layout is owner UX; no finished model needed |
| Car-follow/chase | C# centerline follow plus target-camera math; C++ static low section-start preset | Body-local chase position and target, full orientation/up policy, clear attachment identity and fallback |
| Track-follow preview | C# Preview Viewer samples centerline with time-based station advance | Use existing generated-track sampling and inspection station; explicitly geometric preview rather than vehicle dynamics |
| Banking, yaw, pitch, inversions | C# library copies frame axes; Rust orbit has world-up/pitch clamp | Feed a quaternion/full basis into rendering. Avoid reconstructing a rider view through clamped yaw/pitch or world-up look-at |
| Adjustable rider/camera offsets | C# frame-local offsets; Rust already has stable component-local coordinates | Session-only attachment/offset state is enough initially; physical COM and GLB pivot must not define eye position implicitly |
| Frame-rate-independent playback | Rust worker fixes physics dt; C# preview uses elapsed time but clamps long frames | Preserve 240 Hz integration. Add time-aware presentation interpolation/smoothing; never enlarge physics dt |
| Track seek/scrubbing | C# `setDistance`; Rust exact station and slider in inspection | Scrub the geometric assembly; reinitialize/clear previous pose history on seek. Arbitrary physical-time seeking needs replay/checkpoints, not moving bodies to a spline station |
| Articulated passenger camera | Generic C# frame builder can accept an equivalent frame, but no physical source established | Attach by component ID to the passenger body, so its independent hinge rotation appears in the camera automatically |

C# Preview Viewer `updateFollowCamera` uses the **lead centerline frame**, not `TrainPoseResult` or physical body state. Its position/target interpolation alpha is fixed at 0.18 while playing or 0.28 while paused; smoothing therefore depends on render frequency. It does not assign the track bank to `camera.up` in that follow routine. `advancePlayback` advances station by speed × elapsed seconds, caps long-frame delta at 0.08 s and wraps the preview range. This is useful preview behavior, not fixed-step vehicle simulation or inversion-safe physical POV. [C6]

C++ `applyTrackViewPreset` deliberately discards the vertical tangent component and chooses a static section-start view. `ViewportCamera` supplies good navigation, focus, clipping and pole tests but has no full body-attached roll state in the inspected API. Its `interpolateTrainPreviewPose` interpolates the longitudinal coordinate and resolves the constrained car geometry again; that exact approach is valid for its reduced train model, not for interpolating Rust's independent body states. [Q7, Q8]

For Rust, use the already authoritative `ConsistInspection` component origins as the initial shared consumer boundary. A local eye offset `e` becomes `p_eye = p_component + R_component e`; local view orientation composes with `R_component`. Dynamic presentation should use adjacent committed poses with quaternion sign/shortest-path handling and a common presentation time for camera and train. Reset history on seek/reset/track or train replacement. Keep interpolation transient and out of the fixed-step state. [R3, R7, R10]

If smoothing is desired, a proposed frame-time coefficient such as `1-exp(-dt/tau)` avoids the C# fixed-alpha problem. Whether POV inherits all bank or offers an optional stabilized horizon is an owner-directed camera behavior. Neither policy is silently selected by this report.

## 11. Recommended coherent implementation assignments and boundaries

These recommendations are intentionally combined; they are not requests to create separate planning documents or approval gates for routine technical work. **This audit does not implement them.**

### Priority 1: rigid assembly, coupled gray-box train and functional ride cameras

Deliver one integrated assignment covering A, B and the immediately necessary part of C/D:

- A shared geometric assembly path for the existing definitions, rigid two-pivot/chord closure, consecutive-car anchor/coupler closure, bank-aware frames and explicit unsupported/degenerate cases. Preserve arbitrary IDs and the ability to represent one/many/shared running gear. Document which graph cases are actually solved; do not pretend the two-pivot algorithm handles every graph.
- Existing inspection station entry/scrubbing feeding the same poses as gray-box rendering and camera attachment. Add rider/front/rear/car-follow/track-preview pose services with local offsets and clear pose provenance. UI follows owner direction; minimal missing interaction design may use explicitly replaceable developer controls.
- Previous/current committed simulation poses and presentation timing, so the camera follows the same component transform displayed by the renderer. Preserve 240 Hz integration, rollback, pause/single-step/reset, source invalidation and visual replacement behavior.
- Regression coverage for rigid pivots/couplers, hills/valleys, banked curves/inversions, endpoints, heterogeneous cars, malformed/unusual/shared arrangements, camera offsets/roll, frame-rate independence and save/reopen. Preserve source tracks and gray assets byte-for-byte.

Suggested ownership: `qx-geometry` retains frame/distance/topology queries; `qx-physics::consist` or a cohesive adjacent module owns derived assembly and physics initialization; `qx-document` retains authoritative definitions/commands; renderer receives a f64 eye/orientation/projection contract; application owns transient attachment choice, clock/history and approved controls. No camera needs GLB access or a new train family engine. Prefer a runtime topology contract over unnecessary production format changes; circuit intent must still be explicit and validated.

Acceptance: a person can inspect/scrub the existing gray-box consist, see rigid connections or a specific failure, choose a usable attached/chase view, and run supported experimental dynamics without camera/visual/physical pose disagreement. This remains an experimental simulator, not proof of contact realism or every family.

### Priority 2: wheel/rail contact, compliance and numerical limits

Build a shared contact/suspension mechanism into the existing numerical foundation, with a deliberately documented first physical scope. Define physical running surfaces/contact geometry independently of rendered meshes; normal engagement/separation, running/guide/upstop behavior, stiffness/damping and wheel/axle rotational assumptions need explicit dimensioned models. Include gravity/resistance, large mass ratios, contact impulses, energy/momentum budgets, conditioning and failed-step recovery in one numerical verification pass.

Use C++ planar contact as a reference fixture/diagnostic source, while clearly identifying any restricted first Rust model. A planar proof is not all-layout 3D contact. Do not weaken current transactional semantics or hide large constraint corrections as realistic suspension. Add only schema fields required by the chosen, reviewable physical model.

### Priority 3: specialized articulated and controlled mechanisms

Extend the same bodies/joints/contact mechanisms for suspended/spinning/flying configurations and genuine track-driven mechanical transmission. Verify free hinges, bounded motors and mechanical transmission separately. Establish energy flow, track-relative input, gearing/limits and failure behavior before calling anything true 4D. Any future asset/component taxonomy should follow actual mechanisms and owner decisions, not precede working cameras or gray-box articulation.

## 12. Verification evidence and proposed regression scenarios

### Checks actually executed during this audit

| Check | Result | Scope / caveat |
| --- | --- | --- |
| Rust `cargo test --locked --offline -p qx-physics -p qx-document --tests` | **57 passed, 0 failed**: 24 physics + 33 document integration tests | Built against current local source. Includes train/vehicle workflow, curve guidance, inverted-loop reaction/drift, independent joints and rollback. Zero-test unit targets add no tests to the count |
| Rust `cargo test --locked --offline -p qx-rendering --test assets --lib camera::tests` | **2 camera tests passed** | Filter also ran the assets target with 0 selected/6 filtered tests; those were not counted as executed |
| Rust `cargo test --locked --offline -p qx-rendering --test assets` | **6 passed, 0 failed** | Current GLB hierarchy, actual Blender GLB, material channels/tangents and malformed-input rejection tests. No viewport screenshots or GPU performance validation |
| C# targeted `dotnet test` command below | **116 passed, 0 failed, 0 skipped** | Rebuilt current source using installed .NET SDK 10.0.400, net8.0 test assembly; no native UI interaction |
| C++ targeted `ctest` command below | **12/12 test executables passed** | **Pre-existing Release binaries, not rebuilt**. Useful runtime evidence for those artifacts, not proof they match every inspected source byte. Total CTest wall time reported 47.02 s; this is not an application benchmark |
| Initial Git branch/HEAD/status and final preservation comparison | See preservation result below | Tracked diffs and pre-existing untracked file hashes checked separately from ignored build products |

C# command, from its checkout:

```powershell
dotnet test Quantum.Tests/Quantum.Tests.csproj --no-restore --filter 'FullyQualifiedName~TrainCarTransformProviderTests|FullyQualifiedName~TrainPoseDeterministicRegressionFixtureTests|FullyQualifiedName~CameraFrameBuilderTests|FullyQualifiedName~TrainWheelLayoutTests|FullyQualifiedName~BackendTrainPipelineDeterministicTrackTests' --verbosity minimal
```

C++ command, from its checkout:

```powershell
ctest --test-dir build -C Release --output-on-failure --timeout 60 -R '^(QuantumCore\.(CarPosePhysics|TrainPhysics|DynamicContactPhysics)|QuantumEngine\.(RigidBodyWorld|RigidBodyConstraints|RigidBodySliders)|QuantumEditor\.(TrainMeshPresentation|TrainVisualConsist|RigidBodyArticulatedMechanism|RigidBodyMeshBinding|RigidBodyPresentation|ViewportCamera))$'
```

These are counts of Rust/C# test cases versus C++ **test executables**, not directly comparable coverage measures. No historical documentation result is included in these totals. Existing Rust M8.1/M8.2 application tests were reviewed (`m81_tests.rs` assignment/pivots/save-as/missing assets; `m82_tests.rs` worker buttons/visual independence/development gating), but were **not executed** here. The audit did not run workspace fmt/check/clippy/full tests, a Release rebuild, all C# tests, or all C++ tests: no implementation changed, and targeted checks answered the audited questions without rebuilding entire applications.

### Existing regression evidence worth retaining/adapting

- **Rust:** `qx-physics/tests/dynamics.rs` checks pairwise momentum, off-center rotation, free-fall energy/substep convergence and rollback; `track_guidance.rs` checks curved deterministic motion, banked reference inspection and an inverted loop; `vehicles.rs` checks arbitrary hinge axes, limits/motor bounds, COM conversion, mounts and unsupported mechanical profiles. These tests substantiate mechanisms, not full layouts/families. The inverted-loop test permits <0.01 m guide error, <1% speed drift and a 20 N mean-force difference for its 100 kg, 8 m/s, 10 m radius ideal-guide case.
- **C#:** `TrainCarTransformProviderTests` distinguishes chord/midpoint bodies from single-point frames and checks complete hierarchy/banking; `TrainPoseDeterministicRegressionFixtureTests` stores straight/curved snapshots at 1e-6 comparison tolerance. `CanonicalTransportedFrameTests::{QuarterLoopLikeFrames_RemainContinuousFiniteAndRightHanded,SmoothSegmentBoundary_PreservesFrameContinuity,DuplicateDistances_AreDeterministicAndPreserveInputOrder}` and transported-frame tests were reviewed as additional candidates, not executed by the chosen filter.
- **C++:** `CarPosePhysicsTests::{straightToRisingTransitionIsRigidAndContinuous,asymmetricAndOffsetBogiePivotsRemainRigid,verticalTrackHasNoSingularity,deterministicFallbackHandlesOpposedUpVectors,openTrackEndpointsRemainRigidOrFailDeterministically,circuitSeamIsContinuous}`; `TrainPhysicsTests::{heterogeneousCarsUseTheirOwnGeometry,crestAndValleyArticulate,sixCarClosureDoesNotAccumulate,closestLocalCircuitRootIsSelected,impossibleConnectorFailsExplicitly,consistCrossesCircuitSeamContinuously,reverseTravelPreservesConsistOrder}`. These are the highest-value geometry scenarios to adapt.
- **C++ physical prototypes:** `DynamicContactPhysicsTests` covers support/release, plastic upstop impact, reverse determinism, rejected scope, heterogeneous multi-car contact, connector residuals and repeated steps. `RigidBodyArticulatedMechanismTests::assemblyAndTrajectory` exercises a motorized arm and passive gondola for 7,200 ticks twice, checking attachment separation and independent bindings; its assertions are mechanism tests, not proof of a coaster or 4D transmission.

### Proposed verification for the next assignment (not performed results)

| Scenario | Required assertions / independent reference |
| --- | --- |
| Straight, unequal cars, asymmetric/offset pivots | Exact analytic transforms; rigid distances and coupler anchors within an explicitly chosen f64 length tolerance; changing meshes cannot change results |
| Sharp hill/valley and straight-to-curve transition | Sweep stations over both sides of joins; rigid pivots/couplers stay closed or fail explicitly; no silent body scaling or track reshaping |
| Circle and tight horizontal hairpin | Compare against analytic chord relation and independent root calculation; retain nearest branch, report impossible wheelbase |
| Banked turns, vertical tangent, full inversion | Orthonormal/right-handed body/camera frames; continuous quaternion signs/view direction; no world-up reset, pitch clipping or NaNs |
| Two, six and heterogeneous longer consists | No cumulative spacing drift; closure measured at every connection; permutation of component IDs cannot change mechanics |
| Zero/one/three bogies and shared gear | Schema accepts valid graphs; solver declares actual supported cases; shared body appears once; overconstrained/impossible arrangement reports residuals rather than inventing a fit |
| Open endpoints and valid circuit seam | Full train envelope respected; failed step preserves state/clock/hints; circuit positions/orientations remain continuous in both travel directions; open tracks never wrap implicitly |
| Self-crossing/near-parallel track and large seek | Local step projection stays on its branch; explicit seek resets history and reinitializes safely; invalid branch tracking rejects transactionally |
| Rider/front/rear/chase offsets | Test `p + R e` independently of GLB pivots/COM, including a separately hinged passenger body and missing assets |
| 30/60/144 Hz rendering and a delayed frame | Same fixed-step sequence for the same requested ticks; common camera/train presentation time, bounded lag policy and frame-time smoothing; no frame-count speed dependence |
| Motor, joint mass ratio, stabilization | Equal/opposite impulses, finite maximum torque, limit signs, residual/energy envelopes over long runs and substep convergence; distinguish driven work from artificial stabilization energy |
| Contact/suspension follow-up | Nonnegative normal impulses, separation/re-engagement, running/guide/upstop roles, impact energy loss, compliance timestep convergence and failure rollback |
| Save/undo/reopen/asset replacement | Preserve all IDs, authored dimensions, mechanics, pivots and original file bytes; camera session state does not cause unrelated schema writes |

For ordinary metre-scale geometric closure, a candidate target is 1e-6 m with a documented coordinate-scale term, accompanied by convergence against an independent analytic/reference case. This is a **proposed acceptance target**, not achieved Rust performance. Dynamic residual/energy thresholds need separate physical and numerical justification; do not reuse the current 0.05 m initialization gate as a precision guarantee.

### Screenshots, native checks and executable

No desktop control, native interaction or new screenshots were performed. There is no changed UI to document visually in this read-only audit; existing screenshots are not represented as new evidence. Native camera behavior, GPU material appearance, high-DPI interaction and actual application frame rate are **not manually verified**.

Existing executable found: **`C:\Dev8\QUANTUM-X\target\release\quantum-x.exe`** (21,209,600 bytes). It was neither rebuilt nor launched by this audit, and its presence is not a claim that it includes any proposed articulation/camera changes.

Owner-executable manual follow-up after implementation: launch that path (or the newly built/package path), open an existing project, scrub a gray-box train through a hill, turn and inversion; compare front/rear/chase views and a passenger-body attachment; play/pause/reset, replace a model, save/reopen and confirm local offsets and assembly behavior. Check native resizing/DPI and record fresh screenshots then. Current missing camera modes cannot be manually verified into existence.

**Preservation verified:** all three HEADs and branch names are unchanged; each repository's tracked diff matches the initial snapshot; SHA-256 checks found no changes or removals among the **100 pre-existing untracked files**. The only new nonignored file is this report. C++ and C# status listings are unchanged. Rust has only the additional `docs/audits/` status entry. Test runs generated ordinary ignored build/test products. All 37 local report links resolve, all 13 requested numbered sections are present, and `git diff --check` passed. No commit, push, merge, PR or publication occurred.

## 13. Owner-directed decisions still needed

Only material product decisions are raised; none prevents the nonvisual assembly/camera infrastructure described above.

1. **Ride-camera interaction:** where POV/front/rear/chase choices and local offsets belong in the existing Configuration/Preview/Summary workflow, including how a person chooses a passenger/seat attachment. A provisional development control is not approval of final UX.
2. **View behavior:** strict body-relative bank for rider POV, optional stabilized horizon for chase, and expected look-around/return-to-editor behavior. These are separate camera behaviors, not physics choices.
3. **Persistence scope:** whether named rider locations/camera presets eventually belong to reusable train definitions, projects or application preferences. Begin with transient offsets until a saved workflow is specified; no current schema migration is required just to produce POV.

Final train artwork, production component IDs/taxonomy, manufacturer fidelity and a complete family catalog remain deliberately undecided. They are not blockers for the first integrated gray-box articulation and ride-camera assignment. This report adds no approved UX behavior or temporary UI; it preserves the existing owner-directed interface and marks proposed behavior as future work.
