# QUANTUM-X — Coaster Properties UX Specification

**Status:** Human-directed UX draft  
**Component:** Track Workspace → Coaster Properties  
**Implementation:** Rust / egui, backed by independent core libraries

## Purpose

Coaster Properties allows users to configure the overall roller coaster, including its coaster family, numbered track style, and compatible train styles.

The panel is part of the human-authored QUANTUM-X editor layout. It must not replace the graph/timeline-based track-authoring workflow.

## Family and style selection

Users can browse coaster families and their associated numbered styles.

Examples:

| Family | Styles |
|---|---|
| Hybrid | Hybrid-100; additional variants as developed |
| Monorail | Monorail-100, Monorail-200 |
| Wooden | Numbered wooden styles, to be defined |
| Other coaster families | Expandable through the catalog system |

The family selector filters the available styles.

## Visual previews

Every released family/style must have a representative preview image showing its appearance.

The selector should support:

- Thumbnail images for each style.
- A larger preview of the selected style.
- Human-readable style names and descriptions.
- Compatible train information where available.
- Optional additional reference angles.
- A clear indication when an asset is experimental or unfinished.

Previews must represent the actual style rather than unrelated stock imagery.

## Selection behavior

Selecting a family displays its available styles. Selecting a style updates its information and preview before the user applies the change.

The application should validate compatibility and preserve the authored track centerline when changing visual styles, unless an explicitly supported conversion requires otherwise.

## Technical requirements

Style entries should have stable identifiers and data-driven metadata, including family, style number, preview image path, asset references, compatible trains, and development status.

The underlying coaster catalog must remain independent of egui and rendering code.

## Design authority

This document establishes intended functionality, not final visual styling.

Final placement, typography, colors, iconography, and interaction refinements remain subject to human UX review.

**Core principle:** Visual style selection supports the coaster-authoring experience; it does not replace QUANTUM-X's graph/timeline editor.