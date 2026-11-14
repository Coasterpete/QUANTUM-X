# Native Editor Mode and Simulator Mode — M4K proposal

Status: proposed architecture and UX outline, **not an approved redesign**.
M4J implements no mode transition. Public product: QUANTUM Dynamics; repository:
QUANTUM-X. Both workflows use the same production physics and rendered scene.

## Existing implementation, established by source inspection

| Responsibility | Existing owner / behavior | Missing for a separate mode |
| --- | --- | --- |
| Authored project and history | `qx-document::command::Editor`; command transactions, undo/redo | No second editor is needed |
| File identity and dirty comparison | `authoring::Session.path`, `saved`, `dirty()`; startup/file guards | Mode entry must bypass neither drafts nor close guards |
| Authoring state | Session drafts, selected section/graph region, graph viewport, workbench layout/preferences | Preserve the same Session instance across modes |
| Geometry preparation | Session generation worker, queued job, revision/visible_revision and visible_preview | Explicit snapshot identity tied to committed authored inputs |
| Physical reference | `InspectionScene::train_track()` resolves the physical track, including heartline conversion | Readiness check and immutable snapshot handoff |
| Simulation | `TrackSimulation`, `World`, 1/240 s, four playback substeps, devices and telemetry | Session lifecycle decoupled from editor panel composition |
| Worker | `train_playback`: epoch requests, load/actions, latest snapshot, eight-pose timeline, bounded catch-up | Mode preparation/cancellation/retirement protocol and shutdown acknowledgement |
| Presentation | `ride_camera`: one latched interpolation result for camera and train instances | Mode-independent presentation packet ownership |
| Vehicle rendering | `train_preview` plus existing GLB or labeled development gray boxes; COM-to-component transform in `consist::snapshot` | No articulated rendering rewrite needed for current supported fixtures |
| Camera | Renderer orbit/pan/zoom with attached `CameraView`; Track/Train workspace navigation banks | Independent Editor/Simulator navigation banks and explicit controller ownership |
| Attached views | Free, Rider, Front, Rear, Chase, geometric Track Preview; component-local offsets and rotations | Simulator controller must require a physical pose stream for onboard views |
| Environment rendering | `qx-rendering` environment/material/background lighting facilities | No simulated weather, biomes or environmental force model |
| Audio | `qx-audio` currently contains only the placeholder `add` example/test | No device backend, sound assets, mixer, listener or playback lifecycle exists |

The current Free Camera is renderer inspection navigation; do not label it a
completed six-degree free-fly or walk controller. Track Preview advances an
inspection station geometrically; it is not train dynamics and must not become
the source of Simulator onboard motion. Existing train/camera capabilities are
development tools, not proof of a finished Simulator workflow.

## State and dependency model

Proposed ownership (names are illustrative, not M4J APIs):

```text
Native application / window / shared wgpu device + surface + egui
  ProjectSession (one authoritative authored project)
    Editor (document + undo/redo), path/saved identity/dirty state
    EditorWorkspace (selection, drafts, graph/timeline, layout, editor navigation)
    GenerationService (revision, pending work, latest accepted geometry)
    ModeState = Editor | Preparing(ticket) | Simulator(session)
  SimulatorSession (transient, derived, never an authored document)
    source_key + immutable input snapshot / qualified generated track
    train assembly + successful initial-state/reset snapshot
    simulation worker and epoch / playback clock / errors
    bounded previous/current presentation poses and telemetry
    simulator camera controller + presentation settings
    optional future audio/environment playback handles
```

An input snapshot clones only immutable mechanics/operations/reference inputs
needed to initialize safely; it is not an independently editable Project or a
second source of truth. The authored Editor remains alive and owns all future
edits. No simulation pose is written back as track, placement or document data.

Proposed `SourceKey`: project-session identity, committed authoring revision,
geometry generation identity/options, physical-reference settings, train stable
ID/mechanical revision, operations revision and initialization policy. Do not
equate the existing Session generation counter with a complete document version:
visual-only changes and draft generation have different semantics. M4K should
define explicit keys at the adapter boundary and initially invalidate broadly
when unsure. No persisted schema change is needed for transient identities.

The generated track must correspond to the current **committed** authored inputs.
Require `visible_revision == revision`, no generation error, no visible draft
preview, supported physical-reference conversion and a valid associated train.
Recheck the complete source key when a preparation result returns. A late
generation or simulation reply cannot reactivate a retired epoch.

## Preserve Editor state

Keep the existing Session/Editor rather than reloading a file on return. Preserve:

- Project identity/path, dirty comparison and unsaved committed changes.
- Undo/redo stacks and transactional command boundaries.
- Selection, active section/channel/region, graph pan/zoom/readouts and timeline.
- Docking tree, Track/Train workspace, floating panes and display preferences.
- Editor navigation/camera, independently of simulator camera state.
- Drafts and pending work, with explicit readiness rules; no implicit Apply/Cancel.

