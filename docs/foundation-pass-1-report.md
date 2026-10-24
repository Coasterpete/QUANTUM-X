# Integrated core foundation pass 1 — development report

Completed locally on 2026-10-24, branch `codex/integrated-core-foundation`.
The primary acceptance target is implemented and verified: authored graph/timeline
channels generate a continuous, reproducible three-dimensional centerline with
derived diagnostics, and the authored project survives filesystem save/reopen.

## Implemented and integrated

| Existing area extended | Working behavior |
| --- | --- |
| `qx-geometry::math` | f64 vectors, normalization, dot/cross, Hamilton quaternions, rotations, slerp, validated local/world rider frames, and numeric comparison utilities. |
| `qx-geometry::transition` | Seven requested function families; ordered stable-ID sub-transitions; analytic derivatives; amplitude adjustment; insertion/removal/replacement; C0/C1 diagnostics; compensated input prefix sums. |
| `qx-geometry::authoring` | Stable-ID named ordered sections; explicit domain/extent, initial/inherited boundary poses and join policies; force, world-geometric, rider-local, straight, and radius/plane arc definitions. |
| `qx-geometry::solver` | Coupled spatial RK4 generation; design-speed force-to-curvature conversion; quaternion frame transport; knot-aligned one-sided integration; shared section boundaries; arc-distance queries; curvature, forces, banking, twist, world pitch/yaw, reference-speed diagnostics; contextual errors and warning thresholds. |
| `qx-document` | Version-1 metadata, complete authored track and initial conditions, family/style IDs; strict JSON, float round trips, validated save/load with sibling-file replacement; transactional reusable edit batches and bounded undo/redo. |
| `qx-style` | Independent versioned JSON catalog with family IDs, style numbers, descriptions, compatible trains, asset/preview references, and development status. Three honest concept entries, with no fabricated assets. |
| `apps/quantum-x` | Development-only `demo` and `generate` CLI commands composing all those systems; project save/reopen verification, refinement comparison, CSV/OBJ/SVG exports and summaries. |

No new crate was introduced. Serde and serde_json were added for maintained
serialization support; Cargo locks the actual versions and transitive dependencies.
Their license metadata was inspected. Existing MIT/Apache-2.0 declarations were
preserved; the previously undeclared `qx-style` package now states those same terms.
Physics, audio, and rendering scaffolds remain unchanged.

The actual requested openFVeiD files were reviewed as functional research, with
notes and a pinned upstream source reference in
[research/openfveid.md](research/openfveid.md). No GPL source or translated
implementation was incorporated.

## Track-generation mathematics

With right-handed initial axes `T=+X`, `N=+Y`, `L=+Z`, gravity `G=(0,-g,0)`, and
constant design speed `v` per section, authored normal/lateral load multiples
`n,l` determine the force section's curvature:

```text
kN = (g*n + G·N) / v²
kL = (g*l + G·L) / v²
dP/ds = T
dT/ds = kN*N + kL*L
omega = kN*L - kL*N + twist*T
dq/ds = 0.5 * (0,omega) * q
```

Time-domain angular rates divide by `v` to become rates per metre. Time-domain
extent multiplies by `v` to become spatial length. World-geometric sections use
gravity-referenced pitch and world heading; rider-local sections use `N/L` as
their curvature directions. Force sections and geometric sections have different
mathematical meanings. Reference speed does not come from train simulation.

Generated-graph load estimates use finite differences of adjacent output tangents,
projected onto the output midpoint rider frame. They are independent of the
authored force columns. Classical RK4 gives fourth-order convergence on smooth,
resolved sections; graph finite differences and midpoint elevation are second
order. Roll is frame twist, which differs from banking-angle change in general.

The complete signs, equations, transition definitions, singularity behavior,
boundary tolerances, and API/persistence policies are in
[core-foundation.md](core-foundation.md).

## Run the demonstration

From the repository root:

```powershell
cargo run -p quantum-x -- demo target/foundation-demo
cargo run -p quantum-x -- generate examples/foundation.qx.json target/foundation-loaded 0.1
cargo build --workspace --release
.\target\release\quantum-x.exe generate examples/foundation.qx.json target/foundation-release 0.1
```

The original example combines a lead-in, force-vector rise/bank, rider-local
recovery, time-domain world turn, and construction arc. Outputs are the authored
JSON project; centerline, measurements, and transitions CSV; OBJ centerline; three
SVG plot files; and a text summary. The checked-in version-1 fixture is
`examples/foundation.qx.json`. Generated artifacts are under ignored `target/`.

Actual default demo result:

