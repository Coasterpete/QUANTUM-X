# M7 save, drafts and history corrective pass

Authority: the owner's M7 Save, Draft Management & Undo/Redo corrective brief
and the native Windows acceptance findings enumerated in that brief. No recording
file was available in the repository; this report does not claim a video review
or a manually operated desktop test.

## Findings and corrections

The old Save/Save As and Undo/Redo draft checks were deliberate data protection:
drafts were outside document history, and the viewport could show a draft rather
than the committed document. Removing the checks would have permitted stale saves
and lost edits. The confusing behavior was the dead end: only an ephemeral status
message explained the block, disabled history controls offered no resolution, and
the construction Apply action required scrolling through the Coaster controls.

Open already checked unsaved work **after** file selection. An accessible picker
was not evidence that replacement could silently discard work. The prompt now
also precedes the picker. New and native window close already checked committed
changes and all five draft sources; those checks are retained and their prompts
now use modal protection and list the pending drafts.

Corrected behavior:

- Save and Save As list section/graph, construction, initial conditions, metadata
  and reference drafts, including their owning panels. **Apply All & Save**
  validates the combined candidate (document, track connections, reference/mesh
  generation) before committing a single existing document transaction. A failure
  applies none of the drafts and writes no file. Valid drafts are not silently
  cherry-picked out of an invalid candidate.
- **Keep editing / Cancel** dismisses the save resolver and preserves all drafts.
  There is no discard-and-save action. After successful Apply All, cancelling the
  native Save As picker retains the applied edits as unsaved committed changes.
- A persistent toolbar **Drafts…** action opens the same resolver from either
  workspace, without scrolling to Apply. Its tooltip lists the outstanding
  sources. Affected dock tabs and floating panel titles say **unapplied edits**.
- Undo/Redo remain accessible with drafts and explain why resolution is needed.
  **Apply all drafts** commits once and stops: the user can then choose Undo to
  undo that transaction. It does not immediately undo its own Apply. The dialog
  explicitly explains that applying changes clears Redo. **Cancel drafts &
  continue history** explicitly discards drafts and executes the requested
  history operation. Failed validation preserves both history stacks.
- Open's pre-picker **Discard and continue** authorizes replacement only after a
  file is selected and successfully loaded/generated. Cancelling the picker or
  failing to load retains the current document and drafts. New and Exit retain
  explicit keep/discard choices. Keeping the project allows the user to save it
  before trying replacement/close again.
- Save/file/history commands from widgets and shortcuts are dispatched after
  numeric widgets consume the input batch. Typing and Ctrl+S together cannot save
  the preceding numeric value. Project shortcuts cannot bypass active prompts.
- The state indicator distinguishes unapplied drafts, unsaved committed changes,
  their combination, save failure and Saved. The saved baseline/path changes only
  after successful disk persistence. A loaded file supplies the initial known
  disk baseline. Save failures have a persistent error line independent of
  asynchronous generation messages; retry success or successful replacement
  clears the error. Failed saves retain committed edits for retry/undo.

## Scope and integration

`apps/quantum-x/src/authoring/draft_workflow.rs` implements shared draft discovery,
validation, single-batch application and resolution. `session.rs`, `files.rs`,
`commands.rs`, `panels.rs` and `layout_ui.rs` connect it to the existing editor.
`save_tests.rs` contains the corrective regressions. No mathematical evaluator,
construction algorithm, renderer, schema, dependency or document transaction
implementation was changed by this corrective pass. Existing uncommitted M7 work
was retained.

The existing schema-4 persistence implementation is retained: validate and encode,
write a newly created sibling temporary file, sync and close it, then rename over
the destination. Failure cleans the temporary file and leaves the prior target
intact. This is atomic replacement, not a promise of directory-entry durability
against every power-loss/filesystem scenario. Save As retains the old identity
until the selected destination is successfully written; Unicode paths are tested.

