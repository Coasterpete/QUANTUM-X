# Region terminology and editor corrections — 2026-11-02

Owner-directed corrections within the existing editor composition:

- Track navigation, properties, insertion menus, graph tools, accessibility
  labels, tooltips, status and validation messages use **Region**. New projects
  and inserted Regions receive `Region 1`, `Region 2`, etc. Existing saved names
  remain verbatim. Internal Section types, stable IDs, widget/layout keys,
  diagnostic codes, serialization fields and numerical algorithms are unchanged.
- Attempting navigation/insertion with a Region draft opens a centered modal.
  **Apply & continue** validates and commits that draft, then performs the pending
  action. Apply and insertion remain separate undo transactions. Invalid Apply
  retains the draft, document, history and modal, with the error in the modal.
  **Cancel draft & continue** discards only that Region draft and proceeds;
  unrelated staged settings survive. **Keep editing**, Escape and backdrop
  dismissal retain the draft without inserting. Keep editing receives initial
  keyboard focus; editor shortcuts cannot operate behind the modal. The decision
  no longer expands the bottom status area.
- The shared text helper no longer adds pale per-glyph backgrounds. Dark blue
  and purple small labels use the existing neutral foreground for contrast.
  Exact accent values remain on icons, rules, branding and graphs. Active
  selection fills, focus borders and text selection styling remain in place.

No new developer UI, layout redesign, document migration, dependency, or pending
owner design decision is introduced by this correction. Physics, acceleration
authoring, train inspection, PBR and support construction retain their existing
implementations. Work remains local and uncommitted; no publishing is involved.

## Verification

Headless egui tests exercise the actual workbench widgets at 1440×960 and
960×640: all three modal buttons, insertion before/after, invalid Apply,
non-destructive dismissal, blocked shortcuts, precision preservation,
undo/redo, unrelated drafts and document/geometry round trips. Paint-state tests
check transparent ordinary text backing, readable foregrounds, exact palette
values and intentional selection/focus styles. These tests use in-memory input;
they never control a desktop or launch the application.

```powershell
cargo test -p quantum-x --bin quantum-x --locked
cargo test -p qx-document -p qx-geometry --locked
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --release -p quantum-x --locked
git diff --check
```

Logs are under `target/region-ux-verification`. The resulting executable is
`target/release/quantum-x.exe`. No application launch, recording, desktop
automation or native visual/manual verification is performed in this pass.
Workspace integration tests that spawn the application are excluded.

Results: 166 application unit tests and 93 document/geometry tests passed.
Formatting, workspace check, Clippy with warnings denied, Release build and
diff whitespace checks passed. Existing physics, acceleration, train and PBR
source files were checked against entry hashes and remain unchanged; support
changes in this pass are terminology-only.

Optional owner checks on a future manual run: inspect Region labels and ordinary
text, exercise each insertion choice, then undo/redo and save/reopen.
