# AGENTS.md — QUANTUM-X

> Repository-wide instructions for AI coding agents and human contributors.
> These instructions apply to the repository root and all subdirectories unless a more specific `AGENTS.md` explicitly narrows a rule. Read this file before planning or editing code.

## 1. Project identity and mission

**QUANTUM-X** is a standalone, open-source, native desktop application for designing, editing, visualizing, and simulating roller coasters. Its long-term goal is a capable precision coaster-authoring environment with credible numerical simulation, excellent interactive performance, and an approachable professional workflow.

QUANTUM-X is **not** a NoLimits 2 plugin, a theme-park management game, or merely a track exporter. It draws inspiration from FVD++ (specifically [H27CK/openFVeiD](https://github.com/H27CK/openFVeiD)) for section-based force-vector and geometric authoring, and from NoLimits 2 for the breadth of a standalone coaster design and simulation environment. Neither reference defines QUANTUM-X, and QUANTUM-X must remain an independently engineered application. Do not copy GPL-licensed reference code into this MIT/Apache-2.0 project without a deliberate licensing review.

Core principles:

1. **Human-directed product and UX design.** The project owner decides how QUANTUM-X should look and feel and how people accomplish important tasks.
2. **Real, integrated functionality.** Prefer usable end-to-end features over disconnected demos, decorative interfaces, or scaffolding presented as progress.
3. **Precision and determinism.** Geometry, simulation, and file formats must be predictable, testable, and numerically robust.
4. **Modularity without architecture theater.** Keep clear interfaces between subsystems; avoid both giant files and unnecessary abstractions.
5. **Honest engineering.** Explain limitations and verify claims. Passing a build is not proof that the UX, rendering, or physics is correct.
6. **Affordable, maintainable software.** Avoid unnecessary proprietary services, heavyweight dependencies, and lock-in.

## 2. Authority and decision-making

Follow this priority order:

1. Explicit instructions in the current human request.
2. Human-authored specifications, accepted design decisions, and project documentation.
3. This `AGENTS.md` and any applicable more-local agent instructions.
4. Existing tested architecture and established project conventions.
5. Agent recommendations and implementation defaults.

If requirements conflict, identify the conflict and preserve the higher-priority decision. Do not silently reinterpret product requirements to make implementation easier.

**The agent may:** investigate, design technical approaches, implement substantial features, write tests, refactor within the task's scope, recommend alternatives, and document tradeoffs.

**The agent may not:** invent fundamental UX workflows, redefine the product, declare unapproved visual concepts final, replace owner-authored design choices, or claim a prototype is production-ready.

When a UX decision is missing, continue work on independent infrastructure where possible. For a minimal temporary interface necessary to test functionality, use a clearly labeled, replaceable **development-only** UI. Do not treat that temporary UI as approved product UX. Raise only decisions that materially block the work; do not repeatedly request approval for routine technical choices.

## 3. Human-drafted UX is a hard requirement

**Product workflows, interaction design, information architecture, and visual identity are human-directed.** AI may help implement them, assess usability, and propose clearly labeled options, but proposals are not approvals.

Rules for every UI/UX task:

- Start by reading the relevant human-authored wireframe, sketch, interaction specification, reference, issue, or explicit written direction.
- Translate approved designs into functioning UI; preserve the underlying interaction intent, not just its superficial appearance.
- Do not fabricate an approved screen, workflow, tool behavior, layout, iconography, or theme when no approval exists.
- If no design exists, implement nonvisual application services, reusable UI plumbing, or obviously provisional developer controls rather than locking in final UX decisions.
- Do not copy another application's interface wholesale. References to NoLimits 2, Blender, CAD packages, or ArtCraft applications are examples to evaluate, not designs to clone.
- Avoid gaming-oriented neon styling by default. The known direction is **professional, high-contrast CAD software** with restrained accent color; exact colors, layout, and final identity remain subject to human design.
- Distinguish **UX** (task flows and interaction behavior) from **UI** (the visible presentation). Both require intentional design.
- All implemented controls must perform their advertised action. No dead buttons, pretend tools, or purely decorative functionality in shipping paths.
- Provide discoverable commands, sensible keyboard shortcuts, precision numeric entry, clear feedback, and recoverable error states for approved actions.
- Support keyboard-first and pointer workflows where practical, and do not make color the only way to convey important states.
- Keep viewport, document state, and editor commands decoupled so layouts can evolve without rewriting the engine.

**UX change review:** In the completion report, explicitly list any owner-approved UX behavior implemented, any merely temporary developer UI, and any UX decisions still requiring human design. Do not imply silence means approval.

## 4. Implementation style: substantial, coherent passes

The owner prefers **large, integrated implementation passes** that accomplish meaningful work in a single Codex/Claude invocation, rather than dozens of small scaffolding milestones.

For a substantial request:

1. Inspect the relevant repository files, dependencies, tests, documentation, and working tree first.
2. State or internally establish the acceptance criteria and the components needed to satisfy them.
3. Implement the complete coherent vertical slice across affected crates, including wiring, persistence, UI integration *where approved*, tests, and documentation.
4. Run the available checks; fix issues attributable to the change.
5. Report exactly what works, what remains incomplete, and what could not be verified.

Do **not** respond with a plan alone when implementation is requested and repository access permits changes. Conversely, do not inflate scope to unrelated systems merely to make a pass feel larger. A substantial pass is one that delivers meaningful, demonstrable functionality.

Build in dependency order when it helps, but integrate and verify the whole requested feature before declaring it complete. Avoid repeatedly generating skeleton crates, placeholder APIs, or TODO-only implementations.

### 4.1 Computer Use and active-desktop protection

**Never take control of the owner's active Windows desktop without explicit, task-specific permission.** This applies to Codex, Claude Code, ChatGPT computer-control capabilities, and other AI coding agents.

- Do not move the mouse, type into GUI applications, switch windows, click controls, drag objects, or interact with the user's desktop on the owner's behalf unless explicitly authorized for that session.
- A request to implement a native 3D inspector, test rendering, or verify a feature **does not itself grant permission** to use interactive Computer Use.
- Prefer Rust/Cargo commands, static analysis, unit/integration tests, headless exports, noninteractive rendering tests, and logs. Launching a process for automated, noninteractive verification is not permission to control the user's active desktop.
- Provide exact commands and an owner-executable manual checklist for checks that require desktop interaction. If a check cannot be run without taking control, mark it **not manually verified**.
- Respect cancellation immediately; do not repeatedly reactivate desktop control after the owner cancels or declines it.
- Do not reduce the requested implementation scope merely because desktop interaction is unavailable. Implement what can be verified and report remaining manual checks honestly.

## 5. Repository and technology direction

- **Language:** Rust, using the edition and toolchain specified by the workspace (currently Rust 2024).
- **Desktop target:** Windows first. Linux through WSL2 is a development environment, not automatic evidence of native Linux support. macOS support is not presently a requirement.
- **Graphics:** `wgpu` as the intended graphics foundation. A custom rendering layer and progressively more advanced PBR are long-term goals.
- **Editor UI:** `egui` is the planned application UI toolkit, subject to human-approved UX direction.
- **Math / simulation / audio:** Favor project-owned domain logic when it provides long-term control and auditability. Do not reinvent OS/device abstractions, established numerical utilities, or graphics infrastructure without a clear technical reason.
- **Dependencies:** Prefer maintained, compatible libraries with suitable licenses. Explain significant dependencies and avoid coupling core data to a specific UI or graphics library.
- **Licensing:** Preserve existing repository license declarations and headers; do not unilaterally change licensing. The current repository uses `MIT OR Apache-2.0` package declarations.

Existing workspace areas include:

| Area | Responsibility |
| --- | --- |
| `apps/quantum-x` | Application startup, windows, editor composition, input routing, and command dispatch |
| `crates/qx-geometry` | Curves, coordinate frames, topology, sampling, numerical geometry |
| `crates/qx-physics` | Deterministic simulation, constraints, train dynamics, telemetry |
| `crates/qx-style` (when present) | Data-driven coaster families, numbered style definitions, asset/preview metadata, compatibility |
| `crates/qx-document` | Project model, versioned persistence, validation, undo/redo |
| `crates/rendering` (`qx-rendering`) | `wgpu` device/resource management, 3D viewport, scene rendering |
| `crates/qx-audio` | Audio-domain logic and backend integration |
| `assets/` | Properly licensed or original runtime/development assets |
| `docs/` | Product decisions, architecture, algorithms, and developer guidance |
| `tests/` | Cross-crate and integration tests as appropriate |
| `tools/` | Reproducible developer utilities |

These are ownership boundaries, **not** a demand for one crate per tiny idea. Additional crates or reorganizations require a demonstrable benefit and should respect existing project work.

## 6. Architecture and code standards

- Keep modules cohesive and interfaces explicit. Avoid monolithic `main.rs`, giant god-objects, and cross-crate cyclic dependencies.
- Core geometry, physics, and documents should not import editor widget types or GPU resource types.
- Make the document model authoritative. Viewport meshes, selection handles, visual caches, and telemetry are derived or separate state unless explicitly part of the saved format.
- Use strong types or clearly named fields for units, spaces, IDs, and domain concepts. Document conventions for coordinates, orientation, and units.
- Use `f64` for authored track geometry and simulation state where precision matters; convert to GPU-friendly representation deliberately at the rendering boundary.
- Prefer stable identifiers over transient array indices for persisted objects and references.
- Validate untrusted or corrupt input; return actionable errors rather than panicking during ordinary use.
- Prefer idiomatic Rust ownership, error propagation, scoped lifetimes, and small public APIs. Avoid unnecessary `unsafe`; document and justify every unavoidable `unsafe` block.
- Keep external library types at adapters where possible. Avoid future migrations that require rewriting the entire domain model.
- Do not introduce global mutable state or synchronous blocking work on the interactive rendering path without clear justification.
- Avoid speculative general-purpose frameworks; implement extensibility when there is a real near-term use case.
- Comments should explain invariants, algorithms, units, or surprising decisions—not merely restate the code.

## 7. Track geometry and authoring correctness

### 7.1 Primary graph/timeline authoring philosophy

**QUANTUM-X is fundamentally a graph/timeline-based roller coaster authoring application.** The primary authoring approach draws functional inspiration from FVD++/openFVeiD, including:

- Ordered, editable track sections/regions with explicit boundary behavior.
- Force-vector sections with authored normal and lateral force profiles and roll behavior.
- Geometric and rider-local geometric sections with well-defined pitch/yaw/roll controls.
- Time-based and distance-based section domains, with explicit units and conversion rules.
- Mathematically defined transition functions, sub-transitions, sequencing, and continuity controls.
- Resulting graphs and measurement diagnostics that distinguish authored inputs from derived quantities.
- A track-generation solver that converts the authored sections into continuous three-dimensional track geometry.

Reference implementations are research material, **not code to copy or interfaces to clone**. Preserve QUANTUM-X's own human-authored welcome-screen and editor UX specifications in `docs/ux/`. The transition graph/timeline is a primary authoring surface, not a secondary visualization of conventional 3D spline-control-point editing.

Additional approaches—including B-splines, NURBS, Catmull–Rom, and direct geometric tools—may be implemented as complementary authoring systems. Do not let them replace the graph/timeline-first design, and do not make all of them prerequisites for the first working force-vector authoring implementation.

### 7.2 Human-authored coaster layouts and track shaping

**The human user is the coaster designer. The AI coding agent builds, tests, and maintains the tools through which the user designs.** Do not confuse writing a curve solver with choosing the intended coaster layout.

- The owner/user determines track shape, layout, element sequence, forces, banking, transition timing/length, geometric control points, and artistic or ride-experience goals. These decisions belong in direct graph/timeline editing and complementary geometric-authoring tools.
- Build interactive, precise, predictable authoring operations on top of the tested math engine: editable graphs, ordered sections, numeric parameters, control points/handles where appropriate, responsive 3D feedback, and undo/redo. Follow the human-authored UX documents when implementing these interfaces.
- Do **not** automatically invent a production coaster, choose a final layout, or reshape authored sections just to make the demo look like a realistic ride. Do not silently move handles, redistribute sections, smooth forces, change banking, adjust curves, or reinterpret user intent.
- Automatic helpers (smoothing, continuity fitting, endpoint targeting, banking suggestions, section blending, procedural examples) may be offered as **explicit opt-in commands** with clear previews, constraints, diagnostics, and reversible edits. Leave the original authored state intact until the user applies the change.
- If authored geometry is discontinuous, unsafe-looking, numerically invalid, or mechanically implausible, show specific errors/warnings and optional fixes rather than quietly altering the layout. Report measurable quantities and avoid claiming ride-safety certification.
- Keep **development fixtures** separate from actual user-authored projects. Synthetic examples, random curves, intentionally sharp joins, and colored diagnostic frames are valid solver tests, **not** demonstrations of finished coaster design quality.
- Clearly distinguish mathematical validity, smoothness/continuity, simulated ride behavior, and subjective ride quality. No numerical solver or coding agent alone determines whether a layout is a good coaster.
- All supported authoring methods—force-vector graphs, geometric sections, cubic Bézier, B-splines, NURBS, and Catmull–Rom—must feed the same coherent downstream track representation, without converting every source to Bézier or turning the app into a spline-only editor.
- The production editor must let a person author a meaningful coaster without hand-editing JSON files, programming Rust, or asking an AI to generate the shape. Developer CLI tools may remain for tests and debugging.

**Acceptance principle:** Codex is successful when a human can deliberately create and refine the desired track using QUANTUM-X; an AI-generated example that happens to render attractively is not proof of a usable authoring workflow.

### 7.3 Mathematical geometry and precision

Treat track authoring as an engineering-grade numerical subsystem, not a visual approximation.

- Support a coherent mathematical model for B-splines and NURBS, including degree, knot vectors, weighted control points, valid parameter domains, and well-defined derivative behavior.
- Bézier curves may be supported as a distinct representation or special case; never silently treat every curve as Bézier.
- Use double precision for control geometry, evaluations, and distance-domain data.
- Validate knots, multiplicities, weights, degenerate spans, and invalid or nonfinite inputs. Define how invalid geometry is handled.
- Arc-length evaluation, adaptive sampling, lookup tables, and arc-length inversion must have measurable errors and tests; distance-based traversal must not assume uniform parameter spacing.
- Track orientation must be explicitly modeled or derived by a documented, continuity-aware method. Avoid unstable frames, unexpected roll flips, and hidden coordinate-system assumptions.
- Distinguish centerline position, tangent, normal/binormal or transported frame, roll, gauge, and rendered track-family geometry.
- Regenerate dependent geometry deterministically after edits; invalidate caches correctly.
- Preserve precision through numerical editing, transforms, undo/redo, save/load, and repeated revisions.
- Test line-like, curved, tightly curved, near-degenerate, endpoint, and discontinuous cases. Include reference analytic curves where feasible.

Never claim a curve evaluator is a complete coaster-authoring solution unless editing operations and their downstream effects are actually implemented and tested.

## 8. Physics and simulation correctness

Physics is a real numerical model, not motion keyed to spline progress.

- Use a **fixed simulation timestep of `1.0 / 240.0` seconds** for the main deterministic solver unless the owner explicitly changes this requirement.
- Separate simulation time from rendering time; rendering frame rate must not control the numerical physics timestep.
- Represent physical units explicitly (SI by default) and document coordinate directions, forces, masses, inertia, gravity, velocity, and acceleration.
- Compute forces, constraints, and integration consistently. Do not manually advance a train along a curve and call it dynamics.
- Introduce complexities progressively, with accurate names: a constrained point-mass diagnostic is **not** a full articulated multi-car train simulation.
- For train modeling, keep car bodies, bogies, couplers, restraints, wheel/rail contact assumptions, and operational control systems distinguishable.
- Keep operator devices (lift, launch, brake, transport, station, block systems) separate from core integration and make their behaviors observable and testable.
- Test deterministic replay, timestep consistency, speed/acceleration, forces, energy behavior under defined assumptions, and edge cases.
- Report numeric tolerances and known model assumptions; do not claim physical realism solely because results look plausible.
- Avoid safety or ride-certification claims. QUANTUM-X is an enthusiast and creative design/simulation tool, **not** a substitute for professional ride engineering or regulatory certification.

### 8.1 Physics-Driven Simulation — Fundamental Architecture Requirement

QUANTUM-X is a standalone roller coaster design and simulation application, not a spline-following animation program.

#### Separation of responsibilities

The track-authoring system generates geometric track data using graph/timeline-based force-vector design, geometric transitions, and additional supported mathematical authoring methods.

The physics system independently simulates the dynamic behavior of roller coaster vehicles interacting with that track.

**Authoring geometry must never be confused with the vehicle's simulated motion.**

#### Rigid-body simulation requirements

The long-term physics architecture must support:

- Rigid bodies with mass and inertia.
- Translational and rotational dynamics.
- Articulated multi-car trains.
- Physical joints, bogies, and couplers.
- Wheel/rail contact and mechanical constraints.
- Collision detection and response.
- Gravity, propulsion, braking, and resistance forces.
- Aerodynamic forces and environmental effects.
- Weather- and temperature-dependent physical properties.
- Deterministic fixed-timestep simulation where feasible.

The target baseline timestep is 1/240 second, with additional internal substeps where necessary for numerical stability.

#### Prohibited shortcuts

Do not implement the production train simulation by assigning vehicle positions directly from a spline parameter or prescribed animation timeline.

Spline-following motion is permissible for geometry previews, editor navigation, diagnostic fixtures, and explicitly identified simplified test models.

Such approximations must never be presented as completed rigid-body train physics.

#### Physical realism

Physical behavior must derive from documented mathematical models, measured parameters where available, and validated numerical methods.

Environmental effects must have plausible physical relationships and configurable parameters. Avoid arbitrary realism multipliers.

#### Engineering objective

Build toward a simulation in which coaster vehicles respond dynamically to the track, mechanical components, propulsion, braking, environmental conditions, and physical interactions.

Realism must be verified through tests and numerical diagnostics rather than asserted solely because rigid-body calculations are present.

## 9. Rendering and interactive performance

- Use `wgpu` behind `qx-rendering` interfaces; keep device handling, frame lifecycle, synchronization, resource ownership, and error recovery contained.
- Support robust resize, minimized-window, surface-loss, and device-failure handling wherever the platform permits.
- Prefer incremental mesh and scene updates over full rebuilds for every minor UI event when measurable performance justifies it.
- Distinguish world-space double-precision geometry from view-relative or GPU-space float representations; avoid precision loss in large scenes.
- Keep selection/picking, editing overlays, grid/axes, camera controls, and renderable scenery separate where useful.
- Provide meaningful profiling or diagnostics for expensive generation/render operations.
- **60 FPS or better** is a performance goal for reasonable supported scenes and hardware, not a benchmark result to claim without measurement.
- Advanced lighting, HDR, materials, and PBR should improve real geometry rendering rather than conceal broken track generation.
- Clearly label debug meshes and primitive stand-ins as development assets.

## 10. Track families, trains, and assets

- Track geometry, track-family visual profiles, support attachments, train assets, and physics parameters are distinct concepts.
- Use stable catalog IDs and explicit versioning or validation for external style/asset definitions when introduced.
- Coaster Properties must support family and numbered-style selection; released styles should have representative screenshots/thumbnails and larger previews tied to catalog metadata, following human-authored UX specifications.
- Ensure substituting a GLB model or track style does not rewrite the authored mathematical centerline.
- Existing concept targets include **Hybrid-100**, **Monorail-100**, and **Monorail-200**, but names and catalog scope are not permission to fabricate finalized manufacturer-accurate assets.
- Build import pipelines around documented coordinate systems, scales, pivots, normals, materials, and asset provenance.
- Use original, properly licensed, or expressly permitted third-party assets. Do not copy proprietary CAD, manufacturer, simulator, or game data without rights.
- Placeholder geometry, mock train bodies, and debug assets are allowed during development, but **must not be presented as polished release content**.

## 11. Documents, compatibility, and undo/redo

- Treat a project as a structured, versioned document—not a serialized dump of live UI pointers, ECS internals, or GPU state.
- Provide explicit schema versioning and validation, with migration paths when breaking changes are introduced.
- Ensure save/load round trips preserve meaningful authoring state and stable IDs.
- Never silently discard unsupported or unknown data during normal read/write unless the format's policy explicitly allows it and the user is warned where appropriate.
- Perform document operations transactionally or with recoverable failure behavior; avoid partial corrupted writes.
- Support logical undo/redo at command or transaction boundaries, including geometry edits and numeric inputs where implemented.
- Distinguish temporary editor preferences, document state, simulation state, and derived caches.
- Add fixtures and compatibility tests before changing document formats.

## 12. Accessibility and interaction requirements

When implementing approved workflows:

- Provide keyboard shortcuts for relevant commands and reflect those shortcuts in menus or tooltips where appropriate.
- Use precise numeric controls as well as pointer manipulation for geometric editing.
- Support undo/redo, selection feedback, clear edit modes, and predictable cancel/confirm behavior.
- Ensure high-contrast presentation and usable focus states; do not rely solely on color to identify selections or errors.
- Handle high-DPI scaling and resizable windows thoughtfully.
- Prefer responsiveness and clarity over ornamental animation.
- Never invent an interaction model simply because an egui widget makes it convenient. Escalate material UX decisions to the human-designed specification.

## 13. Testing and verification

Tests are part of implementation, not a later optional phase.

Run as applicable from the workspace root:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Also perform platform-specific Release builds or runtime checks when the feature needs them and the environment supports them. If a command cannot run because of environment limitations, report that **it was not run**, why, and what should be checked on the target machine.

Coverage should include, as relevant:

- Geometry reference solutions, continuity, boundary conditions, numerical tolerances, invalid inputs, and arc-length accuracy.
- Physics determinism, energy/force consistency under declared assumptions, and repeatable fixed-step behavior.
- Document round trips, migration, invalid input, and undo/redo invariants.
- Rendering initialization, surface/resize paths, selection integration, and manual visual/runtime checks.
- Real workflows from project creation through editing and save/reopen, not just individual helper functions.

Do not weaken, delete, or bypass failing tests merely to obtain green checks. Fix the defect or clearly describe the blocker. Never fabricate timings, benchmark numbers, screenshots, test counts, or manual validation results.

## 14. Git and change safety

- Inspect `git status`, branch, and relevant history before editing. Respect uncommitted human work.
- For major feature passes, prefer an isolated feature branch or worktree when supported. Do not overwrite unrelated changes.
- Do not force-push, reset, rebase shared branches, delete branches, remove assets, or perform destructive cleanup without explicit permission.
- Keep commits logically coherent. Do not commit secrets, machine-specific files, generated build directories, or oversized binaries unless explicitly required.
- Do not merge into `main`, publish a release, change repository settings, or open external PRs unless the user or invoking workflow authorizes it.
- Preserve licensing, attribution, and relevant source-history information.
- Clearly distinguish work done locally from work pushed or published remotely.

## 15. Security, reliability, and dependency hygiene

- Treat external files, plugins, scripts, imported models, and network content as untrusted inputs.
- Validate file sizes, counts, bounds, numerical values, and supported versions; avoid panics and resource exhaustion on malformed projects.
- Do not add telemetry, cloud accounts, network uploads, analytics, or background services without explicit product approval.
- Keep sensitive paths, tokens, credentials, and user information out of logs and commits.
- Audit dependency licenses and maintenance status before substantial additions.
- Prefer actionable errors and graceful degradation where recovery is possible.
- Document portability assumptions and GPU/driver limitations honestly.

## 16. Documentation and feature completion

For every substantial pass, update relevant documentation when behavior or architecture changes. Explain algorithms and assumptions well enough for a new contributor or a future AI agent to continue safely.

A feature is **complete** only when its advertised user-visible behavior is wired to working code, required data flows and persistence are integrated, appropriate tests exist, and known limitations are stated. A visually impressive demo is not enough.

End substantial coding tasks with a concise, factual report containing:

1. **Implemented:** major functioning behaviors and key files/modules changed.
2. **Integration:** how systems interact and how to run the feature.
3. **Verified:** exact checks executed and their results; manual tests separately identified.
4. **Not verified / limitations:** incomplete behavior, performance risks, approximations, and external constraints.
5. **UX status:** approved behavior followed, developer-only UI introduced, and unresolved human design choices.
6. **Git status:** branch/commit/PR references, if actually created, and whether anything was pushed.

When unable to complete a requested feature, leave the repository in a buildable state if feasible, accurately describe what remains, and never present a partial implementation as finished.

## 17. Things agents must not do

- Do not rename, rebrand, or redirect QUANTUM-X without explicit instruction.
- Do not convert it into a NoLimits 2-only tool or a park-management simulator.
- Do not substitute attractive mockups for functioning software.
- Do not autonomously finalize UI/UX or override human-authored designs.
- Do not mistake generated test tracks or automated shape fitting for the user's own coaster design; human authoring stays in control.
- Do not take over the active desktop without explicit permission or retry computer control after cancellation.
- Do not build a massive single-file application to satisfy a one-shot prompt.
- Do not overengineer empty architecture while delaying visible functionality.
- Do not fake coaster physics, numerical precision, or performance claims.
- Do not interpret successful compilation as proof of simulation accuracy.
- Do not promote placeholder assets or speculative future catalog items as release-ready.
- Do not abandon useful existing code solely because another stack or AI model looks fashionable.
- Do not claim tests, runtime checks, commits, or deployments that did not occur.

## 18. Working definition of success

QUANTUM-X succeeds when a person can efficiently create, precisely edit, inspect, simulate, save, reopen, and share a coaster project in a dependable native application—and when future contributors can extend that application without repeatedly breaking its foundations.

**Human-designed experience. AI-assisted implementation. Verifiable engineering.**
