# M6 native graph handle dragging correction

This is a correction to the owner-directed M6 interaction, on the existing
`codex/integrated-authoring-pass3` working tree. The pre-existing uncommitted
M4–M6 work is retained. No branch change, commit, push, PR, desktop control, or
interactive application launch is part of this pass.

## Recording and reproduced cause

The supplied `quantum-x_U5LWX6lPzr.mp4` is 124.33 seconds at 30 fps. Frame-based
review covered the complete duration, with closer inspection of 30–45 seconds.
It shows the initial straight, the force section and adjacent graph targets,
temporary profile changes and returns to Committed, pane expansion/redocking,
and further Roll Rate and Normal Force attempts through the end. The recording
alone cannot establish individual button events; the input lifecycle was then
reproduced using complete egui event sequences against the real workspace.

The decisive defect was in `graph_canvas.rs`: egui 0.37's `Response::drag_delta()`
is movement **since the previous frame**, but both value and boundary code added
it to the **original pre-gesture section** every frame. A second move therefore
replaced the first displacement, and an idle frame with zero delta restored the
original values. Release could consequently become a no-op and leave the graph
Committed. This matches the temporary changes in the 30–45 second interval.
An exact multi-move/idle regression failed on the original implementation before
the correction and passes with the captured gesture implementation.

Related weaknesses were also addressed:

- Gesture state had no section/channel/transition/endpoint identity. Acquisition
  waited for the drag threshold, and update/finish depended on individual handle
  responses being rendered. The release frame's final movement was not applied.
- Outer endpoint squares were partly clipped, hit areas were clipped around
  mathematical endpoints, and the diamond ruler was too close to the pane edge.
- Content minimum widths could push the right endpoint beyond a narrow side
  dock's clip rectangle. The graph now uses its actual visible pane width.
- Generation progress changed dock height. Hover/active feedback must not add
  allocated content that changes floating-window sizing. Readouts use a reserved
  status line so they cannot obscure handles in short graphs; status freezes its height during
  capture. The contextual header wraps in narrow panes.
- Alternation between moved and original drafts requested repeated generations.
  Additionally, acceptance regenerated identical preview inputs just to change
  their preview/committed label.

## Pointer and transaction lifecycle

`authoring/graph_gesture.rs` owns the gesture. A primary press on an egui-owned
handle records stable section, channel and transition IDs, endpoint side or
boundary kind, exact preceding draft, pointer origin, and graph conversion scale.
Click/drag threshold handling retains ordinary click selection; sub-threshold
motion does not edit. Beyond the threshold, displacement is always measured
from the recorded press. Stationary frames cannot rewrite the value. Numeric
focus is surrendered to the graph gesture.

Square handles edit only their indicated endpoint vertically; Constant changes
both equal endpoints. Diamonds transfer domain length between the indicated
neighbors horizontally while preserving total extent, endpoint values and other
channels. Adjacent endpoint hit targets are separated. Targets are 24 logical
points across and the complete outer squares remain visible, with leaders to
the exact mathematical endpoints. Hover changes the cursor/appearance; capture
adds an active outline and an exact target/value/unit readout. An out-of-view
captured square remains indicated at the graph edge.

The workspace updates capture before drawing panes. Motion/release continues
outside the original handle, outside graph bounds, after PointerGone, and even
if the pane stops rendering. The press scale stays fixed across resize. A
release processes its final position first, then finishes once. Escape restores
the exact pre-press draft; held-button motion after Escape cannot acquire a new
handle. Focus loss or missing button capture cancels recoverably.

Windows button events are forwarded to egui before viewport routing. Existing
viewport navigation begins only on a press in its own unobstructed viewport;
crossing into it during a graph drag cannot start an orbit or selection. The
installed winit Windows backend already uses native mouse capture on button
press and releases it on button release. No cursor confinement, desktop input
injection or new OS capture implementation is introduced. Actual native routing
and capture across the application window boundary remain manual checks.

Per-motion channel validation rejects invalid numerical values or boundaries
without replacing the last accepted draft. Invalid targets show a prohibited
cursor, altered handle and a specific textual reason. Moving back into range
recovers within the same gesture. Releasing while invalid restores the whole
preceding draft and creates no history entry. Complete connection/generation
validation runs at the release transaction boundary; strict downstream failures
also restore the preceding draft and preserve document/history. The independent
graph feedback survives background worker status publications.

A changed valid gesture begun with no pre-existing drafts commits exactly one
ReplaceSection transaction. A gesture begun over an existing section, initial,
reference or metadata draft remains staged; explicit Apply commits the section.
Click-only gestures and drags returning to their original value create no undo
entry. Undo/redo uses the existing document history and regeneration service.

## Generation and mathematical scope

The existing coalescing worker still supplies live geometry previews. Unchanged
pointer frames produce no candidate change or generation revision. Queue
deduplication compares the candidate project and GenerationOptions; changing an
identical candidate's preview/committed role reuses either the published result
or the in-flight result. Publication uses the current role and still rejects
stale revision results. A revision counts a changed request, not necessarily a
completed worker computation. Actual new pointer values still request previews.

No transition evaluator, polynomial, amplitude, Hermite slope rule, section
solver, authored layout, curve representation or document schema was changed.
The format remains schema 3. Validation preflight at release is synchronous, as
with existing Apply; very large projects may still incur acceptance latency.
This pass makes no frame-rate or native-comfort certification claim.

