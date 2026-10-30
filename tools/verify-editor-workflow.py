"""Independently check editor-workflow-scenario artifacts; Python standard library."""
import json
import math
import struct
import sys
from pathlib import Path

root = Path(sys.argv[1])
evidence = json.loads((root / "evidence.json").read_text(encoding="utf-8"))
assert evidence["automated_offscreen"] and not evidence["manual_desktop_tested"]
for key in [
    "welcome_without_document", "creation_requires_only_name",
    "cancel_creates_no_document", "new_style_unassigned",
    "properties_undo_redo_roundtrip", "geometry_construction_trains_preserved",
    "single_coaster",
]:
    assert evidence[key], key
before = json.loads((root / "unassigned.qx.json").read_text(encoding="utf-8"))
after = json.loads((root / "properties.qx.json").read_text(encoding="utf-8"))
assert before["schema_version"] == after["schema_version"] == evidence["schema_version"] == 6
assert before["style"] == {"family_id": "", "style_id": ""}
assert after["style"] == {"family_id": "monorail", "style_id": "monorail-200"}
assert after["metadata"]["description"] and not before["metadata"]["description"]
assert before["metadata"]["project_id"] == after["metadata"]["project_id"]
assert "Editor" in evidence["title"]
for key in ["sections", "initial", "construction", "heartline", "trains"]:
    assert before[key] == after[key], key
assert len(before["sections"]) == 1 and before["sections"][0]["extent"] == 10
captures = [
    "welcome", "new-project", "properties-unassigned-1440",
    "properties-unassigned-960", "properties-assigned-1440",
    "properties-assigned-960", "properties-scale-1.5", "properties-scale-2",
]
for name in captures:
    data = (root / f"{name}.png").read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    width, height = struct.unpack(">II", data[16:24])
    capture = json.loads((root / f"{name}.capture.json").read_text(encoding="utf-8"))
    assert capture["size"] == [width, height]
    x, y, w, h = capture["viewport"]
    assert all(math.isfinite(v) for v in [x, y, w, h])
    assert x >= 0 and y >= 0 and w > 0 and h > 0
    assert x + w <= width + 1 and y + h <= height + 1, name
print("Editor workflow verified: unassigned/assigned schema 6 round trips, unchanged geometry/construction/trains, eight real captures, bounded physical viewports through 200% scaling.")
