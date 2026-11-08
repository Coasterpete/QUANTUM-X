# NoLimits 2 — Diagnostic Test Fixtures

## Purpose

This directory contains NoLimits 2 Professional exports used to test **QUANTUM Dynamics**, the Rust-based roller coaster design and simulation application.

These files are development and debugging references, not native QUANTUM Dynamics projects.

The primary objective is to test complete roller coaster layouts without manually recreating their geometry using the Region editor.

## Track CSV files

NoLimits 2 Professional track CSV exports are intended for testing:

- Track centerline geometry
- Position and orientation information
- Banking, inversions, and elevation changes
- Track construction and rail generation
- Cross-tie placement
- Materials and rendering
- Camera fitting and navigation
- Numerical and geometric diagnostics

The actual supported columns and coordinate conventions must be established by
examining the export specification and real source files. This setup does not
establish a supported CSV schema, units, axis directions, or orientation format.

## Support XML files

NoLimits 2 Professional supports XML exports are intended for testing:

- Support nodes
- Beams and columns
- Cross-bracing
- Rail connectors and attachments
- Beam-node relationships
- Footer and foundation positions
- Support dimensions and properties
- Support-to-track alignment
- Diagnostic support materials and rendering

The XML may contain nested support structures and prefab information requiring
special handling. These relationships must be investigated rather than assumed
to be a flat list of independent beams.

## Organization

Each coaster should have its own directory containing matching exports.

Example:

```text
nl2/
├── README.md
├── coaster-a/
│   ├── track.csv
│   └── supports.xml
└── coaster-b/
    ├── track.csv
    └── supports.xml
```

CSV and XML files in the same directory should originate from the same NoLimits 2 coaster and export state.

Do not assume files with different origins are compatible, even if their names
are similar or they share a directory. Preserve original filenames unless the
owner explicitly requests renaming. The coaster directories and filenames shown
above are examples only; do not create fictional data or empty placeholder exports.

## Terrain-based layouts

Most of the available test layouts are terrain-based.

- Preserve original three-dimensional XYZ positions and elevation differences.
- Do not flatten track elevations.
- Do not automatically relocate the track to world origin.
- Do not assume ground height is zero.
- Preserve exported support footer and foundation positions.
- Do not automatically extend supports to imaginary ground.
- Terrain geometry might not be available during M1.2.

The absence of a terrain surface does not automatically make track or support
geometry invalid.

## Diagnostic-only limitations

These fixtures are intended for diagnostic importing, rendering, compatibility testing, and numerical verification.

Imported CSV and XML files are read-only references. They must not automatically
be converted into:

- Editable QUANTUM Dynamics Regions
- Native transition profiles
- Reconstructed NURBS curves
- Native editable support documents
- Physically validated vehicle simulation data

Do not modify or overwrite the original exported files.

Diagnostic importing and native roller coaster authoring are separate workflows.
Imported geometry must remain separate from authored QUANTUM Dynamics project data.

## Privacy and Git policy

These exports may contain personal or third-party coaster designs.

Do not automatically commit, publish, redistribute, or include them in release packages.

Original CSV/XML exports are local development inputs unless the owner explicitly
approves publication. They must not be automatically committed to GitHub or
redistributed. Obtain explicit approval before adding individual exported layouts
to Git.

Synthetic or specifically approved fixtures may be committed for automated regression tests.

The root `.gitignore` excludes `.csv` and `.xml` files at any depth under this
directory, while this README remains trackable. These rules do not ignore CSV/XML
files elsewhere. Ignore rules do not untrack previously committed files: report
any tracked personal exports and obtain owner approval before untracking them.
Do not stage, commit, or push personal coaster exports as part of fixture setup.

## Future development

During M1.2, use these files to investigate:

1. NoLimits 2 CSV parsing and coordinate conventions.
2. NoLimits 2 support XML parsing and node connectivity.
3. Track and support alignment.
4. Track construction, support geometry, materials, and camera behavior in rendering.
5. Compatibility with QUANTUM Dynamics' partially implemented support systems.
6. Limitations of imported terrain-based layouts without a terrain surface.

Terrain import and terrain-authoring tools remain separate future work. This
directory setup does not implement M1.2 or CSV/XML importing.

Report unsupported features and alignment discrepancies rather than silently altering the source data.

**These are diagnostic references, not replacements for QUANTUM Dynamics' native track and support systems.**

The M1.2 diagnostic CLI, measured compatibility findings, unsupported properties,
and local-only rendering commands are documented in
[`docs/m1.2-nl2-import.md`](../../../docs/m1.2-nl2-import.md).