Preflight generation runs synchronously only at explicit Apply All boundaries,
as existing section acceptance does. Large projects may pause at acceptance;
this pass makes no new throughput or frame-rate claims. Drafts remain in memory
until applied and saved; crash recovery/autosave is outside this request.

## Verification

The 15 added regressions cover separate and simultaneous graph/construction
drafts; actual modal buttons at 960×640 and 1600×1000; invalid graph, construction
and strict downstream joins; all five draft sources; Cancel; single transaction
Undo/Redo; Redo preservation/replacement; Save As completion/cancellation; Unicode
paths; Open/New/Exit guards; failed/cancelled Open; missing-parent writes; Windows
locked-target replacement failure and temporary-file cleanup; persistent failure
status; complete schema-4 project, centerline and mesh round trips; and real egui
numeric text plus Ctrl+S in one input batch.

Run from the repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
.\target\release\quantum-x.exe authoring-scenario target/m7-save-correction/m6
python tools/verify-m6-exports.py target/m7-save-correction/m6
.\target\release\quantum-x.exe track-scenario target/m7-save-correction/m7
python tools/verify-m7-exports.py target/m7-save-correction/m7
```

The scenario commands exercise automated offscreen GPU rendering; they do not
take control of the owner's desktop. Generated evidence stays under ignored
`target/m7-save-correction/`.

Results on native Windows for this pass:

| Check | Result |
| --- | --- |
| Formatting, workspace check, Clippy with warnings denied | Passed |
| Workspace tests | 167 passed, 0 failed; includes 15 corrective regressions |
| Workspace Release build | Passed |
| M6 scenario and independent verifier | Passed; 901 samples, 90 m, 8 PNGs |
| M7 scenario and independent verifier | Passed; 14 fixtures, 40 PNGs including live-edit workflows |
| `git diff --check` | Passed |
| Native manual interaction | Not performed |

## Owner-executable native acceptance checklist

**Not manually verified.** Start the rebuilt application:

```powershell
.\target\release\quantum-x.exe edit target/m7-save-correction/m7/d-dual-hardware.qx.json
```

1. Stage a graph numeric edit and a construction strategy/dimension/material edit.
   Confirm the panel titles and toolbar Drafts action identify unapplied edits.
2. Save. Confirm both drafts are listed. Cancel and confirm both previews/values
   remain. Save again, Apply All & Save, reopen and compare every authored value.
3. Repeat with an invalid construction dimension, then an invalid section value.
   Confirm validation leaves the dialog open, document/history unchanged and the
   previous disk contents intact. Keep editing, correct the inputs and retry.
4. Undo once after applying both drafts; both changes must undo together. Redo
   must restore both. With new drafts pending, try Undo/Redo and test both resolver
   paths, including the explanation of Redo invalidation when applying.
5. Save As to a Unicode path. Cancel the picker on a second attempt and confirm
   the original path remains current and any applied changes remain unsaved.
6. Try Open, New and native window close with committed changes, drafts, and both.
   Keep the project each time. For Open, confirm the warning precedes the picker;
   explicitly continue, cancel the picker and verify edits remain. A failed Open
   must also retain them. Only an explicit discard may replace/close the project.
7. Try a destination Windows refuses to write. Confirm Save failed remains visible
   through generation updates. Retry a writable path and confirm Saved appears
   only after success. Check native picker focus, modal focus, DPI/scaling and
   small-window usability; automated egui tests are not native acceptance.

## UX and Git status

The implemented save resolver, visible draft state, history guidance and
replacement protection follow the owner's explicit corrective request within the
existing workbench. No developer-only UI or new workflow screen was introduced.
Exact wording/spacing and native comfort remain reviewable presentation choices;
no final visual identity was inferred. No outstanding product decision blocks
this correction.

All changes remain local and uncommitted on
`codex/m7-procedural-track-engine`. No commit, push, PR or merge was performed.