| Measurement | Observed result |
| --- | --- |
| Sections / centerline samples / measurement rows | 5 / 1521 / 1520 |
| Integrated track length | 151.999999999996 m |
| Endpoint, metres | (114.04327140833351, 22.251725311225908, 75.46754437004525) |
| Save/reopen/regenerate | Identical complete generated output |
| 0.1 m versus 0.05 m maximum sampled position difference | 9.788293345539e-11 m; refinement comparison, not an error bound |
| Maximum derived-versus-authored normal residual in force section | 2.039377070107e-6 g |
| Maximum derived-versus-authored lateral residual in force section | 1.334794570429e-6 g |
| Maximum frame-axis norm error | 4.440892098501e-16 |
| Maximum pairwise frame-axis dot magnitude | 3.053113317719e-16 |
| Warnings | 3 intentional section-rate jumps entering sections 3, 4, 5 |

The initial force join is compatible; the later example boundaries deliberately
allow abrupt curvature/twist changes and report them. Their positions/orientations
remain continuous. Debug and Release generated byte-identical project, all three
numerical CSVs, and OBJ for this fixture on this Windows machine. That observation
does not promise bitwise equality across future compilers, CPUs, or platforms.

## Verification actually executed

All commands below passed on native Windows with rustc 1.99.1 / Cargo 1.99.1:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
git diff --check
```

The workspace ran **26 tests, zero failures**, including three unchanged scaffold
tests in physics/audio/rendering. Substantive tests cover mathematical rotations,
all transition families and derivatives, compensated boundaries, independent
analytic circles and force balance, fourth-order convergence, world/rider frame
differences, time/distance domains, vertical loops, singularities and invalid input,
budgets, continuity policies, determinism, compatibility fixtures and unknown fields,
Windows save/overwrite behavior, editing batches/history, style validation and
centerline independence, and the actual CLI save/load/export workflow.

Independent reference assertions include a radius-40 m force-generated horizontal
circle with endpoint error below 1e-9 m at 0.1 m steps, and a radius-10 m vertical
arc whose error reduces by more than a factor of 12 per step halving and is below
1e-7 m at 0.5 m steps. Those are test-case tolerances, not universal solver limits.

Additional output inspection, separate from Rust tests: Python's standard XML
parser accepted all three SVGs; CSV inspection checked finite values, strict
distance ordering, frame invariants, and the force residuals above. SVGs were
rendered to PNG using the available development runtime and visually inspected.
Plot spacing was corrected after inspection. PNG generation is not an application
dependency. Both Debug CLI commands and the native Release CLI were run.

## Completion, limitations, and UX status

All primary pass-1 deliverables are present: reusable mathematics, authored
sections/transitions, real force-derived geometry, resulting numerical graphs,
versioned persistence and editing/history, independent concept styles, diagnostic
execution/exports, research notes, tests, and documentation.

Known limitations are explicit:

- Constant reference speed per section, no energy-derived speed, heartline offset,
  vehicle bodies, wheel contact, forces from mechanisms, or train dynamics.
- Open centerlines without loop-closure/endpoint target solving or physical rail
  surface meshes. OBJ is a polyline diagnostic.
- Fixed spatial sampling with an angular-step guard; no adaptive local-error
  controller. Step-halving is optional evidence and reports unavailability if a
  budget/minimum-step restriction prevents the refinement run.
- World-geometric pitch/heading reject near-vertical tangents; rider-local controls
  continue through them. Gravity-referenced bank is unavailable near vertical and
  otherwise wrapped modulo 2pi.
- Full regeneration and snapshot history; no performance claims or large-scene
  benchmarks. Arbitrary file/asset compatibility, filesystem power-loss guarantees,
  concurrent editing, and non-Windows behavior were not verified.
- Concept styles have no production assets or previews. No complete editor,
  GPU viewport, final visual design, or rigid-body physics is claimed.

Owner-approved direction followed: ordered graph/timeline authoring, time/distance
domains, distinct force/geometric controls, and visual-style independence.
`docs/ux/coaster-properties.md` was read and preserved. No product GUI was added.
The CLI and diagnostic plots are explicitly development-only. Final screen layout,
graph interaction, Coaster Properties presentation, and other editor UX remain
human-directed; no new visual concept is treated as approved.

## Next pass and Git state

Use these existing commands and models for the next owner-designed graph/timeline
interaction pass. Numerical priorities are adaptive integration/error diagnostics,
incremental regeneration, boundary/closure constraints, and rail/contact geometry.
Actual rigid-body vehicle dynamics require a dedicated validated implementation,
with the intended 1/240 s baseline timestep kept separate from geometry sampling.

Work is local and uncommitted on `codex/integrated-core-foundation`. No commit,
push, PR, merge, or release was created. Pre-existing untracked `AGENTS.md` and
`docs/ux/` were preserved; the pre-existing untracked `qx-style` placeholder was
extended in place as requested. No destructive Git operation was performed.
