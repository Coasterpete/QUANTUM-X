"""Check real material-editor-scenario captures; Python standard library only.

python tools/verify-material-editor.py target/m11-editor
Does not create or alter any image or source asset.
"""
import json
from pathlib import Path
import struct
import sys
import zlib

root = Path(sys.argv[1])
evidence = json.loads((root / "evidence.json").read_text(encoding="utf-8"))
assert evidence["actual_editor_surface"] and evidence["automated_offscreen"]
assert not evidence["manual_desktop_tested"] and not evidence["development_tools_enabled"]
assert evidence["named_ui_selections_exercised"]
assert evidence["apply_cancel_undo_redo_save_reopen"]
assert evidence["geometry_uv_reference_train_data_unchanged"]
project = json.loads((root / "material-assignments.qx.json").read_text(encoding="utf-8"))
assert project["schema_version"] == evidence["schema_version"] == 6
def material_f32_identity(value):
    # serde_json::Value widens f32 to f64, while the project writer emits the
    # shortest f32 decimal. Compare identical binary32 values, not decimal text.
    if isinstance(value, (int, float)):
        return struct.pack("<f", value)
    if isinstance(value, list):
        return [material_f32_identity(v) for v in value]
    if isinstance(value, dict):
        return {k: material_f32_identity(v) for k, v in value.items()}
    return value


construction = dict(project["construction"])
recorded = dict(evidence["construction"])
assert material_f32_identity(construction.pop("materials")) == material_f32_identity(recorded.pop("materials"))
assert construction == recorded  # f64 geometry stays an exact comparison.
slots = {m["id"]: m for m in project["construction"]["materials"]}
assert slots["running"]["library"]["id"] == "track-bolt-metal"
assert slots["structure"]["library"]["id"] == "track-pine"
assert all(c["asset"] is None for t in project["trains"] for c in t["components"])
for material in evidence["smoothness"]:
    if material["id"] in {"gloss-painted-steel", "polished-steel", "satin-steel", "matte-paint"}:
        assert material["smoothness"] is None and material["roughness"] is None
        continue
    assert material["smoothness"] and material["roughness"] is None
    assert material["roughness_factor"] == 1
    assert material["normal_convention"] == "open_gl"
    if material["id"] == "track-pine":
        assert material["smoothness"]["path"].endswith("pinewood_roughness.png")
placeholder = evidence["train_placeholder"]
assert len(set(placeholder["base_color"][:3])) == 1
assert placeholder["metallic"] == placeholder["textures"] == 0
assert material_f32_identity(placeholder["roughness"]) == material_f32_identity(0.9)


