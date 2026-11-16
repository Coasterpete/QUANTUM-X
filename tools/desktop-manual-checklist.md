# R0.5 manual owner checklist

NOT PERFORMED by verify-desktop.ps1. This is separate from automated report.json.
Record actual results in manual-checklist.json, including the machine label,
application source commit, UTC test time, failures and untested combinations.
Use passed / failed / not_performed / not_applicable. Silence means untested.
Test the laptop and desktop separately; one machine's result never covers both.

Use a new disposable projects folder OUTSIDE the extracted package. Save only
new test projects or copies there. Do not overwrite existing projects or assets.
Launch the public `QUANTUM Dynamics.exe` from the extracted package in Explorer.
The existing console and unsigned-build warnings are known limitations; R0.5
does not implement R1 console/error handling or a separate Simulator window.

| ID | Owner action and observation | Initial result |
| --- | --- | --- |
| welcome | Inspect Welcome text, icons, contrast and clipping against the owner's design. Check New/Open/Library routes. Record console/startup warnings. | not_performed |
| editor | Create a disposable project. Select sections, navigate graphs and viewport, enter a numeric edit, Apply/Cancel. Check pointer/keyboard feedback and responsiveness. Record stalls; do not infer FPS. | not_performed |
| save-open-undo-redo | Make an observable edit and Apply; Undo and Redo, checking values and geometry. Save in the disposable folder; close/reopen that copy and compare. Check unsaved-change prompts without risking original work. | not_performed |
| simulator | Use a disposable copy with a compatible development train; enable existing View > Development tools if needed. Enter the current Simulator mode, Play, Pause (confirm motion stops), Reset, Return; confirm the Editor document/state is preserved. Record blocked entry or unsupported setup. | not_performed |
| resize-minimize-restore | On Welcome, Editor and Simulator, resize small/large, maximize, minimize and restore. Check redraw, layout, focus, input and continued operation. | not_performed |
| dpi-scaling | Record Windows version, resolution and actual tested scaling (100/150/200% where available). Check labels, clipping, viewport, dialogs and pointer alignment after scale changes/relaunch. Leave untried scales explicitly untested. | not_performed |
| multi-monitor | If available, record monitor count, resolutions, refresh rates and scales without serial numbers. Move the window between monitors, maximize/restore and check focus, input alignment and redraw. Mark unavailable setups not_applicable; do not claim coverage. | not_performed |
| rendering-shutdown | Throughout these checks watch for black/blank frames, corruption, missing geometry, flicker and device errors. Close normally and relaunch; record unexpected shutdowns and steps to reproduce. Attach only reviewed, bounded logs/cropped captures with no private data. | not_performed |

Existing native smokes use hidden windows and a synthetic development fixture.
They do not click these controls or certify visual quality, physical realism,
real-time performance, long rides, GPU recovery or cross-machine determinism.
Do not induce a device/driver reset with unsaved work. Recovery/installer checks
belong to later roadmap milestones.
