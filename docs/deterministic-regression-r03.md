# R0.3 — Deterministic regression baseline

R0.3 freezes the behavior of the existing production-path solver and project
migrations at `69834bc` (latest `main` when this branch was started). It does not
qualify that solver as production-ready physics. No equations, solver tolerances,
initializer defaults, document schemas, UX or packaging behavior change.

The previously uncommitted [core-engine audit](audits/core-engine-audit-2026-11-15.md)
is preserved separately, without editing its historical findings or evidence.
The scope comes from [R0 in the roadmap](roadmaps/production-readiness-v1.md#r0--verification-infrastructure-p0-first).
Later R0 GPU/desktop verification steps and R1–R10 are outside this pass.

## Physics fixture coverage

Inputs and outputs live in
[`crates/qx-physics/tests/fixtures/replay`](../crates/qx-physics/tests/fixtures/replay).
Each JSON file includes the complete initial conditions, authored sections, train
definition, operations, initial station and selected ticks. The manifest records
the baseline source commit, compiler, target, capture profile, generation settings
and SHA-256 of every fixture. These are synthetic development fixtures, not
owner-authored coaster layouts or manufacturer assets.

| Case | Scope | Stored ticks (after completed outer steps) |
| --- | --- | --- |
| `level` | Three conventional cars, 3 m/s, gravity and captive guidance | 0, 1, 12, 60, 120, 240 |
| `grade` | Same train, 10° downhill, gravity-driven acceleration | 0, 1, 12, 60, 120, 240 |
| `launch` | Three cars, partial device occupancy, 1000 N force cap | 0, 1, 12, 60, 120, 240 |
| `brake_reverse` | Reverse velocity, brake, rolling resistance and drag | 0, 1, 12, 60, 120, 240 |
| `banked_curve` | Three cars, R80 arc, initial bank and varying bank | 0, 1, 12, 60, 120 |
| `rotating_seats` | Three-car rotating-seat mechanism and bounded torque motors on that arc | 0, 1, 12, 60, 120 |
| `circuit_seam` | One car on an R30 circle, wrapped launch and forward seam crossing | 0, 1, 12, 120, 240, 360 |

There are **7 fixtures and 40 stored checkpoints**. The longest run is 1.5 s;
the selected runs total 1560 ticks. Every run uses
`TrackSimulation::with_operations`, its default legacy initializer, sampled
centerline captive guidance, **1/240 s**, **4 substeps** and **32 iterations**.
The harness asserts that configuration. It does not enable source-guidance
research or the opt-in compatible-lead initializer. Existing `devices`,
`vehicles`, `default_velocity_compatibility` and consist replay tests informed
the cases and remain unchanged. Native spline families are covered in the schema
corpus, not in these production-physics replays.

The capture stores named numerical fields for every body: mass, COM world
position, Hamilton local-to-world quaternion (w,x,y,z), linear/angular velocity,
external force and torque, plus snapshot component IDs/stations, guide hints,
joint anchors/errors, train/device telemetry, constraint residual maxima and
per-joint/guide/attitude reaction forces/torques. Units follow field suffixes:
metres, seconds, radians, kg, N and N·m; +Y is up, local +X forward and +Z lateral.
Tick 0 has no step diagnostics. Later diagnostics describe the completed outer
step, not a lifetime maximum. Internal private attitude hints and all intermediate
substep state are not serialized; this is an observable-state regression record,
not a resumable world checkpoint.

## Exact identity and numerical comparisons

The default and CI gate require **exact identity**:

- Tick counts, component IDs, field inventories, solver settings, occupancy and
  force-limit flags match exactly.
- Every recorded f64 matches with `to_bits()`, including signed zero. JSON uses
  the workspace's existing `serde_json/float_roundtrip` setting. NaN/infinity are
  rejected during capture.
- Each checkpoint also matches a stored FNV-1a/64 fingerprint. Its canonical
  encoding is tick as u64 little endian, lexically sorted UTF-8 discrete keys and
  values each terminated by NUL, then sorted scalar keys terminated by NUL and
  f64 IEEE-754 bits as u64 little endian. This is a regression digest, not a
  cryptographic authenticity guarantee. Direct field comparison prevents a hash
  collision from passing the test.
- Two independently generated tracks/initialized simulations must match these
  recorded fields bit for bit **at every tick**. Reset must restore the initial
  record exactly and replay all selected checkpoints exactly.

Debug and Release both compare against the **same** committed golden files.
CI explicitly selects `Exact`; no platform detection, hash refresh or fallback
relaxes a failure. A one-ULP change and changed component identity are deliberately
injected in comparator tests and must be rejected.

`-Comparison Tolerant` is an **opt-in portability diagnostic**, never the CI
gate. It retains exact discrete structure and same-machine repeatability/reset
checks, reports expected/actual hashes, differing scalar counts and the largest
absolute difference with its field name. For the committed-vs-current comparison
only, it uses `abs(a-b) <= A + 1e-10*max(abs(a),abs(b))`, where A is `1e-6` for
force/torque fields (`force_n`/`torque_` in the name) and `1e-10` otherwise, in each
field's units. Acceleration/specific-force fields use `1e-10`. Quaternions
are compared componentwise, preserving quaternion sign; no hidden q/−q
normalization occurs. These are fixed diagnostic bounds, not modified solver
tolerances, ride-validity criteria or an exact-pass verdict. `result.json`
reports `exact_gate_passed: false` in this mode, even if all diagnostic bounds pass.
The reported maximum mixes units; use its named field rather than treating it
as a physical scalar error metric.

## Immutable project compatibility corpus

[`crates/qx-document/tests/fixtures/compatibility`](../crates/qx-document/tests/fixtures/compatibility)
contains **9 inputs**, **9 separately committed schema-7 expected snapshots**,
and a provenance/SHA-256 manifest.

| Schema | Input coverage and provenance |
| --- | --- |
| 1 | Actual saved `foundation.qx.json` from `b2fdce9`: force/world/rider-local sections, distance/time domains, section/transition IDs and style |
| 2 | Actual saved `mixed-authoring.qx.json` from `6b8b2ca`: graph sections plus distinct Bézier, B-spline, NURBS and Catmull–Rom data, knots, weights, control IDs, normalized roll |
| 3 | Actual saved `heartline-reference.qx.json` from `497980d`: offset and nominal train-reference association |
| 4 | Reconstructed from existing migration-test patterns: nondefault construction/materials, offset and precision position fields |
| 5 | Two reconstructed variants: three-car mechanics plus an early asset without `pivot_m`; and legacy Hermite regions with nonzero amplitudes/slopes, IDs 41/42 and an explicit jump |
| 6 | Reconstructed scalar NURBS regions, discontinuity and train/construction data |
| 7 | Reconstructed integrated project: Shuttle intent, launch keys, brake transition curve, resistance, train, construction and timber supports; plus E01's preserved defective project |

The schema 1–3 inputs are unchanged Git blob content (LF), also present at
`69834bc`. Schema 4–7 inputs are frozen **reconstructions**, not claimed to be
original user projects saved by historical binaries. Their detailed provenance
is in the manifest. Missing early optional asset pivot data exercises the current
documented default. GLB assets are referenced, not loaded; this tests authored
asset metadata preservation, not external-file availability or import rendering.

The corpus test loads via the real filesystem persistence API, migrates, validates
against independently stored expected snapshots (deserialized directly without
migration), checks every authored field, generates geometry, saves to a separate
temporary copy, reloads, overwrites that copy atomically and reloads again. The
original fixture bytes must remain unchanged. It also rejects truncated input,
unknown data and a future schema for every case. These bounded malformed cases
are not a fuzzing or exhaustive security qualification.

Full authored state, IDs, defaults and save/reopen state require exact typed
equality. Legacy Hermite→NURBS is a representation change, so its evaluated value,
first derivative and second derivative are compared over 1001 samples per
transition using the existing `transition_regions` bounds: `<2e-14`, `<2e-12`,
`<2e-9`. Generated position differences before/after migration must be `<2e-12 m`,
also taken from that existing suite, with equal sample counts. Every other field
is compared exactly. No existing tolerance is changed. These samples include
both sides of the stored discontinuity through each transition's endpoints.

## Known issue E01 — deliberately retained

The [audit §4.5](audits/core-engine-audit-2026-11-15.md#45-confirmed-defect-operations-use-inconsistent-authored-and-physical-stations)
reproduction is now permanent: a radius-20 m, 40 m rider-authored arc with normal
offset −2 m produces a 36 m physical path. A brake at 38–40 m passes authored
generation and save/reopen but fails physical-track operations validation with
`device stations exceed current track; revise its start/end positions`.

The test intentionally asserts **current defective behavior**, is named as a known
defect and is not an ignored/expected-failure test. An intentional domain-contract
fix must update that behavioral assertion in its own reviewed change while keeping
the input intact. This test performs no train motion and does not claim the
project is simulation-ready. No stations are rescaled and no E01 fix is made here.

## Commands and evidence

From the repository root, with PowerShell 7, pinned Rust 1.100.0 and Python 3:

```powershell
./tools/verify-regression-baseline.ps1 -Comparison Exact -MachineLabel laptop
```

This checks SHA-256/inventory before and after, runs the compatibility integration
test, then physics goldens in debug and Release. Every Cargo command uses
`--locked`. Logs and `result.json` go to a fresh ignored `target/regression-r03/`
directory. The report records commit, working-tree status, compiler, Cargo,
OS/processor description, explicit Rust flags, machine label, command arguments,
exit codes and comparison mode. Successful results apply to that configuration;
dirty-tree evidence is explicitly visible. No GPU, active desktop control,
application launch, timing benchmark or package build is involved.

For a numerical investigation after an exact mismatch:

```powershell
./tools/verify-regression-baseline.ps1 -Comparison Tolerant -MachineLabel comparison-machine
```

The separate **Deterministic regression (CPU only)** Actions job uses Exact,
has a **20-minute timeout**, uses the pinned toolchain and uploads numerical logs
and the report for 14 days even after failure. The default workspace test job
also runs both new integration-test binaries. R0.2's existing release/package job
is unchanged. Required-check repository settings are not changed by this pass.

### Baseline maintenance

`tools/verify-regression-fixtures.py` is read-only and has no update mode. Fixture
directories have explicit LF checkout attributes so SHA-256 is independent of
`core.autocrlf`. Normal tests/CI never write expected results or source fixtures.

For a deliberate, separately reviewed solver/migration change, the capture tools
write to **new directories only**:

```powershell
cargo run -p qx-physics --example capture_replay_baseline --locked -- target/replay-candidate-NEW
cargo run -p qx-document --example capture_compatibility_fixtures --locked -- target/compatibility-candidate-NEW
```

Capture tools reuse existing fixture constructors and are development utilities,
not shipping commands. Compare candidate inputs/outputs, field changes and numerical
evidence before accepting anything. Preserve old schema inputs permanently; add
cases for new schemas/variants. Do not overwrite an old input or loosen a bound
because a check failed. Update expectations/manifests only as an explicit reviewed
compatibility decision; keep historical reference sets when solver semantics change.
The capture tools do not automatically rewrite manifests or promote candidates.

## Guarantees and limits

Same-machine bit identity is checked across independent initialization, every
outer tick's recorded fields and reset/replay. Passing local debug/Release proves
those profiles match this baseline on that tested machine. A green hosted run
additionally demonstrates exact agreement with that particular Windows runner,
compiler and fixture set; it is **not** proof across all CPUs, operating systems,
compilers, flags, architectures or mathematical libraries. Compare the CI evidence
with machine-specific reports. The owner's desktop is unverified until separately
tested; these Cargo-based tools require a toolchain and are not a new packaged
desktop verifier. R0.4/R0.5 or later work is not implemented.

The fixtures freeze current behavior, including approximation errors and startup
transients. They do not establish full-ride energy conservation, 60–180 s stability,
long-train qualification, a real-time envelope, wheel contact/collisions,
operational ride control or ride safety. Existing experimental-feature suites,
ignored long research controls, GPU tests and manual UX remain separate evidence.
There is no UI change or temporary developer UI in this pass.
