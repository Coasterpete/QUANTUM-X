# Transition splitting and scalar NURBS regions (2026-10-30)

This report preserves the implementation pass's original evidence. See the later
[source-publication audit](publication-2026-10-30.md) for current verification and
publication boundaries. Git-ignored evidence links below are local artifacts and
will not resolve in a fresh source checkout.

The subsequent [local C++ reference study and Rust editor pass](transition-cpp-reference-report.md)
builds on this implementation and records current interaction changes, source
comparisons, regression results, limitations, and fresh before/after captures.

## Diagnosis

The supplied recording at approximately **03:10** shows the second region's start
being dragged independently at approximately **3.577 m**. The viewport diagnostic
reports a pitch value jump of **1.2488377508273916 rad/m**. The previously flat
profile was continuous immediately after subdivision; the independent endpoint
edit introduced the visible step. Changing the selected function to cosine did
not connect that start to its preceding region. This was authored state, rather
than an apparent discontinuity introduced by graph rendering or unit conversion.

There was also a separate splitting defect: `editing::split_hermite` replaced
every source with two cubic Hermite interpolants and set their amplitude to zero.
This preserved endpoint values and slopes but changed quartic bumps, quintics and
cosines inside their intervals. Previous documentation explicitly called it a
reshape. That behavior no longer satisfies the owner's split requirement.

Finally, editor-only boundaries previously repartitioned the RK4 sample grid.
Even an equivalent polynomial could consequently produce different integration
stations and small numerical geometry differences.

## Representation and mathematics

`qx-geometry::profile::NurbsProfile` is a **scalar engineering profile**, with
degree, scalar control values, a clamped normalized knot vector, positive rational
weights, and explicit incoming/interior continuity contracts. New profiles are
cubic with unit weights. Control values have the selected channel's units:

| Channel | Output | Input |
| --- | --- | --- |
| Normal / lateral force | g | section distance m or design time s |
| Pitch / yaw / roll rate | rad/m or rad/s | section distance m or design time s |
| Spline-section roll | rad/normalized distance | normalized physical arc distance |

For region domain length `h`, `u = x/h`. Homogeneous de Boor evaluation gives
numerator `A` and denominator `W`; `f=A/W`, `f'=(A'-f W')/W`, and
`f''=(A''-2 f' W'-f W'')/W`. Normalized derivatives are divided by `h` and `h²`.
Derivative control nets and sided limits are analytic, including at repeated knots.
No physical station is inferred from uniform spacing of control points.

Subdivision uses homogeneous Boehm knot insertion to degree multiplicity,
extracts the two control nets, and affinely renormalizes their knot domains.
The left region retains its ID; the right receives a checked fresh channel ID.
Neither other regions nor channels are rewritten. The section extent and all
downstream stations remain fixed. Profile sizes, knot ordering/multiplicities,
finite values, positive weights and endpoint mirrors are validated.

Integration boundaries distinguish actual rational-span changes from redundant
editor splits. Analytic jets through order 2p (at most 10) identify real piece
changes, including narrow C2 features with higher-derivative jumps. Redundant
inserted knots do not change RK4 stations. A conditioning allowance reflects
roundoff in short domains; malformed/nonfinite results retain conservative breaks. Existing force-vector,
world/rider-local rotation, banking, centerline integration, curvature measurement,
rendering and rigid-body train models remain separate. Profile continuity is not
presented as proof of geometric or mechanical smoothness: generated geometry and
the existing rigid-body solver have independent regression comparisons.

