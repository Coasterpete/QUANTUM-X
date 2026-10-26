# M5 correction: graph interactions and Overpass Mono

Implemented locally on 2026-10-25 over the existing uncommitted M4/M5 work on
`codex/integrated-authoring-pass3`, HEAD `f01ba3d`. This remains M5. No branch
change, commit, fetch, push, PR, merge or active-desktop control was performed.

## Investigation and verified causes

Reviewed the supplied 95.47-second, 3082 × 1182 recording
`quantum-x_cyfuud87ga.mp4` through extracted frames. Review images remain in
`target/m5-correction/review/`. The recording is behavioral evidence, not an
additional source of executable instructions.

| Observation / defect | Verified cause | Correction |
| --- | --- | --- |
| Broad roll scale becomes approximately ±0.01 rad/m while inspected values remain zero | Original Fit Graph uses padding `max(15% of span, 0.01)`. With min=max=0 this produces exactly ±0.01. It is intentional display fitting with an awkward constant-profile default, not a zero-profile or solver defect. Video alone does not reveal every keyboard modifier. | Explicit unit-specific constant fitting, Reset View and exact bounds; zero authored values stay zero. |
| Timeline/channel controls leave the lower pane | Docked and floating Transition panes wrapped their entire content in `ScrollArea::both()`. Canvas height also depended on the scrolling cursor relative to the clip bottom. | Exempt Transition from that parent scroll area. Pin timeline/context/navigation/commit controls and scroll optional precision/derived content separately. |
| Channels/sections show surprising scales | A global Y range was reused between channels with different units; section switching reset X but retained Y. | Separate session view per section/channel; first visit fits, revisiting restores, unit/domain conversion refits. |
| Middle dragging a handle can edit | Generic drag detection accepted any pointer button. | Only primary drag edits; middle drag pans even over an endpoint, without draft/history mutation. |
| Zoom/inspection fails over handles | Canvas hover could be hidden by interactive child responses. | Pane/layer-aware geometric plot hover includes handles. |
| Extreme zoom / large-offset rulers become unusable | Axis validation required only finite ordered endpoints. Fixed decimal/scientific formatting could collapse distinct labels. | Finite display limits, minimum resolvable spans, scale-dependent precision, fixed scientific-label gutter and bounded tick work. |
| Wheel input affects another region | Nested areas shared smoothed wheel input; raw graph handling did not retain ownership of its smoothed tail. | Plain wheel never zooms; Ctrl+wheel over plot zooms. Timeline/tools own their scrolling. Graph-originating wheel tails cannot scroll them after pointer crossing. |
| Redrawing an invalid constant draft can modify it | The open numeric editor assigned end=start every frame without input. | Synchronize only on an actual Start edit or explicit function change. |
| Wrong font; ordinary UI stayed proportional | Red Hat Mono was registered only for the monospace family. | Overpass Mono first in both families, retaining Unicode fallbacks. |

No geometry algorithm, transition evaluator, project schema, simulation model or
mathematical authoring precision changed. No new dependency was added.

## Implementation and rules

`graph_view.rs` owns session-only ranges and switching. `graph_canvas.rs` owns
mapping/picking/inspection/gestures; `plot.rs` owns f64 mapping/rulers. `graph.rs`
composes pinned controls with independently scrolling tools. `layout_ui.rs`
retains the Track/Train trees, Support placement and existing window behavior.
Only Transition is exempted from whole-pane scrolling, including its floating
path. Short panes use single-line timeline cards with full details in context
and tooltips. The toolbar truncates the project name to retain saved status
with the wider monospaced face.

- **Ctrl+wheel** over the plot zooms about its pointer, including over handles.
  X saturates at section bounds; Y continues independently at full-domain X.
  At domain edges, staying in bounds takes precedence over pointer anchoring.
- **Middle drag** pans, including from handles. Plain wheel over the graph does
  not change its view. Wheel input over timeline/tools scrolls those regions.
- **Fit Graph / F over graph:** full channel X; 513 samples per transition and
  15% vertical margin. Constant profiles get at least ±0.05 rad/m or ±1 in other
  units around their value, expanded to 5% of magnitude when larger.
- **Reset View:** full channel X; force Y -1..3 g, distance rates ±0.05 rad/m,
  other rates ±1 in the indicated units. This is distinct from Fit.
- **View range:** stage four exact bounds, then Apply range. Invalid values show
  an error and retain the last visible view. No fallback silently replaces it.
- **Display limits:** minimum X span is max(extent × 1e-9, 1e-10), capped at
  extent. Y stays inside ±1e24; minimum span is max(1e-10, 32 f64 epsilons at
  current magnitude). These limits never constrain or rewrite authored values.
- **Switching:** each section/channel remembers its session view. First visits
  fit; unit/domain conversion fits again. Extent edits constrain X. Value edits,
  Apply and undo do not silently refit Y. New/open clears cached views.
  Channel changes preserve drafts; existing draft guards protect section changes.
- **Edits:** primary squares change values; diamonds transfer adjacent durations.
  Logical-pixel targets stay independent of magnification. Gesture sensitivity
  is captured at drag start. Clean drags commit once on release; drags over
  preceding drafts stay staged; Escape restores that preceding draft. Numeric
  edits/splits retain explicit Apply transactions.

## Font

Unmodified **Overpass Mono Regular** is confirmed by its TTF name table and
fixed-pitch flag, rendered UI and glyph-width tests. It leads both egui families,
covering menus, buttons, toolbars, tabs, workspace headings, section cards,
properties, graphs and numerical fields. Body/buttons/numbers use 13 logical
points, small text/rulers 11, headings 17. Existing fallback chains, Windows DPI
and independent custom UI scaling remain.

