# M7 construction controls — implementation mapping

Authority: the owner's M7 procedural track engine directive and resume request.
This document records the implementation mapping, not a new human-authored design
or final visual approval.

Requested behavior implemented in the existing workbench:

- Track Workspace → Coaster: generic dual/single configuration, profile dimensions,
  offsets, repeated hardware, materials, visibility, Apply/Cancel.
- Existing View menu/panel: solid, solid with centerline, technical and wireframe
  modes, with independent component/reference visibility.
- Existing Diagnostics panel: CPU mesh/source timing and mesh statistics.
- Existing graph gestures, precise numeric entry, draft previews, history and file
  dialogs retain their M4–M6 behavior.

The owner's subsequent save/draft corrective request adds the shared
Apply All & Save resolver, toolbar draft discovery, unapplied panel badges,
history resolution and pre-picker unsaved-work confirmation. See the
[corrective report](../m7-save-drafts-correction.md) for exact behavior,
regressions and the native acceptance checklist. These corrections preserve the
existing workbench and its explicit Apply/Cancel transaction model.

The generic configurations are explicitly provisional engine validation data.
Hybrid-100, Monorail-100 and Monorail-200 remain conceptual catalog selections.
No manufacturer model or final production dimensions were added.

The exact control grouping and panel spacing are reviewable implementation
choices. Final profile/asset authoring UX and visual identity require owner
input. There is no separate developer-only editor screen. CLI-generated projects
and captures are opt-in verification fixtures.

See [the implementation report](../m7-procedural-track-engine.md) for reproducible
checks, architecture and the native Windows checklist.