Mathematical references: [MTU homogeneous NURBS knot insertion](https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/spline/NURBS-knot-insert.html)
and [CMU Hermite/Bernstein basis conversion](https://15462.courses.cs.cmu.edu/fall2020/article/10).
The implementation is project-owned Rust; no reference implementation was copied.

## Editing and continuity

The existing colorful Transition Editor, channel colors, docking, value handles,
timing diamonds, view navigation, precision entry and Apply/Cancel remain.
Owner-requested changes are implemented in the existing surfaces:

- **Split region (preserve shape)** and **Merge with next (preserve shape)**.
- Numeric and graph endpoint edits identify constrained shared boundaries as
  edits to **both neighboring profiles**. This scope is stated in the value editor,
  hover feedback and tooltip. They preserve the shared physical-domain jet and
  uninvolved outer endpoint jets; all other regions remain untouched.
- Routine **Shape controls** reshape only the selected region, preserving its
  endpoint values and first/second derivatives. Extra control freedom is obtained
  through shape-preserving refinement; users need not manage knots or weights.
- Moving a timing boundary changes the adjacent durations, preserves their outer
  endpoint jets and shared jet, and keeps the total distance/time unchanged.
- Optional **Advanced NURBS / continuity** exposes C2/C1/C0/Free rules, internal
  knot continuity, all interior controls, weights, and exact knot insertion.
  Relaxing a rule changes no curve. Tightening validates the existing curve and
  rejects incompatible values with numeric gaps; it does not fit neighbors.
- **Join incoming smoothly (C2 reshape)** explicitly repairs an already-saved gap.
  It retains the selected region's start jet and both outer jets while reshaping
  those two neighboring profiles, with Apply/Cancel. Legacy cosine interiors
  explicitly become cubic under this command; loading never performs this repair.
- **Reshape as cubic NURBS** explicitly changes the interior while retaining
  endpoint jets. It remains a reversible draft, useful for non-cubic legacy data.

Invalid control, weight, reorder or boundary edits retain the previous valid
draft. Reordering preserves the constraint at the affected boundary; it cannot
implicitly drop a split's C2 contract. Intentional value steps require the advanced
Free choice (or existing unconstrained legacy data). C0 and C1 constraints allow
their stated derivative changes. Generated diagnostics include value, first and
second derivative jumps, not just the presence of a transition ID boundary.
Join validation uses absolute `1e-8` plus relative `1e-9` tolerances in the relevant
domain; more sensitive regression tolerances are specified below and in tests.

No new product screen or developer-only interactive UI was introduced. The CLI
evidence scenario is development-only. Exact visual acceptance still belongs to
the owner; Windows pointer operation was not manually verified by an agent.

## Persistence and compatibility

Schema **6** adds these profile functions and continuity contracts. Versions 1–5
load through explicit migration; older versions containing new representations
are rejected. Unknown fields still fail rather than disappearing during save.
Legacy Hermite regions convert by algebraic change of basis:

`P = [start, start + h*start_slope/3, end - h*end_slope/3, end]`.

Nonzero amplitude adds the original quartic `16 A u²(1-u)²` exactly. Such regions
retain degree 4, and exact quintic conversions retain degree 5. It is impossible
to preserve those general polynomials as cubic profiles without approximation.
Cosine is not a finite rational polynomial: existing cosines stay analytic, and
splitting uses exact analytic restriction plus polynomial composition for a bump.
Restricted cosine editing requires an explicit reshape; it is never silently fit.

Legacy continuity is **Free**, preserving preexisting steps, IDs, durations,
values, slopes and authored intent. Migration does not heal a previously authored
jump. Numeric diagnostics expose it, and the owner can explicitly use Join
incoming smoothly to repair it while retaining review and undo.
Project save/reopen, command history and draft Apply/Cancel use the existing
transactional document paths. No train or asset schema is replaced.

Exact merge currently requires equal degrees, equal shared values and a combined
maximum of 256 controls. Incompatible merges report an error and retain data.
The UI permits split fractions 0.001–0.999; the API additionally checks both pieces
retain at least `1e-8` domain units. Extremely short pieces have the usual
double-precision derivative conditioning limits; no certification claim is made.

## Reproduction and evidence

From the repository root (the output directory must be new):

```powershell
cargo run --release -p quantum-x -- transition-scenario target/transition-evidence/captures
cargo test -p qx-geometry --test profiles
cargo test -p qx-document --test transition_regions -- --nocapture
cargo test -p qx-physics --test profile_equivalence -- --nocapture
```

The scenario writes four actual wgpu/egui PNGs, corresponding project fixtures,
10,001 CSV samples and `numerical-evidence.json`. The before/after edit pair
**reconstructs** the independent start drag using the value visible in the
recording; it is not a saved copy of the owner's original project. The separate
before/after split pair verifies unchanged nonconstant shape. All fixtures are
explicitly synthetic; their rendered layout is not a proposed coaster design.

Measured in this workspace:

| Check | Maximum difference |
| --- | --- |
| 10,001 cubic profile samples after split at 3.577 m | `1.388e-17 rad/m` |
| First / second physical derivative | `2.168e-18 rad/m²` / `9.216e-19 rad/m³` |
| Generated position / curvature, 101 matching stations | `1.388e-17 m` / `8.153e-16 /m` |
| Reconstructed old independent-edit value jump | `1.2488377508273916 rad/m` |
| Shared edit value / first derivative jump | `0` / `0` |
| Shared edit second derivative jump | `1.302e-15 rad/m³` |
| 240 existing rigid-body steps: body position / velocity | `7.110e-15 m` / `6.026e-13 m/s` |

Tests additionally cover nonunit rational weights, nonuniform/multiple knots,
splits at existing knots, repeated subdivision, 0.001/0.999 fractions, narrow C2
features, preservation of full legacy numeric limits, amplitude
and quintic preservation, malformed data, shared edits, isolated neighboring
regions, explicit gap repair, timing edits, exact merge, normalized roll on all
existing positional curve methods, time/distance round trips, legacy step
preservation, undo/redo and save/reopen. Real egui pointer sequences test the
shared C2 handle in docked/floating panes at 100%, 150% and 200% UI scaling,
without sending input to the Windows desktop.

Manual owner checklist: launch the packaged executable; create a rider-local
section; split pitch at 0.3577; drag either shared value; verify the smooth join,
reshape controls, timing diamond, Apply/Cancel, undo/redo and save/reopen.
Under advanced controls, choose Free deliberately and confirm the resulting
diagnostic; tightening to C2 must reject incompatible data without losing it.


## Final verification and local delivery

All of these completed successfully on native Windows:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` — 260 passed, zero failed.
- `cargo build --workspace --release`
- `tools/package-windows.ps1 -OutputDirectory "target/distribution/QUANTUM Dynamics-0.1.0-transition-fix-windows"`
- Packaged public executable `transition-scenario target/transition-evidence/release-captures`.
- `git diff --check`.

The portable distribution verifies public/development executables, embedded icons,
catalogs/assets, Welcome startup and project-open entry points. The final scenario
ran on the NVIDIA GeForce RTX 4070 with Vulkan. These are offscreen initialization
and rendering checks, not an FPS benchmark or manual Windows interaction test.

Evidence (generated, ignored by Git):

- [Before edit reconstruction](../target/transition-evidence/release-captures/recorded-action-before-fix.png)
- [After shared-boundary correction](../target/transition-evidence/release-captures/recorded-action-after-fix.png)
- [Before nonconstant split](../target/transition-evidence/release-captures/profile-before-split.png)
- [After nonconstant split](../target/transition-evidence/release-captures/profile-after-split.png)
- [Numeric evidence](../target/transition-evidence/release-captures/numerical-evidence.json)
- [Verification summary](../target/transition-evidence/verification-summary.json)
- [Full test log](../target/transition-evidence/workspace-tests-verified.log)

The current branch remains `codex/public-identity-library`, HEAD `8691591`.
The preexisting local work was retained. This pass is uncommitted; nothing was
pushed or merged, and no PR was created, following the current explicit request.