## Automated verification

`authoring/graph_drag_tests.rs` sends actual egui primary press, multiple moves,
idle frames and release events through the full workbench. It checks Normal
Force and Roll Rate, all four endpoint sides around a shared join, diamonds,
0.75/1/1.25/1.5/2 UI scale, bottom/side docks and floating panes. Additional tests
cover out-of-graph capture, PointerGone, final movement in the release batch,
Escape, focus loss, pane disappearance, resize, pre-existing drafts, invalid
boundaries and numerical values, strict downstream rejection, all seven
transition functions, live generated geometry, no-op behavior, and undo/redo.

The original fixed-size target regression now asserts the enlarged 24-point
target. The original worker regression now verifies reuse of a published preview
on acceptance instead of waiting for an intentionally redundant new generation.
All existing M4/M5/M6 regressions remain enabled.

Final native Windows verification passed:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo check --workspace` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --workspace` | Pass: 139 tests, zero failures (72 application unit tests, including 14 new graph-drag regressions) |
| `cargo build --workspace --release` | Pass |
| `target/release/quantum-x.exe authoring-scenario target/m6-drag-review/scenario` | Pass: Apply/undo/redo/save/reopen agree; 8 offscreen PNGs generated |
| `python tools/verify-m6-exports.py target/m6-drag-review/scenario` | Pass: schema, numerical profiles/frames and all 8 PNGs checked |

Final Release Normal Force (1440×960) and Roll Rate (960×640) renders also pass
and were visually inspected for the full endpoint squares, separate join targets
and diamond ruler. The offscreen adapter was NVIDIA GeForce RTX 4070 / Vulkan.
These are offscreen checks, not desktop interaction or frame-rate measurements.

Local evidence is under `target/m6-drag-review/`, including
`cargo-test-final.txt`, `cargo-release.txt`, `final-normal.png`, `final-roll.png`
and the scenario exports. The Release executable is `target/release/quantum-x.exe`.

Key modules under `apps/quantum-x/src/authoring/` are `graph_gesture.rs`
(captured transaction), `graph_canvas.rs` (targets/feedback), `graph.rs`
(responsive composition), `session.rs` (preview reuse), `panels.rs`
(workspace/keyboard lifecycle), and `commands.rs` (stable status/readouts).
`graph_drag_tests.rs` contains the new complete input sequences. Existing
`correction_tests.rs` and `tests.rs` retain their coverage with the explicit
target-size and preview-reuse expectations described above.

## Owner-executable Windows reproduction checklist

These desktop checks are **not manually verified by the agent**. Run from
`C:\Dev8\QUANTUM-X`:

```powershell
cargo run --release -p quantum-x -- authoring-scenario target/m6-drag-review/scenario
target\release\quantum-x.exe edit target/m6-drag-review/scenario/project.qx.json
```

Use the generated `.qx.json` filename printed by `authoring-scenario` if it
differs. The opt-in hill is a development fixture, not a proposed ride layout.
An existing project with multiple transitions is also suitable.

1. Select the force section and Normal Force. Hover each square: verify the
   vertical cursor and target label. Press, move upward, pause for at least two
   seconds, move again and release. The value must retain total movement during
   the pause and after release; valid generated geometry must update.
2. Check the two separate squares at a shared transition boundary. Drag each
   individually, including across the other square. Only the indicated endpoint
   changes; selection and the active target do not jump to its neighbor.
3. Drag a diamond horizontally and pause. Only its adjacent durations change;
   their sum and endpoint values stay fixed. Vertical motion does not edit force.
4. Repeat 1–3 for Roll Rate. Check that Normal Force values remain unchanged.
5. Hold a square while leaving the graph into the viewport and another pane,
   then release. Repeat outside the application window. Capture must retain the
   target; no viewport orbit, section selection or pane/tab movement may start.
6. With no prior draft, confirm release changes Committed and one Ctrl+Z restores
   the whole gesture; Ctrl+Y restores it. Clicking or returning to the original
   value must not add an undo step. Save/reopen and confirm the value persists.
7. Stage a numeric/name edit without applying, then drag. Release must retain
   both edits as a draft. Apply once, then verify one section undo/redo. Repeat
   with Escape during the drag: the exact preceding draft must return, and later
   held-button movement/release must not restart or commit it.
8. Drag a diamond past a neighbor until its minimum-duration error appears.
   Check the reason and unchanged last valid draft/geometry. Move back to a
   valid boundary and release successfully. Repeat and release while invalid:
   the preceding draft/document/history must return, with the reason retained.
9. Test 75%, 100%, 125%, 150% and 200% UI scale in Display preferences, with
   Transition Editor at the bottom, docked beside the viewport, enlarged, and
   floating. Outer squares and ruler diamonds must stay visible and grabbable.
10. During a valid drag, stop moving: the value, pane bounds and generation
    revision must stop changing. Release after a preview has completed: the
    identical preview must become committed without another generation revision.
    Repeated actual moves should still give live previews. Alt+Tab during a
    gesture should cancel it recoverably when focus is lost.

## UX and Git status

Implemented behavior follows this correction request and the existing M6
owner-authored graph/draft workflow. No developer-only UI was added. Enlarged
targets, outlines, ruler spacing, narrow-header wrapping and status feedback are
implementation choices for the requested interaction correction, subject to
owner native review. No new product workflow or final visual identity is claimed.
All work remains local and uncommitted on the original branch; nothing is pushed.