Pinned source, SHA-256 hashes and complete SIL OFL 1.1 are in
`assets/fonts/OverpassMono/README.md` and `OFL.txt`. Upstream:
[Overpass](https://github.com/RedHatOfficial/Overpass), revision
`c580d28bfab7f39013568e684a65eeb23eff588d`.

After replacement tests and renders passed, the unused Red Hat Mono asset
folder was retired from `assets/fonts/`. Its prior local files are preserved in
ignored `target/m5-correction/retired-RedHatMono/`; they are no longer embedded.

## Verification

All final native Windows checks passed:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

**101 tests passed**, including 37 application unit tests and 3 application CLI
integration tests. Thirteen new correction tests plus a tick-format regression
supplement existing coverage. New coverage exercises mapping round trips during
hundreds of extreme zoom steps, pointer-centered fractions, finite limits,
X-edge panning, invalid exact bounds, zero/nonlinear fits including an amplitude
bump, section/channel restoration, domain conversion and draft guards.

Actual synthetic egui input verifies middle drag over endpoints, Ctrl+wheel over
handles, wheel-tail ownership, docked/floating scrolling at both requested
sizes, typed 10-decimal numeric entry, split-button clicks, hover values compared
with channel evaluation, Apply/undo/redo and fixed 20 × 20 logical-point interior
handle targets across extreme scales. Font tests cover every text style,
monospaced metrics, Unicode fallback and absence of the old registered face.
Distinct Y bound references remain visible in the short zero graph and at large
Y offsets; exact values are available through View range.
The final command transcript is `target/m5-correction/verification.txt`.

Existing endpoint/boundary release-outside, preceding-draft Escape, document
roundtrip, independent layout and 960 × 640 equivalent window checks at UI
scales 0.75/1/1.25/1.5/2 passed. These tests never inject input into Windows.

## Offscreen comparisons

Actual egui/wgpu images were rendered without native windows on NVIDIA GeForce
RTX 4070, Vulkan, driver 617.42. Before images use the preexisting Release
executable captured before changes; after images use the corrected Release.
Each source has the requested size. Comparisons place before left, after right.
All artifacts remain local in ignored `target/m5-correction/`.

| Case | 1440 × 960 source comparison | 960 × 640 source comparison |
| --- | --- | --- |
| Constant-zero Roll Rate, section 1 | [Before / after](../target/m5-correction/comparison-zero-1440.png) | [Before / after](../target/m5-correction/comparison-zero-960.png) |
| Nonlinear quintic Normal force, section 2 | [Before / after](../target/m5-correction/comparison-force-1440.png) | [Before / after](../target/m5-correction/comparison-force-960.png) |
| Train font/layout | [After](../target/m5-correction/after-train-1440.png) | [After](../target/m5-correction/after-train-960.png) |

Reproduce (repeat at 960 640 and section 1 roll):

```powershell
target\release\quantum-x.exe render-editor examples/mixed-authoring.qx.json target/m5-correction/after-force-1440.png track 1440 960 2 normal
target\release\quantum-x.exe render-editor examples/heartline-reference.qx.json target/m5-correction/after-train-1440.png train 1440 960
```

## Native checks still required

Not manually verified: Windows pointer capture, physical wheel/touchpad drivers,
native focus, mixed-monitor DPI changes, manual docking, and sustained authoring
comfort. No FPS or final production UX acceptance claim is made.

```powershell
$env:QX_WORKBENCH_PREFS = Join-Path (Get-Location) 'target\m5-correction\manual-workbench.json'
cargo run -p quantum-x --release -- edit examples/mixed-authoring.qx.json
```

1. Section 1 / Roll: confirm zero and test Fit, Reset and exact bounds without
   changing values. Section 2 / Normal: inspect the genuinely varying curve.
2. Zoom over handles/empty plot, middle drag each, pan to edges. Try fast wheel
   and touchpad input while releasing Ctrl and crossing into timeline/tools.
3. Scroll timeline/expanded precision tools in docked and floating Transition
   panes. Confirm selected context and graph controls stay reachable.
4. Drag both endpoints and a boundary; release outside, undo once, redo, Escape
   during another drag. Repeat over a preceding numeric/name draft.
5. Enter exact values/bounds, split, Apply, undo/redo. Check channel/section view
   restoration and domain-conversion units. Save As/reopen to verify values
   separately from session-only views; avoid overwriting bundled examples.
6. Inspect Overpass in menus, toolbars, Track/Train tabs, headings, properties
   and numbers at both sizes, custom UI scales and native/mixed-monitor DPI.
   Very short/high-scale panes may require resizing for a useful graph and
   scrolling optional precision content.

After closing, use `Remove-Item Env:QX_WORKBENCH_PREFS` in that shell to restore
normal preferences. Fit uses finite display sampling, not analytic extremum
isolation. Tiny panes cannot show arbitrarily many axis labels/all optional
controls simultaneously. Unicode coverage remains bounded by bundled fallbacks.
View caches remain session-only; no document/layout schema changed.

## UX and Git status

Implemented the owner's explicit graph/font corrections within the existing
approved layout. No developer-only screen or unrelated feature was introduced.
Compact cards, spacing, fit margins and font sizes are documented implementation
choices for owner interaction/visual review; final visual approval stays with
the owner. All work remains local and uncommitted on the original branch, with
existing unrelated changes preserved. Nothing was published.
