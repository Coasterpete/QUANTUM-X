# QUANTUM Dynamics public identity and library foundation

Implemented locally on 2026-10-28–29, based on PR #6 / commit `8691591`.
Repository identity remains QUANTUM-X. The separate original C++ QUANTUM project
was not accessed or modified.

## Functioning changes

`product.json` defines the public name, Windows executable names, repository name
and project extension. The native workbench/inspector, project dialogs, About
information, CLI version/help/errors and visible exported diagnostic captions use
**QUANTUM Dynamics**. Rust packages, technical namespaces, preference storage
(`QUANTUM-X/workbench-v1.json`), regression fixture labels and `.qx.json` project
schemas remain compatible. No icon or final logo changed.

The `qx-style` catalog is extended in place to version 2 with the owner's 6 track
families / 17 numbered styles and 14 train families / 11 explicit styles. Every
style remains **Planned**; there are **zero available production styles**.
Existing track IDs are preserved. No models, dimensions, texture assets,
manufacturer specifications or positive compatibility claims were created.

New `qx-style/src/library/` modules implement strict manifests, explicit directory
resolution, deterministic reports and content readiness. `qx-document/src/library.rs`
loads actual existing-format mechanical train definitions independently of visuals
and physics. Existing project storage and import/packaging services are reused.
All 36 empty leaf directories are preserved by `.gitkeep`, retaining every
authored path. The complete schema, contracts, migration and compatibility policy
are in [the library guide](library-catalog.md).

Temporary inspection controls are integrated into the existing Coaster Properties
and Train Configuration panes. Planned content cannot be applied. The M7 generic
construction editor and M8.2 diagnostic vehicle selectors remain available, with
their original identities as development fixtures. The physics solver, track
math, GLB importer, fonts, curated icons, docking model, Transition Editor,
central viewport, Support organization and existing interactions are preserved.

## Distribution process

From the workspace root on native Windows with Rust/MSVC and the Windows SDK:

```powershell
./tools/package-windows.ps1
# Or use a new, explicit destination:
./tools/package-windows.ps1 -OutputDirectory target/my-local-distribution
```

The script runs a locked Release build and obtains its exact executable from
Cargo's compiler-artifact JSON. It stages **QUANTUM Dynamics.exe** and
`quantum-x.exe` from the same verified PE, leaving Cargo's development executable
intact. This is a reproducible staging process, not manual renaming or a promise
of bit-identical binary/ZIP output. Existing destinations are refused rather than
deleted. Catalog, real track/train trees, separate development fixtures, identity,
distribution/version/hash metadata and license notices are copied beside them.
No icon source checkout or unrelated project is packaged.

