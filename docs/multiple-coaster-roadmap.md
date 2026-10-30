# Future multiple-coaster architecture

Owner decision, 2026-10-29: multiple-coaster authoring is planned **after Alpha 1
or Alpha 2**. Initial Alpha projects contain one coaster. No coaster-management
UI or collection migration is implemented in this pass.

Today `Project` owns one `initial` pose, ordered `sections`, `heartline`, `style`,
`construction` and a set of alternative `trains`. Multiple train configurations
are not multiple coasters. Metadata name/description serve the single coaster
and project together. Physics playback, generated meshes, cameras and editor
drafts remain separate session state.

When the product work is approved, extract those existing coaster-owned fields
into a `Coaster` aggregate, add a stable `CoasterId`, and store a bounded collection
in the project alongside shared environment data. Preserve project identity;
give the legacy coaster a stable identity during an explicit schema migration.
Separate project name/description from coaster metadata at that boundary rather
than duplicating fields now. Keep schema 1–5 fixtures and migration tests.

Scope section/train/device identifiers to their coaster (or use composite
references), and target commands with `CoasterId`. Keep each aggregate's internal
references valid transactionally. An active coaster is editor state rather than
an implicit index persisted into geometry. Group transactions spanning coasters
at the project level; retain coherent undo/redo.

Generation caches should use coaster identity plus revision/options. Physics
instances, device operating state, telemetry and playback workers should be
coaster-scoped, while the shared scene composes their render instances with
environment assets. Current geometry/physics APIs already consume track/train
data without editor widgets, so they need no speculative rewrite now.

Before introducing collections, the owner must design coaster selection,
visibility, cross-coaster editing, shared placement, simulation coordination and
save/history behavior. The current Editor remains focused on one meaningful
coaster workflow. This document is an architectural recommendation, not approval
of a new UX or a promise of a particular release date.
