# Track and Train Library contracts

## Identity, namespaces and ownership

`qx-style::Catalog` remains the sole catalog model. `assets/catalog/styles.json`
is its version 2 index, compiled into the application as fallback metadata and
loaded from disk at normal editor startup. Version 1 catalogs still deserialize;
`concept`, `experimental`, and `released` map to the canonical `planned`,
`in_development`, and `available` statuses. Existing `hybrid-100`, `monorail-100`
and `monorail-200` track IDs are unchanged. Schema 5 projects need no migration
for this pass; existing schema 1–4 migration remains in `qx-document`.

Track and train IDs occupy separate namespaces. `LibraryKind` accompanies service
lookups, so track `hybrid-100` and train `hybrid-100` are distinct. A numbered style
has a positive number unique within its family. A named train style has `number:
null`; families without authored styles receive no invented default style.
IDs contain lowercase ASCII letters, digits and interior hyphens, up to 128 bytes.
Directory names are explicit metadata and can retain parentheses independently
of IDs. Never derive mechanics, identity, dimensions or compatibility from paths.

`families` and `styles` retain the existing track model. `train_families` and
`train_styles` extend it. Legacy `train_ids`, inline track `geometry`,
`track_asset`, and preview references remain readable. New indexed content uses
explicit `directory` references and style manifests. Legacy metadata without a
directory stays valid for project loading but is not an installed library style.

## Authored directory mapping

Track family IDs are `backboned`, `box-spined`, `hybrid`, `monorail`,
`traditional-wood`, and `truss`. Their 17 numbered directories are retained
verbatim, including `100(topper-track)`, `200(steel-ibeam-track)`,
`100(prefabricated-track)`, and `200(traditional)`. The complete mapping lives in
the index; directory strings include the `tracks/` or `trains/` prefix and are
relative to the assets root.

Train family IDs are `4d`, `floorless`, `flying`, `free-spin`, `hybrid`, `inverted`,
`monorail`, `motorcycle`, `motorcycle-suspended`, `sit-down`, `spinning`,
`stand-up`, `suspended`, and `wing`. Numbered styles use `family-number` IDs.
The named style IDs are `floorless-dive-machine`, `xtreme-spinning`, and
`xtreme-spinning-floorless`, preserving the owner's terminology. A family is not
a train or a validated simulation configuration. Controlled 4D, free-spin,
flying, inverted and suspended motorcycle categories remain distinct.

`.gitkeep` preserves the 36 otherwise empty leaf directories. No style manifests,
cross-sections, train bodies, textures or previews were manufactured to fill them.
`assets/development/train/` retains the original diagnostic GLBs separately.

## Style manifest version 1

Register a directory in the index, then place an actual `style.json` inside it.
Unindexed directories are ignored. The index owns name, description, ID, family,
status and compatibility; the manifest owns content. An incomplete train
manifest can look like the following **documentation example**, not installed
content:

```json
{
  "version": 1,
  "kind": "train",
  "style_id": "motorcycle-suspended-100",
  "visuals": [],
  "previews": { "thumbnail": null, "large": null, "references": [] },
  "construction": null,
  "mechanical_definition": null
}
```

For finished content, `visuals` holds records with a stable `id` and a
style-relative GLB `path`. Preview metadata holds a thumbnail, larger image and
optional reference angles. All referenced files must remain within the style's
directory, including through Windows junctions/symlinks.

Track manifests use `kind: "track"` and an explicit, validated M7
`TrackConstruction` in `construction`. Its serialized format is the same as a
project's `construction` field. Arbitrary GLB references are metadata, not a new
track-mesh rendering implementation. A ready track requires supported procedural
construction; a standalone mesh reference cannot satisfy that requirement.

Train manifests use `kind: "train"` and a `mechanical_definition` path to a
standalone serialized `qx_document::train::Train`. Its SI component masses,
principal inertia, joints, mounts, guides and visual transforms use the existing
M8.2 definition format. No mechanical defaults come from a category or model.
Component `asset.path` values refer to declared `visuals` paths. The application
adapter must import/validate each GLB and package it relative to the project
before committing a train. Source library paths do not become absolute saved
project references.

