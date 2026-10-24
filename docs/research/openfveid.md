# openFVeiD functional research for foundation pass 1

Reviewed on 2026-10-24 against the upstream `main` source at commit
`4d797e3ae9fb23a3997464f429c87eec377b9ca6`:
[H27CK/openFVeiD](https://github.com/H27CK/openFVeiD/tree/4d797e3ae9fb23a3997464f429c87eec377b9ca6).
The source was retrieved into a temporary directory outside QUANTUM-X. No source
code, GUI assets, or translated implementation was incorporated into this project.
Upstream identifies its license as GPL-3.0. QUANTUM-X's existing licenses remain.

| Source inspected | Functional observation | QUANTUM-X decision |
| --- | --- | --- |
| `core/section.h` | Ordered sections distinguish straight, curved, forced, geometric, and rider-local types; sections own channels and generated nodes. | Persist tagged section types and stable IDs; keep generated output separate from authoring. |
| `core/secforced.cpp` | Force channels and roll changes influence successive rider frames; time and distance evaluation have separate paths. | Use a single spatial ODE with explicit domain conversion and constant reference speed per section. Derive curvature from the projected force equation. |
| `core/secgeometric.cpp` | Geometric pitch/yaw use world orientation and include corrections for banking and near-vertical behavior. | Define gravity-referenced pitch and world yaw mathematically. Reject the world-geometric singularity near vertical; do not claim identical upstream behavior. |
| `core/secgeometricriderlocal.cpp` | Pitch/yaw controls rotate around rider-relative frame axes. | Use rider normal/lateral curvature components with explicit signs; quaternion transport avoids Euler-frame flips. |
| `core/function.cpp` | Channels contain ordered sub-functions; insertion, removal, and length changes move subsequent input boundaries. | Store transition lengths and stable IDs. Boundaries are prefix sums; edits validate transactionally without silently translating other values. |
| `core/subfunction.cpp` | Transition families, endpoint changes, amplitude-like parameters, and additional warping behaviors shape channels. | Implement the requested standard mathematical families with analytic derivatives and an explicitly defined amplitude bump. No custom expressions, locked transitions, time warp, or equivalence claim. |
| `core/track.cpp` | Updating one section propagates from its final node into subsequent sections. | Carry the integrated pose exactly into the next section and expose shared boundary sample indices. Regenerate fully for now. |
| `ui/graphview.cpp` | Graphs distinguish authored channels, resulting measurements, section boundaries, and diagnostic regions; both distance and time axes exist. | Produce separate authored/derived numerical columns and developer SVGs, without implementing or reproducing a graph GUI. |
| `ui/transitionview.cpp` | Editing includes function type, length, change, insertion/removal, and domain-dependent units. | Public transition data and document commands provide those operations for a future human-designed interface. |

These observations informed domain responsibilities, not source-level structure.
QUANTUM-X's solver is independently specified in `docs/core-foundation.md` using
standard moving-frame kinematics, quaternion differentiation, and RK4.

## Deliberate assumptions and differences

The design-speed assumption, channel amplitude definition, force signs, roll
meaning, world-geometric equations, boundary policy, persistence, and numerical
integration method are QUANTUM-X definitions. Upstream includes speed/energy,
heartline offsets, smoothing, limits, and advanced transitions beyond this pass.
No upstream coordinate, unit, GUI, or numerical-result compatibility is promised.

No human UX specifications beyond `docs/ux/coaster-properties.md` were present
in this checkout. That file was read and preserved. This pass adds no GUI.