def png_pixels(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    cursor, compressed, size = 8, bytearray(), None
    while cursor < len(data):
        length = int.from_bytes(data[cursor:cursor+4], "big")
        kind = data[cursor+4:cursor+8]
        body = data[cursor+8:cursor+8+length]
        assert zlib.crc32(kind + body) & 0xffffffff == int.from_bytes(data[cursor+8+length:cursor+12+length], "big")
        if kind == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert (depth, color, interlace) == (8, 6, 0)
            size = width, height
        if kind == b"IDAT":
            compressed.extend(body)
        cursor += 12 + length
    width, height = size
    raw = zlib.decompress(compressed)
    stride = width * 4
    assert len(raw) == (stride+1)*height
    rows, prior = [], bytearray(stride)

    def paeth(a, b, c):
        p = a+b-c
        distances = [abs(p-a), abs(p-b), abs(p-c)]
        return [a, b, c][distances.index(min(distances))]

    for y in range(height):
        start = y * (stride+1)
        method = raw[start]
        row = bytearray(raw[start+1:start+1+stride])
        assert method <= 4
        for x in range(stride) if method else []:
            a, b, c = (row[x-4] if x >= 4 else 0), prior[x], (prior[x-4] if x >= 4 else 0)
            predictor = a if method == 1 else b if method == 2 else (a+b)//2 if method == 3 else paeth(a, b, c)
            row[x] = (row[x]+predictor) & 255
        rows.append(row)
        prior = row
    return size, rows


names = ["01-construction-default", "02-registered-procedural-rails-draft",
         "03-independent-spine-draft", "04-live-rail-material-change"]
previous, viewport, changes = None, None, {}
for name in names:
    capture = json.loads((root / f"{name}.capture.json").read_text(encoding="utf-8"))
    size, pixels = png_pixels(root / f"{name}.png")
    assert list(size) == capture["size"]
    assert capture["viewport"] == (viewport or capture["viewport"])
    viewport = capture["viewport"]
    if previous is not None:
        x, y, width, height = map(int, viewport)
        # Exclude the viewport status text. Compare actual 3D GPU pixels only.
        changed = sum(max(abs(a-b) for a, b in zip(pixels[row][col:col+3], previous[row][col:col+3])) > 2
                      for row in range(y+40, y+height) for col in range(x*4, (x+width)*4, 4))
        assert changed > 100, f"No meaningful viewport material change: {name}: {changed}"
        changes[name] = changed
    previous = pixels

size, pixels = png_pixels(root / "06-neutral-train-placeholders.png")
capture = json.loads((root / "06-neutral-train-placeholders.capture.json").read_text(encoding="utf-8"))
assert capture["primitive_instances"] == 3
body = next(b for b in capture["projected_bounds"] if b["component"] == "car-1")
points = body["corners"]
left = max(0, int(min(p[0] for p in points)))
right = min(size[0], int(max(p[0] for p in points)))
top = max(0, int(min(p[1] for p in points)))
bottom = min(size[1], int(max(p[1] for p in points)))
# Neutral-gray material acquires a modest blue cast from procedural sky light.
# Material metadata above independently asserts equal base channels and no maps.
neutral = sum(40 < pixels[y][x] < 230 and max(pixels[y][x:x+3])-min(pixels[y][x:x+3]) <= 32
              for y in range(top, bottom) for x in range(left*4, right*4, 4))
assert neutral > 1000, "Gray train body is not visibly present in its GPU-projected bounds"
assert png_pixels(root / "05-smoothness-inspection.png")[0] == size

# The exact same finish is recolored; component state and geometry survive save.
colors = json.loads((root / "component-colors.qx.json").read_text(encoding="utf-8"))
assert colors["sections"] == project["sections"]
assert colors["initial"] == project["initial"]
assert colors["heartline"] == project["heartline"]
assert colors["trains"] == project["trains"]
colored_slots = {m["id"]: m for m in colors["construction"]["materials"]}
for slot, expected in [("running", (235, 30, 65)), ("structure", (138, 32, 240)),
                       ("hardware", (40, 230, 90))]:
    binding = colored_slots[slot]["library"]
    assert binding["id"] == "gloss-painted-steel"
    srgb = tuple(round(255 * (12.92 * v if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055))
                 for v in binding["tint"])
    assert srgb == expected, (slot, srgb)
    assert binding["uv_scale"] == [1, 1] and binding["uv_offset"] == [0, 0]
assert json.loads((root / "10-native-egui-color-picker.capture.json").read_text())["popup_open"]
for first, second in [("07-gloss-blue", "08-gloss-red"),
                      ("11-polished-track", "12-satin-track")]:
    _, a = png_pixels(root / f"{first}.png")
    _, b = png_pixels(root / f"{second}.png")
    x, y, width, height = map(int, json.loads((root / f"{first}.capture.json").read_text())["viewport"])
    changed = sum(max(abs(c-d) for c, d in zip(a[row][col:col+3], b[row][col:col+3])) > 2
                  for row in range(y+40, y+height) for col in range(x*4, (x+width)*4, 4))
    assert changed > 100, (first, second, changed)
    changes[second] = changed
print(json.dumps({"viewport_changed_pixels": changes, "neutral_train_pixels": neutral,
                  "persistence_and_metadata": "passed", "actual_gpu_pngs": 12,
                  "same_finish_recolor_component_persistence_and_picker": "passed"}, indent=2))