Unknown fields, unsupported versions, duplicate IDs/numbers/directories,
family-directory mismatches, unknown compatibility targets, invalid geometry and
path traversal are rejected. JSON readers enforce a 4 MiB per-file limit. Windows
reserved device names, trailing dots/spaces, absolute paths and control characters
are rejected. A corrupt individual manifest is an entry diagnostic; it does not
prevent inspecting other entries. A corrupt index produces a visible editor
warning and built-in metadata fallback; the strict CLI check fails.

## Services and readiness

- `Catalog::from_json`, `validate`, `train_style`, `styles_for_family`, and
  `compatibility` handle validated, serializable metadata without UI/GPU types.
- `Library::open(assets_root)` reads the index and explicitly registered manifests.
  `Library::from_catalog` resolves an existing metadata snapshot against a root.
  Entry reports sort by kind and stable ID, independent of OS directory ordering.
- `Library::asset(kind, id, reference)` resolves only declared content paths.
  Canonical containment prevents junction/symlink escapes; missing/corrupt content
  is reported, not substituted or generated.
- `EntryState::content_ready` requires declared `available` status, a matching
  manifest, all referenced files, both previews, and the required content
  definition. It describes resolved content, not validated image/GLB bytes,
  mechanical accuracy, compatibility certification or release approval.
- `qx_document::library::load_train_definition` additionally parses and validates
  the actual mechanical definition, rejects conflicting compatibility metadata,
  and returns a definition with canonical visual sources for the import adapter.
  Its output retains all authored mechanical values. The existing GLB importer
  remains responsible for model/texture validation.

The disk snapshot is loaded at editor startup, outside the interactive rendering
path. Restart after editing the index/manifests; there is no file watcher or
background service. `QX_ASSETS_ROOT` explicitly overrides the root. Packaged
executables use adjacent `assets/`, independent of the current working directory.
Development executables fall back to the workspace assets directory.

```powershell
./target/release/quantum-x.exe library
./target/release/quantum-x.exe library C:/path/to/assets
./target/release/quantum-x.exe --self-check
```

The strict check also parses mechanical definitions for content-ready trains.
All current entries are planned and therefore report `content_ready: false`.

## Compatibility and project persistence

Train declarations use the existing `VehicleCompatibility`. Track allowlists
reference known train style IDs. Train allowlists reference known track style
IDs; optional required features are assessed against the track's feature tags.
An explicit restriction on either side wins; conflicting lists or missing
required features report incompatible. Empty declarations mean unspecified.
Equal style numbers or matching family IDs contribute no compatibility evidence.
No current track/train pair has a positive compatibility declaration.

The document remains authoritative: track selection persists in `Project.style`,
actual construction in `Project.construction`, and instantiated train association
in `Train.compatibility.train_style_id`. Actual train mechanics and project-relative
visual assignments persist in the existing train records. Catalog snapshots,
availability, absolute library paths and GPU data are not saved into projects.
Unavailable catalog content does not discard saved definitions or assignments.
`SetStyle`/`SetConstruction` and `SetTrains` retain existing atomic undo/redo.

## Editor boundary

Existing Coaster Properties now exposes the indexed track families, explicit
status, paths, diagnostics and compatibility. Planned entries have disabled Apply
controls. A content-ready procedural track can apply its construction and style
selection as one transaction, preserving the authored centerline.

Train Configuration contains a collapsed **Train Library · development inspection**
group showing the owner's public families and styles. It is read-only: this pass
does not introduce an unapproved train-installation workflow. Backend train
loading, GLB assignment, save/reopen and force-integrated stepping are exercised
end to end by synthetic tests using the existing services. Existing development
fixture selectors, reference axes, residuals, playback and telemetry remain.

Final browsing, thumbnails, property-editing layout, insertion/preview workflows,
toolbars and professional visual design require owner direction. This foundation
pass did not create an approved final library browser. The later
[welcome/startup pass](welcome-startup.md) adds provisional sample/template and
catalog navigation while retaining these services and planned statuses.
