# M6 owner-directed authoring workflow

Authority: the owner's supplied “M6 Integrated Track Authoring & Workflow
Overhaul” brief, the associated recording `quantum-x_7WWGob62Sf.mp4`, and the
explicit resume instruction prioritizing stable egui control IDs. Existing
[M4](workbench-m4.md) and [M5](editor-m5.md) workbench direction remains in force.

## Implemented owner-directed behavior

- Keep the top menus/toolbars, Track left, viewport center, Support right,
  Transition Editor below, and separate Train profile. Retain independent
  layouts/camera state, docking/floating, Ctrl+1/2, scaling, Overpass Mono, and
  Lucide/Carbon assets.
- Ordinary insertion offers Straight, Force-vector, Rider-local geometric,
  World-geometric, and Circular arc before/after the selected section. Advanced
  spline creation remains available. Insertion selects the new section and its
  relevant graph channel.
- Preserve committed data until explicit Apply. Pending navigation/insertion
  offers Apply & continue, Cancel draft & continue, and Keep editing. A rejected
  candidate preserves its authored draft, document/history, and last valid view.
- Numeric entry must retain its control identity when draft notices appear.
  Plain Enter finishes numeric entry. Apply/Ctrl+Enter accepts a section. Escape
  first belongs to an open popup or focused editor; an unclaimed Escape cancels
  the section draft. A graph-gesture Escape restores the preceding draft.
- Keep authored inputs distinct from read-only design-speed measurements. Show
  applicable channels and exact transition values/functions without opening
  unrelated property groups. Advanced speed/boundary/domain options follow the
  essential controls.
- Share persistent section selection across list, timeline, inspector, graph,
  and viewport. Hover/station inspection remains transient.
- Frame startup/New/Open using actual viewport dimensions and visible geometry.
  Fit Selection targets the selected range. Ordinary edits/insertion/regeneration
  preserve manually positioned cameras. Valid drafts preview independently.

## Reviewable implementation choices

These remain replaceable interpretations of the brief: a nonmodal status-area
draft resolver; contextual Edit values/Transitions menus; compact More/Graph view
menus in smaller panes; 15% camera fitting margin; eight-logical-pixel centerline
picking; endpoint arrows and coordinate readouts; and the exact placement/spacing
of basic/advanced controls. They are not assertions of final visual approval.

No new shipping developer-only screen was added. Existing line-reference
rendering, support/train capability disclosures, and derived diagnostic overlays
remain development capabilities. The CLI hill fixture is explicitly opt-in
validation data and is never loaded into a new project automatically. Native
comfort and final visual/interaction acceptance remain for the owner.

See [the implementation report](../editor-m6.md) for algorithms, evidence,
limitations, and the owner-executable Windows checklist.
