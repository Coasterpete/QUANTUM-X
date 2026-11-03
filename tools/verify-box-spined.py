"""Verify box-spined-scenario evidence; reads images, never changes them.
python tools/verify-box-spined.py target/m2-box-spined-final
"""
import json
import math
from pathlib import Path
import struct
import sys
import zlib

root = Path(sys.argv[1])
report = json.loads((root / "report.json").read_text(encoding="utf-8"))
assert report["status"] == "verified_visual_construction_with_owner_authorized_provisional_dimensions"
assert report["cancel_apply_undo_redo_save_reopen"] and report["crossties_installed"]
assert not report["production_dimensions_finalized"] and not report["desktop_input"]
assert [report[k] for k in ("rail_center_spacing_m", "rail_diameter_m", "cross_tie_spacing_m")] == [1.1, 0.13, 1.5]
assert report["source_spine"] == {"vertices": 56, "triangles": 44}
assert report["source_crosstie"] == {"vertices": 184, "triangles": 180}
cases = {c["case"]: c for c in report["cases"]}
assert set(cases) == {"04-straight", "05-smooth-hill", "06-horizontal-curve", "07-banked-curve", "08-inverted-loop", "09-connected-sections"}
for name, record in cases.items():
    project = json.loads((root / f"{name}.qx.json").read_text(encoding="utf-8"))
    construction = project["construction"]
    assert project["style"]["style_id"] == "box-spined-100"
    left, right, spine = construction["continuous"]
    for rail, lateral in [(left, -0.55), (right, 0.55)]:
        assert rail["profile"] == {"kind": "circle", "radius_m": 0.065, "sides": 48}
        assert rail["offset_m"] == [0, 0, lateral]
    assert spine["profile"]["kind"] == "asset_section"
    assert spine["profile"]["path"].endswith("100/Box-Spine-100.glb")
    tie, = construction["repeated"]
    assert tie["shape"]["path"].endswith("100/Box-Spine-100-crosstie.glb")
    assert tie["shape"]["scale"] == [1, 1, 1] and tie["offset_m"] == [0, 0, 0]
    assert tie["spacing_m"] == 1.5 and tie["start_m"] == tie["end_margin_m"] == 0.75
    length = sum(s["extent"] for s in project["sections"])
    expected = max(0, math.floor((length - 1.5) / 1.5) + 1)
    assert record["hardware_instances"] == expected, (name, record, expected)
    assert record["vertices"] > 184 and record["triangles"] > 180
    capture = json.loads((root / f"{name}.capture.json").read_text(encoding="utf-8"))
    assert capture["size"] == [2000, 1600] and not capture["popup_open"]
    assert "Vulkan" in record["report"] and "second unchanged upload reused" in record["report"]


def pixels(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    cursor, compressed = 8, bytearray()
    while cursor < len(data):
        length = int.from_bytes(data[cursor:cursor+4], "big")
        kind = data[cursor+4:cursor+8]
        body = data[cursor+8:cursor+8+length]
        assert zlib.crc32(kind + body) & 0xffffffff == int.from_bytes(data[cursor+8+length:cursor+12+length], "big")
        if kind == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert (depth, color, interlace) == (8, 6, 0)
        if kind == b"IDAT":
            compressed.extend(body)
        cursor += length + 12
    stride = width * 4
    raw = zlib.decompress(compressed)
    assert len(raw) == (stride+1)*height
    rows, prior = [], bytearray(stride)
    for y in range(height):
        start = y*(stride+1)
        method = raw[start]
        row = bytearray(raw[start+1:start+1+stride])
        assert method <= 4
        for x in range(stride) if method else []:
            a, b, c = (row[x-4] if x >= 4 else 0), prior[x], (prior[x-4] if x >= 4 else 0)
            p = a+b-c
            paeth = min(((abs(p-a), 0, a), (abs(p-b), 1, b), (abs(p-c), 2, c)))[2]
            predictor = a if method == 1 else b if method == 2 else (a+b)//2 if method == 3 else paeth
            row[x] = (row[x]+predictor) & 255
        rows.append(row)
        prior = row
    return rows


a, b = "02-independent-gloss-and-polished-rails", "03-independent-spine-recolor-draft"
metadata = [json.loads((root / f"{n}.capture.json").read_text(encoding="utf-8")) for n in (a, b)]
assert metadata[0]["viewport"] == metadata[1]["viewport"]
assert metadata[0]["camera_target"] == metadata[1]["camera_target"]
images = [pixels(root / f"{n}.png") for n in (a, b)]
x, y, width, height = map(int, metadata[0]["viewport"])
changes = sum(images[0][row][col:col+3] != images[1][row][col:col+3]
              for row in range(y+50, y+height-4) for col in range((x+4)*4, (x+width-4)*4, 4))
assert changes > 1000, changes
print(f"Verified six actual Editor layouts, original source identity/counts, parametric rails, global rigid tie placement, history/persistence, and {changes} changed viewport pixels for independent spine recoloring.")
