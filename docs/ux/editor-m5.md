# M5 editor UX direction and implementation interpretation

The owner's 2026-10-25 M5 brief and subsequent M5 correction are the authority.
The initial pass had only the text brief; its referenced Inkscape image was not supplied.
The correction's screen recording was supplied and reviewed.
Implementation follows the written diagram and requirements. Visual comparison
with the original drawing remains for owner review.

## Owner-directed behavior

- Keep menus/command toolbar above docked Track left, viewport center, Support
  right and substantial Transition Editor below. Preserve separate Train
  Configuration / Preview / Summary and Ctrl+1/Ctrl+2.
- A prominent authoritative section list and horizontal block sequence share
  selection. Insert Before/After and Earlier/Later identify their exact action.
- Use contextual section properties, engineering numbers, and explicit drafts.
- Graphs are authoring surfaces: visible handles, boundaries, channel/unit labels,
  precise inputs, zoom/pan/fit and one logical history transaction per clean drag.
- File operations use desktop dialogs, retaining unsaved-change protection.
- Use Lucide/Carbon and **Overpass Mono throughout the application**, including
  ordinary controls and engineering data (owner's M5 correction supersedes the
  earlier proportional-text interpretation).
- Support Settings and Appearance stay in the right-hand Support Workspace.
  Future support/train/simulation functionality must remain honestly unavailable.

## Implementation details awaiting visual/interaction review

These are replaceable interpretations within that direction, not assertions of
final product design: neutral dark surfaces, restrained existing accent, section
cards, Section/Coaster/Reference/Project context tabs, compact camera toolbar at
small sizes, square value handles and top-edge diamond boundary handles, channel
colors and precision/derived disclosures. Final brand palette and presentation
remain owner decisions. No silence is treated as design approval.

Value handles move vertically. Internal boundary handles transfer duration
between adjacent transitions; total extent and other channels remain unchanged.
Independent values at joins have separated hit targets. Ctrl+wheel zooms the graph
about the pointer; middle drag pans; F fits the hovered graph/viewport. Numeric
bounds affect only the view. A gesture beginning with a clean section commits once
on release; with a preceding draft it stays staged. Esc during a gesture restores
that preceding draft. Split/remove retain their explicit reshape warnings.

## Owner-directed M5 correction

The owner's corrective brief and recording `quantum-x_cyfuud87ga.mp4` supersede
the earlier graph/font interpretation. This is M5, not M6. Track/Train layout,
Support placement, window management and geometry/document algorithms stay intact.

- Keep the section timeline, selected-section context, channel selection, Apply /
  Cancel and navigation controls outside the precision/derived scrolling area,
  including floating Transition panes. A short pane uses single-line timeline
  cards; the pinned context and card tooltips retain full details.
- Ctrl+wheel over the graph zooms about its pointer; plain wheel never zooms it.
  Middle drag pans even over an endpoint; primary drag edits. Wheel input over
  the timeline and precision tools belongs to those regions. Graph wheel tails
  cannot later scroll them by crossing the pointer into them.
- Each section/channel remembers its session-only view. First visits fit; changing
  domain/units fits again. Extent edits constrain X; value edits/Apply/undo do not
  silently refit Y. Opening another project clears this cache.
- Fit Graph shows the full channel and sampled values with 15% margin. A constant
  rate per metre uses at least ±0.05 around its value; other constants use ±1,
  expanded to 5% for large magnitudes. Zero is a valid constant profile.
- Reset View is explicit: full domain, force Y -1..3 g, distance rates ±0.05 rad/m,
  other rates ±1 in their indicated units. Exact X/Y bounds are staged, validated,
  and published with Apply range; invalid ranges retain the visible view.
- Zoom/pan have finite range/precision limits. Fixed logical-pixel handles and a
  stable scientific-label gutter remain independent of data magnification.
- All application text uses Overpass Mono first, retaining bundled Unicode
  fallback, native DPI and custom UI scaling. The Red Hat Mono integration is retired.

These corrective behaviors follow the explicit brief. Exact typography sizes,
spacing and compact-card treatment remain reviewable implementation choices;
the recording and offscreen images do not establish final visual approval.

No new developer-only screen or fake construction control was added. Existing
concept style catalog, diagnostic overlays/reference geometry and nominal train
configuration remain development capabilities, with their limitations visible.

See [the implementation report and Windows acceptance checklist](../editor-m5.md).
