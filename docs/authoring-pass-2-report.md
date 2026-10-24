# Integrated development pass 2 — completion and verification

Completed locally on 2026-10-24 on `codex/integrated-authoring-pass2`, based on
Pass 1 commit `b2fdce9`. The central authoring slice is implemented: all four
spline representations generate actual ordered track sections alongside the
existing graph/timeline engine, support transactional numerical editing and
persistence, and feed shared numerical exports and a native development inspector.

## Functionality and integration

| Area | Implemented behavior |
| --- | --- |
| `qx-geometry::curve` | Cubic connected Bezier, degree-1..8 B-spline, weighted rational NURBS, and uniform/chordal/centripetal Catmull-Rom; f64 position, analytic first/second derivatives, tangent, curvature, native domains, validation and one-sided G/C diagnostics. |
| `qx-geometry::arc_length` | Adaptive, budgeted Simpson speed quadrature, error estimates, monotonic compensated lookup tables, bracketed inversion, and actual-curve distance queries. |
| `qx-geometry::curve_edit` | Stable-ID point/handle edits, insert/remove, weights, knots, degree with explicit knots, Catmull-Rom parameterization, exact supported conversion to NURBS, shape-preserving knot insertion, explicit start alignment and Bezier G1/C1/C2 fits. |
| `authoring`, `spline_track`, `solver` | Section-local inherited placement, normalized distance roll channels, transported orientation, shared section ranges, measurements, continuity and quadrature diagnostics. Existing force/geometric solver equations are preserved. |
| `qx-document` | `EditSpline` and explicit `ValidateConnections` commands in existing transactions/history; version 2 preserves all new authored data; explicit strict version-1 migration; recoverable save/load. |
| `qx-rendering` | Native winit/wgpu window, perspective/top/side/front and orthographic views, orbit/pan/zoom, fit, grid/axes, boundary crosses, selected-section feedback, frames and scaled curvature diagnostics. f64 scene rebasing precedes GPU conversion. Worker-thread reload retains the previous scene on failure. |
| `apps/quantum-x` | Integrated `examples`, `inspect`, `move-point`, and `benchmark` commands alongside `demo`/`generate`; CSV/OBJ/SVG exports, migration/save/reopen/regeneration checks and refinement comparisons. |
| Fixtures/tools/docs | Three checked-in version-2 examples, unchanged version-1 foundation, independent standard-library CSV/XML/reference verifier, updated README and mathematics/API guide. |

No new crate was introduced. Physics, audio, catalog data and existing UX
specifications were not changed. `crates/qx-style` remains independent and style
selection continues to leave centerline geometry unchanged.

Run and API details, conventions, algorithms and limitations are documented in
[authoring-pass-2.md](authoring-pass-2.md). A convenient starting command is:

```powershell
cargo run -p quantum-x -- inspect examples/mixed-authoring.qx.json
```

The editor remains graph/timeline-first; the inspector is a temporary engineering
surface. `H` prints its controls, `F` fits, `Tab` selects, `1`..`4` change views,
`N`/`K` toggle diagnostics and `R` reloads. Precise edits use shared Rust document
commands or `move-point`; the inspector does not pretend to implement graphical
spline editing or the final editor workspace.

## Checks executed

Native Windows, rustc/Cargo 1.99.1:

```text
cargo fmt --all -- --check                           PASS
cargo check --workspace                             PASS
cargo clippy --workspace --all-targets -- -D warnings PASS
cargo test --workspace                              PASS
cargo build --workspace --release                   PASS
git diff --check                                    PASS
```

The final full workspace test run uses `CARGO_TARGET_DIR=target/pass2-validation`
so rebuilding the CLI does not replace the open native Debug inspector. The
workspace has 46 passing tests, including the unchanged audio/physics placeholder
tests. Coverage includes independent Cox-de Boor basis comparison, analytic
derivatives and central differences, rational circles, knot insertion, repeated
knots and side limits, all Catmull-Rom parameterizations, arc-length inversion near
zero speed, quadrature refinement/budget failure, invalid authoring inputs,
Bezier continuity fitting, near-reversed alignment, spline loops through vertical,
inflection frames, normalized-roll length edits, mixed boundaries, command rollback,
undo/redo, insertion/removal/degree/conversion edits, strict migration, existing
Pass 1 cases and actual executable export/edit/reopen workflows.