Proposed minimal M4K draft rule: use existing human-directed Apply/Cancel guards
before entry if a draft is pending. Unsaved committed edits do **not** require
saving. Pending generation keeps the application in Editor with preparation
feedback; it must never consume the last older visible geometry. Whether the
owner prefers a disabled command or an entry prompt remains a UX decision.

## Transitions and failure behavior

| Trigger | Proposed transition / invariant |
| --- | --- |
| Editor → Simulator | Capture editor navigation and source key, prepare on worker, remain in Editor/Preparing until success; publish mode, initial poses and simulator camera atomically |
| Cancel preparation | Retire ticket/epoch; discard any late result; preserve document, editor camera and prior accepted scene |
| Initialization failure | Stay/return in Editor, show the mechanism/track/speed error; retain authored state and history; never enter a partly initialized Simulator |
| Play | Schedule real production steps; show running only after worker acknowledgement |
| Pause | Stop scheduling, consume final accepted snapshot and hold it; attached cameras/meshes latch the same pose |
| Reset | Pause and restore the successful initial world, hints, presentation pair and telemetry exactly; keep the same source key; do not re-seat from accumulated state |
| Simulator → Editor | Pause/retire simulation epoch, restore editor workspace/navigation; preserve dirty project and all history |
| Edit then re-enter | Prepare from the new committed revision; discard incompatible previous simulation state; never reuse stale track/assembly |
| Open/close/new project | Existing save/discard/cancel guards run first; successful project replacement retires pending and running simulation workers; cancellation preserves the current project/mode |
| Worker/dynamics failure | Hold last accepted poses, pause, display actionable error; reset allowed if the initial snapshot remains valid, return-to-editor always available |
| Visual asset replacement | In future, update presentation resources without changing physics; initially a conservative session rebuild is acceptable |

Minimum M4K policy: retire/clear the active simulation on return to Editor.
Preserve simulator camera preferences separately, but prepare a fresh simulation
on re-entry. Reusing a paused session when its full source key is identical is a
later optimization, not required for seamless document preservation. Retired
workers must not block egui: add explicit asynchronous retirement/acknowledgement
and eventual joining rather than an unbounded UI-thread join.

Existing `train_playback` source checks and load rollback are reusable, but its
`State` is currently editor-owned. Extract a shared session/controller interface
before making two UI compositions call it. Avoid two independent workers, duplicate
simulation clocks or two sets of engine state selected by mode.

## Timing and telemetry

Retain 1/240 s outer physics dt. Render interpolation is presentation-only and
must never change authoritative body state. Keep bounded buffering and explicit
starvation/lag reporting. A simulation-speed control changes scheduling of fixed
steps; it never increases dt or prescribes vehicle position. Minimal M4K may
support 1x only until pause/speed/catch-up transitions are tested.

Use the worker's existing `TrainTelemetry` for SI speed, acceleration and local
specific force. Define which body/rider frame a displayed g value refers to:
current mass-weighted train telemetry is not a rider-seat accelerometer. Likewise
M4J's lead-guide start-speed contract is not the COM mean-speed display. Future
rider telemetry must derive translational/angular acceleration at the actual
seat offset. Do not display authored design loads as measured simulation loads.

Keep playback/session errors separate from authored validation and generation
errors. Engineering overlays may expose joint/guide reactions and residuals,
with units and simplified-model labeling, without obscuring normal playback.

## Camera architecture

Proposed camera controller takes a presentation packet and user camera intent,
and returns `CameraView`. It owns no physics and applies no body impulses.

| Camera | Source and behavior |
| --- | --- |
| Editor | Existing renderer inspection navigation, restored exactly on return |
| Simulator free | Independent inspection navigation initially; six-axis free-fly needs explicit input/focus/shortcut design |
| Rider seat | Stable train/component/seat identity plus local eye/forward/up transform composed with the actual simulated passenger/body pose |
| Front/rear train | Stable chosen physical component; offsets from its presentation pose, not sampled track station |
| Train follow/chase | Actual physical reference pose with explicit world/body offset policy; optional filtering is presentation-only |
| Fixed trackside | World eye/orientation, optionally aimed at a simulated target; no track attachment constraint |

One presentation time and pose sample per frame must feed both meshes and
attached cameras. Preserve vehicle roll and rider orientation by default;
stabilized views must be explicit and visibly selected. Missing components or
invalid poses cause a clear fallback to free camera, not a jump to geometric
animation. M4K needs only free inspection and one existing Rider or Chase view.
Actual seat definitions, selection UX, look controls and comfort filtering need
owner design; an arbitrary camera offset is not a finalized rider-seat catalog.

