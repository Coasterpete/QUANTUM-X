# M4I production starting-velocity proposal — authorization required

This is a design and test-only evaluation, not authorization to modify playback.
M4H production files, including `playback.rs`, `solver.rs`, `angular.rs`,
`devices.rs`, and document operations, are unchanged.

## Problem and measured counterfactual

`TrackSimulation::with_operations` seats the assembly, then assigns each body's
sampled tangent times the requested speed. Angular velocities remain zero.
Different body tangents need not give equal velocities at connected anchors;
on curved track the longitudinally separated guides of a single carriage also
require angular velocity. Seating valid positions does not satisfy velocity
constraints. The first integration step must remove those incompatible modes
at the same time as applying gravity, devices and stabilization.

The independent R20 horizontal reference uses 100 kg, isotropic 20 kg m², two
guide points at +/-0.3 m, and a rail roll constraint. Both flat and 0.6-rad-banked
starts at 15 m/s have initial transverse guide velocity 0.225 m/s. Straight
incline starts give zero. A test-only mass-weighted velocity projection using
these production point-guide/roll rows reduces the curved residual below 1e-8
without changing a pose or advancing time. The existing production read-only
`assembly_diagnostics::velocity_residuals` independently measures the result.

Let rho = sqrt(20² - 0.3²). The exact one-dimensional compatible velocity family
has omega_y = -v/rho. Minimizing the kinetic metric's distance to the original
(v=15, omega=0) seed gives

```text
v_projected = 15 * (100 rho²) / (100 rho² + 20) = 14.992502061933 m/s
omega_y = -v_projected / rho
```

This is distinct from requesting exactly 15 m/s **after** constructing compatible
angular velocities. The former removes about 5.623 J of the original seed's
kinetic energy; the latter supplies rotational kinetic energy as part of the
initial condition. Neither is an applied physical launch impulse. The six-second
zero-omega and compatible-omega public-engine controls quantify the subsequent
transient separately from ongoing discrete integration drift.

The test projection covers only these constant-bank single-carriage rows. It
does not pretend to solve the full production train, active limits, varying bank
or motors. Separate four-second public playback controls export actual train
initial residuals, all body states and energy, and verify exact reset/first-step
replay. No single-body centripetal formula is applied to articulated trains.

## Proposed construction after separate approval

After seating and validating the unchanged assembled positions, build velocity
rows from the **same production constraint definitions** used by stepping.
Extract reusable row assembly if needed; do not maintain a second approximate
production Jacobian. Reuse full rotated body inertia, mounting frames, point
arms, actual guide projections and the spatial bank derivative of rail twist.
Do not use B1's bilateral face datums in the production projector.

For stationary bilateral constraints, solve

```text
minimize 1/2 (v - v_seed)^T M (v - v_seed)
subject to J(q) v = b
```

Here b is zero for stationary passive constraints, and explicitly nonzero only
for a genuinely prescribed relative velocity. The velocity construction has no
gravity/force kick, Baumgarte position bias, pose correction or advancement.
Calling `World::step` with an artificial timestep is not an implementation of
this operation. Position defects require a seating failure or explicit repair.

Use rank/conditioning diagnostics and bounded resources. Production systems can
have redundant rows and thousands of bodies; the small test's full-rank dense
solve is not an adequate production algorithm. A future shared sequential-row
implementation must stop on residual and iteration budget, rather than assuming
32 sweeps suffice. Validate all constraint groups after the final sweep because
later rows can disturb earlier rows. A rank-revealing reference solve on bounded
fixtures should verify any production iterative implementation.

## Required cases and policy decisions

| Case | Required behavior |
| --- | --- |
| Straight level/incline and constant bank | Preserve an already compatible translation. Zero speed must not divide by a normalization scalar. Include negative speed. |
| Curved/constant-bank | Construct point velocities and angular velocities together; distinguish centerline station rate from COM speed on a chord. |
| Varying bank | Include the existing rail-roll row's linear station/projection derivative as well as angular terms. A torque-only correction is insufficient. |
| Articulated conventional/shared-bogie trains | Solve one coupled velocity system for all physical bodies, ball anchors, fixed/hinge/roll rows and guides. Never assign a separate prescribed trajectory to each car. Test multiple cars and unequal/anisotropic mass properties. |
| Active joint limits | Enforce the tangent-cone inequality at a reached bound: prohibit outward velocity, permit inward velocity. Use complementarity/active-set checks; do not freeze a free hinge or activate an unreached bound. Out-of-range position is a seating issue. |
| Torque motors | Retain the free hinge's permitted relative velocity; a torque motor is a force law, not a prescribed speed row. Do not apply a motor impulse during construction. |
| Prescribed relative velocities | Solve affine Jv=b with feasibility checks. Do not normalize all velocities afterwards, which would scale b. MechanicalProfile transmission remains unsupported as today. |
| Initial speed | Owner must choose the physical meaning: recommendation is signed tangent speed of one explicitly identified lead running-gear reference **after** projection. Current telemetry uses a mass-weighted body speed, which is a different measurement. Do not silently constrain every body to the same scalar speed. |
| Speed with affine rows/limits | Enforce a separate speed row while solving the coupled feasible system, or construct a particular affine solution plus an allowed travel mode. Scaling a homogeneous result is allowed only after verifying all rows/inequalities remain satisfied. Report infeasibility. |
| Launches/brakes/lifts/stations | Leave force laws and occupancy unchanged. Initialize once, then apply devices at the first real 1/240-s step. Do not repeatedly project to the requested initial speed or turn a launch into prescribed animation. Static holding/kinematic devices need their own explicit contract. |
| Reset/replay | Store the final successfully constructed world, hints, attitudes and consistent initial telemetry together. Reset restores that snapshot exactly; projection is not recomputed using accumulated state or a warmed cache. |
| Failure/rollback | Validate finite values, station domain, chart regularity, mass/inertia, rank, feasibility and every residual. Publish no partial world. Return an actionable error identifying the incompatible group and iteration/rank/limit failure. Preserve any prior playable state. |

The initial-speed semantic and how an initialization failure is presented still
need owner direction. This proposal neither chooses new UI controls nor changes
the schema. Existing serialized `initial_speed_mps` can retain its field while
its precise semantics are deliberately reviewed; no silent migration is proposed.

## Acceptance before production adoption

Require analytical straight/R20/constant-bank references; changing-bank references
with the full twist row; multi-car conventional/shared-bogie and off-axis mounts;
nonzero and zero/reverse speeds; active-bound inward/outward velocities; passive,
torque and supported prescribed-velocity drives; device starts; redundant and
infeasible rows; invalid/nonfinite data; deterministic reset and failure rollback.
Compare local solutions and energy construction to independent linear solves.
Keep initialization and subsequent integrator errors separately reported. A
successful initial projection does not resolve finite-step curvature error,
wheel contact, production energy drift or B1's tick-563 rejection.
