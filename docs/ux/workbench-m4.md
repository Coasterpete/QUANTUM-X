# Integrated Workbench M4: owner-directed structure

This records the owner's 2026-10-25 implementation brief. It supersedes Pass 3's
provisional layout description, while retaining its existing editing semantics.
The original Inkscape sketches were not available in the inspected QUANTUM-X UX
materials. Do not describe this implementation as a translation of those sketches.

## Approved organization

- Top: application menus, command toolbar, Track/Train switching and project status.
- Track profile: ordered sections/regions and authoring on the left, shared 3D
  viewport in the center, Support Workspace on the right, Transition Editor below.
- Support Settings belongs inside Support Workspace. Support Appearance is a
  focused subpanel there, rather than Preferences or another workspace profile.
  Prefabs, foundations, rail connectors, support definition copy/paste, structure
  and component editing belong here when backed by implemented document services.
- Train profile: Configuration, Preview and Summary, sharing the same document
  and history. Expose existing nominal configuration/reference data honestly.
- Both profiles retain independent layouts and pane navigation. Splits, tabbing,
  floating panes and resetting must be recoverable without document mutations.
- Display preferences are application state. Logical window size, monitor DPI
  and UI zoom are distinct. Never change the monitor's display mode.
- Transition graphs use subdued adaptive dots, meaningful axes/units and zero
  references. The 3D ground grid remains independent.
- Ordinary section insertion presents coaster concepts. Existing spline sections
  and explicit advanced mathematical tools remain supported without conversions.

## Replaceable development presentation

Exact colors, typography, spacing, tab treatment, floating-window presentation,
support construction interactions and a complete train editor have not been
approved as final product design. M4 uses neutral dark egui presentation and
labels its development/reference limitations. Existing Coaster Properties UX
remains authoritative for that component. No absent support or train backend may
be represented by an enabled pretend control.

See [the implementation and verification guide](../workbench-m4.md) for concrete
behavior, limitations, reference research and the Windows acceptance checklist.