## UI outline — proposed, awaiting owner design

Use the existing native wgpu viewport and egui shell. Proposed mode commands are
an easy-to-find **Simulator** entry in Editor and **Return to Editor** in Simulator,
with a persistent clear mode indicator. Exact placement, visual treatment and
shortcuts require an owner sketch/decision. Do not repurpose the current Track /
Train workspace tabs as application modes or reset their saved layout.

Simulator composition should reserve most space for the viewport, with a compact
play/pause/reset strip, camera choice, associated-train identity and speed/local
load readout. Train selection appears only for actually supported associations;
current projects hold one coaster and physical playback resolves one associated
train. No invented ride manager. Optional diagnostics belong in a disclosure or
overlay. Omit graph/properties/support authoring panels in this composition while
preserving their Editor state. Every visible control must perform a real action.

Existing playback shortcuts can be retained where appropriate (Ctrl+Space and
Ctrl+Shift+Space); resolve mode-switch/camera focus conflicts with the owner.
Respect keyboard focus and error readability. A full redesign, train catalog,
walk controls and collection of exposed debug panels are outside minimum M4K.

## Shared versus separate resource lifetimes

- Share window/surface/device/queues, pipelines, immutable scene geometry, licensed
  assets, GPU mesh/material caches and document/generation services.
- Separate editor navigation/layout state from simulator camera/playback state.
  Save navigation/projection preferences in the mode camera bank, not derived
  `asset_instances` or an old physical pose packet. Rebuild those presentation
  fields from the active session when switching modes.
- Simulation owns its frozen physical reference, World/reset snapshot, worker,
  bounded telemetry/presentation buffers and error lifecycle.
- Render instances are derived from physical component-origin poses; do not
  duplicate authored train definitions inside renderer resources.
- On project close, release retired snapshots/assets when their Arc references
  drain. Cancel generation/preparation by identity and ignore stale publications.
- Surface loss/resize/device recovery remains `qx-rendering`'s responsibility;
  losing a GPU surface must not silently reset physics or modify the document.

## Future audio and environments

Audio extension: consume accepted simulation timestamps, component poses, speed,
device events and future contact data through bounded event/telemetry adapters.
Train body, wheel/track and wind emitters need licensed/original sound assets and
documented relationships to measured state. A listener follows the selected camera
presentation. Pause/reset/exit must pause, seek/reset or retire playback explicitly.
Backend/device acquisition and spatial mixing belong behind `qx-audio`; none exist
yet. Audio must never drive physics or block the physics/render threads.

Environment extension: separate rendering settings (sky, exposure, time-of-day
lighting), authored terrain/scenery/biome assets, and deterministic physical
environment parameters (wind, air density, temperatures and documented resistance
laws). Weather presentation is not evidence of weather-dependent mechanics.
Current physical settings already include gravity, air density, drag area and
rolling resistance, but no wind, temperature or weather model. Future changes need
validation and their own persistence design; introduce no speculative schema now.

## Minimum viable M4K and verification

1. Obtain owner direction for entry/return placement, compact Simulator composition,
   camera selection and draft-entry feedback.
2. Extract a transient session/controller from editor playback. Add explicit mode
   state and source-key preparation with stale/cancel/error tests.
3. Preserve the live Editor/Session and renderer editor navigation. Add a separate
   simulator navigation bank and composition using the same scene/device.
4. Enter paused with the associated supported train; offer play/pause/reset,
   free inspection plus one attached camera, clear model limitations and real speed.
5. Return to Editor without changing file, dirty state, undo/redo, selection,
   timeline/graph or layout. Edit and re-enter from fresh geometry.
6. Test repeated transitions, unsaved edits, undo/redo, generation races, draft
   rejection, failed seating/velocity initialization, runtime failure, stale
   worker messages, pause/reset, project replacement and close cancellation.
7. Use the native noninteractive wgpu/egui workflow for deterministic evidence;
   separately obtain owner desktop testing for focus, navigation, DPI, resize,
   surface recovery and interaction feel. Measure initialization and steady
   stepping/rendering separately on supported scenes.

Dependencies/risks: M4J is an opt-in, bounded initializer, not default adoption.
Keep its developer policy explicit until further qualification and owner policy
approval. Production remains an experimental ideal-captive-guide rigid-body model;
no wheel/rail contact, frictional suspension, full ride operations or safety claim.
Long/ill-conditioned mechanisms may exceed the bounded solve; 16-car stepping
already needs performance attention. Mode architecture must expose such limits
honestly rather than hiding them behind geometric playback or a second backend.

M4K acceptance is the native author → simulate → return → edit → re-simulate loop
with preserved project state. Audio, environment, full operations, every camera
and broader train qualification are subsequent capabilities.
