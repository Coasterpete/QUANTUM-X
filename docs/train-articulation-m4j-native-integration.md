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
