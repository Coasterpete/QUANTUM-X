# Approved Windows application icon

The official executable icon is the owner-approved original
`assets/icons/quantum-dynamics/QUANTUM_Dynamics.ico`. Its seven original PNG frames
are 16, 24, 32, 48, 64, 128 and 256 pixels square. Do not regenerate, recolor or
redesign it as part of a build. The Blender model, rendering script and PNG source
artwork in that directory are preserved; Blender is not a build dependency.

`apps/quantum-x/build.rs` adds `101 ICON` to the existing `embed-resource` Windows
resource compilation alongside the unchanged version metadata. Cargo watches the
source ICO and rebuilds resources when it changes. An unchanged copy is staged in
Cargo's output directory for resource compilation; it is not a runtime asset.

Both the native workbench and development inspector obtain window attributes from
`qx-rendering::identity::window_attributes`. On Windows this loads resource 101
with winit's `IconExtWindows::from_resource`, sets the window icon and explicitly
sets the large taskbar icon from the same resource. Loading errors propagate
through the existing startup error path. Other platforms retain their existing
window attributes. No document, CLI dispatch, track, train or physics behavior
changes. Windows Explorer also obtains its executable icon from this resource.

The resource uses the standard Windows [ICON declaration](https://learn.microsoft.com/en-us/windows/win32/menurc/icon-resource).
The taskbar uses winit's [Windows window attributes](https://docs.rs/winit/0.30.13/winit/platform/windows/trait.WindowAttributesExtWindows.html#tymethod.with_taskbar_icon).

## Reproducible distribution and checks

From the workspace root on native Windows with Rust/MSVC and the Windows SDK:

```powershell
./tools/package-windows.ps1
# Existing destinations are refused. Use a new destination for another build:
./tools/package-windows.ps1 -OutputDirectory target/my-icon-distribution
./tools/verify-windows-icon.ps1 -Executable 'target/my-icon-distribution/QUANTUM Dynamics.exe'
```

The locked Release build still produces `quantum-x.exe`. Packaging stages both
`QUANTUM Dynamics.exe` and the compatible development CLI from that same PE.
The embedded icon needs no loose ICO/PNG, Blender files or rendering scripts in
the distribution. The original artwork remains in the repository assets tree.
This is a repeatable build/staging process, not a claim of bit-identical builds.

Packaging verifies the build artifact before staging and both packaged executable
names afterward. `verify-windows-icon.ps1` uses Windows resource APIs with the PE
loaded as data; it never executes that PE or creates a window. It compares the
`RT_GROUP_ICON` 101 directory (allowing the resource compiler to normalize the
PNG frames' unspecified plane count from 0 to 1) and every referenced `RT_ICON`
payload byte for byte with the source ICO, then exercises Windows' icon decoder
at 32 and 256 pixels.
Missing, stale, different or undecodable resources fail packaging. The script
needs only Windows PowerShell/.NET, with no additional runtime dependencies.

`icon-check.json` records both packaged checks, frame sizes and source SHA-256.
`distribution.json` also records the icon resource ID and source SHA-256. Existing
version/catalog/CLI launch checks remain enabled. A hash change in the approved
ICO is reflected automatically; no generated or hardcoded replacement is used.

## Owner-executable manual check

Automated resource checks do not prove how a particular Windows shell cache or
desktop theme displays an icon. No interactive desktop control is needed for the
checks above. To check native presentation manually:

1. Open the new distribution folder in Explorer and inspect `QUANTUM Dynamics.exe`.
2. Open that executable in Explorer, or run
   `& './target/my-icon-distribution/QUANTUM Dynamics.exe'` in PowerShell.
   Inspect the native title-bar, taskbar and Alt+Tab icon.
3. Close the application, then run
   `./target/my-icon-distribution/quantum-x.exe --help` to check the development CLI.
4. Run `./target/my-icon-distribution/quantum-x.exe inspect <project.qx.json>` and
   check the same icons in the existing development inspector.

Owner-approved UX change: apply the supplied original icon to executable/window/
taskbar identity. No developer UI, new workflow, visual redesign or unresolved
UX design decision is introduced by this integration.

## Integration verification (2026-10-29)

Passed on native Windows:

- `cargo fmt --all -- --check`
- `cargo check --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`: 226 passed, zero failed or ignored.
- `cargo build --workspace --release --locked`
- `./tools/package-windows.ps1 -OutputDirectory target/quantum-icon-verification/distribution`:
  locked Release build, icon checks on the artifact and both staged executables,
  public version/catalog self-check and legacy CLI help all passed.
- A second packaging run under Windows PowerShell 5.1 into
  `target/quantum-icon-verification/repeat distribution` also passed.
- The verifier rejected the previous executable without resource 101, an ICO
  with an altered image payload and an invalid empty ICO directory.
- All seven original source/artwork files matched their pre-edit SHA-256 hashes.
- The packaged public self-check and legacy CLI help passed from outside the
  repository, resolving the adjacent packaged assets.
- `git diff --check`

Both packaged names contain group icon 101 and all seven original PNG image
payloads, byte-identical to the approved ICO. Windows decoded the embedded icon
at both checked sizes. Evidence is in each distribution's `icon-check.json` and
`distribution.json`; generated packages remain under Git-ignored `target/`.
The original ICO SHA-256 is
`C1B8F3571FCCA5A95A9D63ED20EB247159B91554395E345C6BE74862C5830A22`.

Explorer/title-bar/taskbar presentation was **not manually verified**. No desktop
control occurred; use the manual checklist above. No commit, push or PR was made.