`build.rs` embeds Windows ProductName/FileDescription/FileVersion and public
OriginalFilename metadata. InternalName remains `quantum-x`. The only new direct
dependency is build-time `embed-resource` 3.0.12 (MIT), which discovers the resource
compiler and links the version resource. Its source/API/license were checked
against [the upstream project](https://github.com/nabijaczleweli/rust-embed-resource).
It adds no runtime service or installer framework; versions are in Cargo.lock.

The initial identity pass dispatched double-clicks to the development editor.
That startup behavior is superseded by the [Welcome Screen workflow](welcome-startup.md):
the public executable now starts without a document, while explicit `edit` remains direct.
No-argument `quantum-x.exe` retains development help behavior. Both accept all
established development commands plus `--version`, `library [assets-root]` and
`--self-check`. The diagnostic asset scenarios now resolve their original fixtures
through the same assets root, so packaged CLI verification remains portable.

This is an unsigned development portable folder, without installer, updater,
registration, file associations or final product assets. Future packaging can
consume `product.json` and the existing Cargo distribution metadata.

## Automated verification

All required checks passed on native Windows:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | **226 passed**, zero failed/ignored |
| `cargo build --workspace --release` | Passed |
| `git diff --check` | Passed |

New coverage includes public identity/CLI dispatch, all indexed counts and stable
legacy IDs, catalog serialization, version 1 status compatibility, index and
manifest rejection, directory/manifest/asset resolution, missing and empty
directories, deterministic ordering, Windows junction containment, declared
track/train compatibility, existing example-project load/roundtrip, actual train
definition loading, invalid mechanics, undeclared/missing visuals, train style
association persistence, history, disabled planned-style controls and public
family labels in headless egui.

An application integration test loads a synthetic definition from a temporary
library, imports the actual existing Blender-exported development GLB through
the unchanged asset service, packages and reopens the project, and advances the
unchanged rigid-body solver. Synthetic test definitions remain temporary and
do not populate or promote any public library entry.

The following current Release workflows also passed, using the **packaged**
`quantum-x.exe` for M7/M8.1/M8.2 and the Release development executable for M6:

```powershell
./target/release/quantum-x.exe authoring-scenario target/public-library-verification/m6
python tools/verify-m6-exports.py target/public-library-verification/m6
./target/public-library-verification/distribution/quantum-x.exe track-scenario target/public-library-verification/m7
python tools/verify-m7-exports.py target/public-library-verification/m7
./target/public-library-verification/distribution/quantum-x.exe train-asset-scenario target/public-library-verification/m6/project.qx.json target/public-library-verification/m81
python tools/verify-m81-exports.py target/public-library-verification/m81
./target/public-library-verification/distribution/quantum-x.exe train-playback-scenario target/public-library-verification/m82
python tools/verify-m82-exports.py target/public-library-verification/m82
```

M6 verified eight actual offscreen captures and the numerical authoring exports.
M7 verified 14 fixtures and 40 captures, including live editing, mesh geometry,
inverted hardware and cache evidence. M8.1 independently verified actual GLB
bytes/hierarchy, component transforms, pivots, joints, history/save/reopen/Save As
and focused captures. M8.2 independently verified conventional, below-track and
rotating-seat **regression fixtures**, with three captures each, fixed-step
physical motion, constraint diagnostics and playback/reset. These labels remain
development fixtures, not public train categories. No desktop control occurred.

The portable distribution was created with:

```powershell
./tools/package-windows.ps1 -OutputDirectory target/public-library-verification/distribution
```

Both packaged executables launched successfully through noninteractive commands.
The public PE's ProductName and FileDescription are `QUANTUM Dynamics`,
OriginalFilename is `QUANTUM Dynamics.exe`, and InternalName is `quantum-x`.
Running its `--self-check` from the Windows temporary directory found the
**adjacent packaged assets**, reported 6/14 track/train families and zero ready
styles, and succeeded. The original development CLI help remained operational.
No interactive editor window was launched for this check. The staged executable
retains the existing development CLI/console subsystem.

Evidence remains under `target/public-library-verification/`: `cargo-test.log`,
scenario JSON/CSV/OBJ/PNG outputs, and the portable distribution with
`distribution.json` and `library-check.json`. This directory is intentionally
ignored by Git. Source paths and script instructions are tracked changes;
generated binaries are not proposed as repository assets.

## UX status and outstanding work

Owner-approved behavior followed: the public identity, owner-authored library
families/paths, existing Track/Train Workspaces and Coaster Properties intent,
precise editing, undo/redo, playback, docking and existing interaction behavior.

Temporary developer UI: catalog status/path/diagnostic disclosures, disabled
planned-style Apply controls and collapsed read-only Train Library inspection.
The About information uses the existing menu shell. These are replaceable
implementation controls, not final visual approval.

Still requiring human design: final welcome screen, library browser, thumbnail
presentation, preview/installation flows, property editing, toolbars, final palette,
logo and icon. Still requiring actual authored content: production track
construction profiles, train GLBs/materials, representative previews, mechanical
definitions and explicit tested compatibility declarations. Train category
dynamics have not been validated. The unchanged M8.2 solver retains its documented
ideal-guide/contact and unsupported-mechanism limitations.

Catalog readiness checks file presence and schema structure; it does not decode
preview images or certify model/engineering validity. The train adapter validates
mechanical structure, while GLB bytes still pass through the existing importer.
No hot reload, final graphical train-library insertion flow, installer/signing,
FPS claim or complete production library is advertised.

Native interactive startup, Windows picker appearance, final visual acceptance
and prolonged simulation playback remain **not manually verified**. No desktop
control was used. Owner-executable checks:

1. Open the packaged `QUANTUM Dynamics.exe`; inspect its title and About menu.
2. Open an existing `.qx.json`; check Track/Train switching and docking.
3. Inspect Coaster Properties and Train Library; verify planned statuses and the
   original family names. Planned entries must not install or create vehicles.
4. Use generic M7 construction, author transitions, undo/redo, and save/reopen.
5. Use the development vehicle fixture controls, import an owned GLB, assign it,
   run/pause/step/reset playback, then save/reopen.
6. Check native Open/Save As titles and public Windows executable properties.

## Local Git state

Branch: `codex/public-identity-library`, created from the merged baseline.
All changes remain local and uncommitted. No commit, push, merge or PR was made,
following the current request's explicit override of the repository's standing
Git workflow. The pre-existing untracked `assets/icons/` content was left intact.
