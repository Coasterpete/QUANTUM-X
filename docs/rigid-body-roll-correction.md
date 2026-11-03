# Conventional rigid-body roll correction

The existing rigid-body engine still integrates independent masses, gyroscopic
angular dynamics and joint/rail impulses at 240 Hz (four internal substeps in
normal train playback). Track queries locate constraints; they never assign
vehicle poses during simulation.

## Mechanical correction

- Retain the partial `RollLock` mechanism, using a consistent swing/twist angle
  and its exact angular Jacobian in assembly, impulses and diagnostics. Pure
  pitch/yaw swings remain free. Opposed longitudinal axes report a singular
  constraint transactionally rather than a false zero residual.
- Reference each guided running gear's roll to its own authored rail frame,
  respecting its mounting rotation and mass-center offset. Include the spatial
  frame derivative in both linear and angular constraint Jacobians: bank-change
  reaction torque has a companion longitudinal force and cannot act as an
  unbalanced motor. Cache frame derivatives once per substep.
- The conventional development arrangement uses one front roll-locating bearing
  and one rear spherical bearing per car. This three-point idealization permits
  different banking at the two bogies without imposing incompatible ideal roll
  locks. Passenger bodies have no direct absolute orientation constraint.
- Replace the development arrangement's coincident rigid hitch extensions with
  independent 1 m, 20 kg drawbars, with spherical bearings at both ends. Hitch
  anchors stay on the running-gear plane. Remove the partial, assembly-only
  coupler-clearance workaround; all positional joints close in both solvers.
- Below-track bodies retain spherical suspension; rotating seats retain their
  independent limited hinges. Existing persisted joint definitions are not
  silently rewritten. Re-create a development fixture to obtain the revised
  topology; custom trains need explicitly appropriate mechanical connections.
- Keep assembly limits at 1e-5 m / 1e-5 rad and the bounded 2400 sweeps. Remove
  temporary environment-variable probes and the scratch debugging test. Twelve
  twisting-track initialization cases, including reversed joint endpoints/order,
  now close with maximum residuals 9.968e-6 m and 4.997e-8 rad.
- The bounded document/world body capacity is 2048, accommodating the existing
  256-car development option with separate drawbars and rotating seats.

## Numerical verification

Reproduce the measurements without any desktop, rendering or capture:

```powershell
cargo test -p qx-physics --test roll_stability --test assembly -- --nocapture
```

Peak absolute roll angles in degrees, measured independently by swing/twist:

| Scenario | Body / gear | Gear / local track | Body / local track |
| --- | ---: | ---: | ---: |
| Stationary, 2 s | <0.000001 | <0.000001 | <0.000001 |
| Banked curve, 2 s | 0.008375 | 0.007999 | 0.008375 |
| Changing pitch/yaw curvature and bank, 3 cars, 2 s | 1.044482 | 0.004464 | 0.523491 |
| Complete vertical loop, 2 cars | <0.000001 | <0.000001 | <0.000001 |
| Former ball mechanism, 0.01 rad perturbation, 4 s | 179.926395 | <0.000001 | 179.926395 |
| Corrected mechanism, identical perturbation, 4 s | 0.315503 | 0.077213 | 0.315503 |

The control reconstructs the former free ball suspension and missing rail-roll
reaction using the same integrator; it is not a replay of OpenCode's recording.
The changing-bank rear-bearing roll is legitimate articulation; it is distinct
from the front roll-lock residual (3.643e-6 rad) and rail residual (7.791e-5 rad).

The two-car loop crosses the upright entry, inverted apex and upright exit in
1005 ticks (4.1875 s). Peak joint gap is 0.04723 mm, guide error 0.2070 mm, total
gear/track orientation error 0.12428 degrees and body/gear pitch/yaw articulation
3.46245 degrees. Peak angular speed is 2.4520 rad/s; relative energy drift is
-0.001323%. A separate unforced changing-bank test measures -0.000010% energy
drift after initial velocity constraint seating. A tighter banked arc measures
-0.343690% over 1 s; stabilization is not an energy-conserving integrator.

An intentional 0.28 rad (16.04 degree), 1.5 rad/s car perturbation violates the
ideal rigid roll bearing. It recovers below 1e-6 rad roll and 1e-5 rad/s by 0.5 s,
but the existing Baumgarte correction produces a 35.49 rad/s initial transient.
This is a numerical recovery test, not a model of a compliant suspension impact.

## Limits and UX

Captive bilateral centerline guides and ideal rail-roll reactions remain
simplifications, not individual wheel/rail contact, suspension compliance or
manufacturer-validated mechanics. Incompatible custom constraint graphs and
open-end escapes still fail explicitly. The fixed-body assembly test suite also
covers offset pivots, arbitrary IDs, shared gear and impossible guides.

No UI workflows, themes, authoring geometry, cameras, train visibility, supports
or PBR rendering were changed. Existing provisional mechanics controls remain
provisional. Native visual behavior and desktop interaction are **NOT MANUALLY
VERIFIED**; no GUI, browser automation, screenshots or recordings were used.

## Checks executed (native Windows x86_64 MSVC)

- `cargo test -p qx-physics --tests`: 52 passed.
- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 159 editor tests passed; two assertions expected the
  old component counts. Both were updated to verify every independent component
  has a presentation instance, then passed individually with their exact names.
  The unrelated long graph-gesture test passed on the first run and was not repeated.
- `cargo test --workspace --exclude quantum-x`: passed (all remaining workspace
  unit, integration and doc tests, including the physics suite).
- `cargo test -p quantum-x --test headless`: all four CLI workflows passed.
- `cargo build --release -p quantum-x`: passed; one optimized build, Windows
  executable at `target/release/quantum-x.exe`. The application was not launched
  for interactive verification.
- `git diff --check`: passed. No commits, pushes, merges or publication.

The debug builds emitted filesystem hard-link fallback warnings; these were not
Clippy diagnostics. Existing unrelated working-tree changes were retained.

Owner-only manual check, not executed: from the repository root run
`.\target\release\quantum-x.exe`, use the existing development controls to
create a revised conventional consist, and check train/coupler visibility and
attached cameras while simulating an owner-authored hill, bank and inversion.