Additional execution, separate from Rust tests:

```powershell
.\target\release\quantum-x.exe examples target/pass2-final
.\target\release\quantum-x.exe generate examples/foundation.qx.json target/pass2-foundation-release 0.1
.\target\release\quantum-x.exe benchmark
python tools/verify-authoring-exports.py target/pass2-final
```

The independent verifier parses all nine new-example SVGs as XML, checks finite
CSV fields, strict distance ordering, frame orthogonality and norm errors, method
identities, and analytic reference length/positions/curvature. It does not claim
manual visual review of every SVG. All examples save/reopen and regenerate with
identical complete generated output on this machine/build.

## Numerical results

Default spatial step 0.1 m, default spline quadrature tolerances:

| Example | Sections | Samples | Length (m) | Maximum 0.1/0.05 m position difference (m) |
| --- | ---: | ---: | ---: | ---: |
| Unchanged foundation | 5 | 1521 | 151.999999999996 | 9.788293345539e-11 |
| Spline comparison | 4 | 1524 | 151.81213887273634 | 1.287876166723e-4 |
| Mixed authoring | 10 | 3644 | 363.81213887275896 | 2.724938410759e-4 |
| Analytic reference | 4 | 1117 | 111.41592653591022 | 4.831338371898e-9 |

The refinement differences are measurements, not error bounds. Spatial Bishop
transport is an approximation, and its error carries into downstream sections.
That explains why the spatial mixed examples have larger refinement differences
than the planar analytic fixture; the 1e-8 m arc quadrature tolerance is not a bound
on transported orientation or accumulated downstream position error.

Independent exported-reference checks measured:

- `80 + 10*pi` m total-length error: **1.2278178473934531e-11 m**.
- Radius-20 m circle sampled-position error: **4.9338866199459144e-9 m**.
- Circle finite-difference curvature error: **5.427642863625737e-8 /m**.
- Maximum squared frame-axis norm error across new examples: **2.220446049250313e-15**.
- Maximum pairwise frame-axis dot magnitude: **9.159339953157541e-16**.

Separate Rust analytic tests use a radius-10 m rational quarter circle, asserting
length error below 1e-8 m, curvature error below 1e-12 /m, and distance-inverted
position error below 3e-8 m. Tighter quadrature is tested by refinement. These are
specific test tolerances, not universal guarantees for every accepted curve.

