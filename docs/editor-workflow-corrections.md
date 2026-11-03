# Editor workflow corrections — 2026-11-01

Both requested editor corrections work in the native Release application.
All changes remain local and uncommitted on `codex/m1-track-pbr-materials`.
OpenCode's physics/train files match the hashes taken when this task resumed.

**Fixed.** Acceleration profiles use the existing Transition Editor, with a
separate device data domain, local metres and commanded m/s². The actual drag
failure was native input batching: press and movement could arrive together,
leaving the pointer outside the small handle before egui assigned ownership.
The shared gesture code now captures the clipped, unobstructed press origin and
applies that frame's movement. Endpoints, shape handles and numeric controls
edit the same validated draft. Apply/Cancel, history and persistence retain the
authoritative channel, including easing/NURBS functions and nonnegative bounds.

Train inspection previously rejected rider-reference geometry even though the
construction/support path already derived its physical centerline. The scene
now shares that derived reference with train placement, framing and camera
attachments. Authored geometry stays authoritative. Gray components render
without GLBs; inspection does not initialize a physical simulation.

**Actually tested natively.** Opened the integrated verification copy, selected
Launch 1 and opened its bottom graph. Dragged an endpoint from 2.0 to 2.66136
m/s², observed the matching number, and canceled back to 2.0. Dragged the start
endpoint and a cubic NURBS shape handle; applied, undid, redid, saved and reopened
the resulting curve. Experimental playback used it: the first step reported
+2.987 m/s² and 7,165 N for Launch 1, then the command changed with distance.

Opened both the integrated rider-reference reproduction and the mixed-authoring
project. Saw all three gray cars, used Fit Train and Focus Selected Car, and
scrubbed inspection stations. Rider POV and Chase Camera followed the train
during geometric preview playback. The mixed project displayed the train at
10 m on its compatible lead-in after explicit association; no geometry was
changed. Original projects were preserved; native saves went to evidence copies.

**Checks.** Passed: two workflow regressions; all 19 existing graph-drag tests;
the device-force command regression; whole-curve document validation regression;
scoped rustfmt; `git diff --check`; and `cargo build --release -p quantum-x`.
Logs: `target/corrections-resume-{focused,graph,forces,validation,release}.log`.
Workspace fmt and Clippy were run but fail in untouched physics changes
(`assembly.rs`, including `collapsible_if` at line 347). The workspace test suite
was not rerun; focused checks were used as requested.

**Limits / UX.** Existing mechanical closure failures still stop inspection with
specific residuals and recovery advice (including the integrated connector near
124.26 m and curved portions of the mixed fixture). Experimental physical
playback still exhibits the separate roll defect; it was observed, not repaired.
Native insert/remove, graph zoom/pan and fine-adjustment combinations were not
retested individually; shared graph regressions passed. This implements the
owner-requested workflow within the existing UI. No new provisional UI or
unresolved design decisions were introduced; existing development controls stay
labeled. Materials, construction, assets, layout, colors and fonts were retained.

**Executable:**
`target/distribution/QUANTUM Dynamics-workflow-corrections-20261101/QUANTUM Dynamics.exe`
(matches the Release binary, SHA-256
`84672bc171e83d131d25c3e90b11242ee0a2e30bce878f26eb4be33b4bd5e7db`).

**Authentic evidence:** [before graph](../target/workflow-corrections-evidence/before-acceleration.png),
[after shaping](../target/workflow-corrections-evidence/after-acceleration-shaped.png),
[before missing train](../target/workflow-corrections-evidence/before-missing-train.png),
[after train](../target/workflow-corrections-evidence/after-rider-reference-train.png),
[Chase Camera](../target/workflow-corrections-evidence/after-chase-camera.png),
[mixed project](../target/workflow-corrections-evidence/after-mixed-authoring-train.png),
[launch force](../target/workflow-corrections-evidence/after-launch-force-command.png).
Short native clips: [drag](../target/workflow-corrections-evidence/native-acceleration-drag.mp4)
and [Rider POV inspection](../target/workflow-corrections-evidence/native-train-inspection.mp4).
These use timestamped Windows Graphics Capture frames with no generated
intermediate images. Earlier GDI recording attempts captured stale swapchain
content and are excluded from acceptance evidence.