Pass 1's endpoint remains `(114.04327140833351, 22.251725311225908,
75.46754437004525)` m, with 1521 samples and three intentional section-rate warnings;
an explicit regression assertion protects those results. Version-1 loading changes
the in-memory schema version without reinterpreting original sections. New saves
emit version 2, so old Pass 1 readers cannot load them; no downgrade is claimed.

## Measured cost and memory considerations

Single Release runs on an Intel Core i7-14700KF, including validation and all
curve/frame/measurement generation, with 0.25 m maximum spatial spacing:

| Cubic B-spline controls | Length | Samples | Generation | Sample + graph struct payload |
| ---: | ---: | ---: | ---: | ---: |
| 64 | 1000.098990 m | 4031 | 10.457 ms | 1,450,872 bytes |
| 1024 | 10001.089245 m | 40917 | 107.478 ms | 14,729,832 bytes |

These are single observations, not statistical benchmarks or frame-rate promises.
Payload excludes vector spare capacity, preparation/table allocations, strings,
authored data/history, allocator overhead, exports and GPU storage. Full regeneration
is still the baseline; no persistent incremental cache was added. Camera motion
only updates a uniform buffer; new loads or changed display diagnostics rebuild
the line buffer. Fixed spatial steps and angular guards may reject pathological
curvature/roll; there is no adaptive frame-error controller or global cusp proof.

## Native runtime verification

An actual Windows Debug inspector was launched on the mixed-authoring project and
visually inspected through the computer-use skill. Adapter output identified
**NVIDIA GeForce RTX 4070, Vulkan, NVIDIA driver 617.42**. The test displayed the
actual generated centerline with shared section boundaries and verified:

- Perspective and top/side/front orthographic projections.
- Pointer orbit and keyboard pan.
- Orientation triads and scaled curvature overlays.
- Section cycling, selected-section name/method feedback and selected/all fitting.
- Numeric point edit through the CLI, save, `R` reload and changed scene/length.
- Invalid JSON reload, actionable error feedback, previous geometry retained,
  then successful reload after restoring the valid file.

The native Debug window was left open for inspection. Subsequent mouse-wheel and
resize checks encountered the computer-use input guard and were not completed.
Wheel zoom, pointer pan, resizing/minimization, alternate DPI settings, file-drop
loading, DX12, adapter absence, surface/device-loss paths, other hardware/platforms,
and sustained performance were not manually verified. The final Release executable
was built and its numerical/export workflows were run; interactive checks were not
repeated against Release. Compilation is not presented as runtime verification of
those unchecked paths. No FPS or physical-rail rendering claim is made.

## Dependencies, UX status, limitations and next work

Rendering adds wgpu 30.0.1, winit 0.30.13, pollster 0.4.0, bytemuck 1.25.2 and glam
0.30.10 for graphics, windowing, startup async bridging, POD buffers and matrices.
Exact versions and transitive dependencies are Cargo-locked. Cargo metadata license
expressions were inspected; the direct additions offer MIT, Apache-2.0 and/or Zlib
terms. Transitive expressions include BSD, ISC, Unicode, Unlicense and an optional
LGPL alternative with permissive alternatives. This is a metadata audit, not a
claim that a legal review of every distribution obligation was performed. No GPL
reference code, proprietary assets, external service, telemetry or train physics
was added.

Owner-approved behavior followed: the existing graph/section engine remains primary,
additional authoring methods share it, mathematical editing is independent of UI,
and the brief's native inspector is explicitly development-only. No final production
UX is introduced. `docs/ux/coaster-properties.md` is preserved. Other referenced
Inkscape/editor/welcome designs were absent from this checkout; final layout,
graph editing interactions and production spline controls still require human design.

Remaining technical limits are documented in the guide: restricted exact conversions,
degree redefinition rather than degree elevation, no generic cross-method G2 fitting
or endpoint/loop-closure solver, no disconnected/tangent-jump track sections,
non-certified quadrature and sampled regularity checks, approximate frame transport,
no cross-edit generation cache, no automatic device-loss recovery, and centerlines
rather than rail surfaces. The production editor and rigid-body train simulation
remain separate future work. None of these is represented as implemented.

Recommended next pass: implement the owner's approved graph/timeline interaction
design using these commands and derived outputs, with adaptive orientation-error
control and measured incremental regeneration where useful; develop physical rail
contact geometry and validated rigid-body dynamics in their own dedicated pass.

Git: the working tree was initially clean on `codex/integrated-core-foundation`;
local `main` was behind Pass 1, so the new branch was based on the completed
foundation rather than discarding it. Changes are local and uncommitted on
`codex/integrated-authoring-pass2`. No push, PR, merge, release or publication occurred.
The original foundation fixture, licenses, AGENTS.md and human UX file are unchanged.

## Git handoff verification — 2026-10-24

Before continuing Pass 3, the completed Pass 2 working tree was backed up outside
the repository, including all modified/untracked source files, a binary Git patch,
and a Git history bundle. No Pass 3 implementation changes existed at this point.
The owner's subsequent AGENTS.md edits are preserved in their own commit.

Fresh native Windows runs passed format, workspace check, Clippy with warnings
denied, all 46 workspace tests, and the Release workspace build. Release `examples
target/pass2-handoff` and `python tools/verify-authoring-exports.py
target/pass2-handoff` passed, reproducing the numerical results above. No desktop
interaction was performed during this handoff; earlier interactive observations
above are historical Pass 2 results, not newly repeated verification.

The Pass 2 feature branch targets `main`, whose fetched history already contains
the Pass 1 merge. The owner explicitly authorized committing, pushing, creating
pull requests and merging verified passes. Pass 3 will continue from that merged
baseline on its separate branch. No reset, force push, or branch deletion is used.
