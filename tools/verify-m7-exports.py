"""Independent M7 geometry/evidence checks; Python standard library only.
Run after `quantum-x track-scenario <directory>`.
No desktop interaction, image synthesis, or mesh repair is performed.
"""
import json
import math
import struct
import sys
import zlib
from pathlib import Path

root = Path(sys.argv[1])
evidence = json.loads((root / "evidence.json").read_text(encoding="utf-8"))
assert evidence["automated_offscreen"] and not evidence["manual_desktop_tested"]
fixtures = {f["case"]: f for f in evidence["fixtures"]}
required = {"a-centerline", "b-dual-rails", "c-dual-spine", "d-dual-hardware", "e-single-beam", "f-inverted-loop", "g-edited-force-roll", "h-technical", "h-wireframe", "i-dual-detail", "j-single-detail"}
required.update({"k-banked-dual","k-banked-single","f-inverted-single"})
assert required <= fixtures.keys()

def png_pixels(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    cursor, compressed, size = 8, bytearray(), None
    while cursor < len(data):
        length = int.from_bytes(data[cursor:cursor+4], "big")
        kind = data[cursor+4:cursor+8]
        body = data[cursor+8:cursor+8+length]
        assert len(body) == length
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
    def paeth(a,b,c):
        p = a+b-c
        distances = [abs(p-a),abs(p-b),abs(p-c)]
        return [a,b,c][distances.index(min(distances))]
    for y in range(height):
        start = y * (stride+1)
        method = raw[start]
        row = bytearray(raw[start+1:start+1+stride])
        assert method <= 4
        for x in range(stride) if method else []:
            a = row[x-4] if x >= 4 else 0
            b = prior[x]
            c = prior[x-4] if x >= 4 else 0
            predictor = a if method==1 else b if method==2 else (a+b)//2 if method==3 else paeth(a,b,c)
            row[x] = (row[x]+predictor) & 255
        rows.append(row)
        prior = row
    return size, rows

for name, fixture in fixtures.items():
    project = json.loads((root / (name+".qx.json")).read_text(encoding="utf-8"))
    assert project["schema_version"] in (4, 5, 6)
    if project["schema_version"] >= 5:
        assert project["trains"] == []
    construction = project["construction"]
    assert construction["version"] == 1
    running = [c for c in construction["continuous"] if c["role"] == "running"]
    assert len(running) == (1 if construction["strategy"] == "single_rail" else 2)
    assert fixture["samples"] >= 2 and fixture["triangles"] > 0
    assert all(math.isfinite(fixture[k]) and fixture[k] >= 0 for k in ["solver_ms", "mesh_ms", "cpu_payload_bytes"])
    for component, repeat in zip([c for c in fixture["components"] if c["role"] == "Hardware"], construction["repeated"]):
        end = fixture["length_m"]-repeat["end_margin_m"]
        expected = max(0, math.floor((end-repeat["start_m"])/repeat["spacing_m"]+1e-10)+1)
        assert len(component["stations"]) == expected
        for i, station in enumerate(component["stations"]):
            assert abs(station-repeat["start_m"]-i*repeat["spacing_m"]) < 1e-9
        assert len(component["stations"]) == len(set(component["stations"]))
        assert all(abs(sum(x*x for x in n)-1) < 1e-10 for n in component["placement_frames"])
    positions, triangles = [], 0
    for line in (root/(name+".obj")).read_text(encoding="utf-8").splitlines():
        if line.startswith("v "):
            p = tuple(map(float,line.split()[1:]))
            assert len(p) == 3 and all(math.isfinite(x) for x in p)
            positions.append(p)
        elif line.startswith("f "):
            indices = list(map(int,line.split()[1:]))
            assert len(indices) == 3 and all(1 <= i <= len(positions) for i in indices)
            a,b,c = [positions[i-1] for i in indices]
            u,v = [b[i]-a[i] for i in range(3)],[c[i]-a[i] for i in range(3)]
            cross = [u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
            assert sum(x*x for x in cross) > 1e-30
            triangles += 1
    expected_faces = sum(c["triangles"] * (len(c["stations"]) if c["role"] == "Hardware" else 1) for c in fixture["components"] if c["visible"])
    assert triangles == expected_faces
    for capture in fixture["captures"]:
        size, pixels = png_pixels(root/capture["file"])
        assert size == (capture["width"],capture["height"])
        assert "1 uploads (second unchanged upload reused)" in capture["gpu"]
    assert {(c["width"],c["height"]) for c in fixture["captures"]} == {(1440,960),(960,640)}

assert any(n[1] < -0.95 for n in fixtures["f-inverted-loop"]["components"][-1]["placement_frames"])
assert (root/"d-dual-hardware.obj").read_bytes() != (root/"g-edited-force-roll.obj").read_bytes()
# Compare only the interior GPU viewport; exclude menus, labels and graph text.
_, baseline = png_pixels(root/"d-dual-hardware-1440.png")
for name in ["a-centerline", "b-dual-rails", "e-single-beam", "g-edited-force-roll", "h-technical", "h-wireframe"]:
    _, other = png_pixels(root/(name+"-1440.png"))
    different = sum(baseline[y][x:x+3] != other[y][x:x+3] for y in range(190,525) for x in range(330*4,1100*4,4))
    assert different > 100, (name,different)
for name in ["f-inverted-loop","f-inverted-single"]:
    assert any(n[1] < -0.95 for n in fixtures[name]["components"][-1]["placement_frames"])
assert len(evidence["live_edit"]) == 4
for live in evidence["live_edit"]:
    assert not live["native_windows_input"]
    for key in ["mesh_changed_while_pointer_held","idle_revision_stable","release_retained_draft","apply_reused_revision","undo_redo_restored_mesh","cancel_restored_mesh","save_reopen_identical"]:
        assert live[key]
    for capture in live["captures"]:
        size,_ = png_pixels(root/capture["file"])
        assert size == (capture["width"],capture["height"])
        # The unedited quintic needs 901 stations. A local NURBS endpoint edit
        # introduces genuine internal span breaks, adding three integration
        # stations at this fixture's requested resolution. Apply must retain
        # the exact preview sample count rather than reverting to the old grid.
        expected_samples = 901 if "-before-" in capture["file"] else 904
        assert capture["samples"] == expected_samples and capture["triangles"] > 0
for construction in ["dual","single"]:
    before=(root/f"live-{construction}-before.obj").read_bytes()
    draft=(root/f"live-{construction}-draft.obj").read_bytes()
    after=(root/f"live-{construction}-after.obj").read_bytes()
    assert before != draft and draft == after
    before_project=json.loads((root/f"live-{construction}-before.qx.json").read_text(encoding="utf-8"))
    after_project=json.loads((root/f"live-{construction}-after.qx.json").read_text(encoding="utf-8"))
    assert before_project["sections"] != after_project["sections"]
    assert before_project["construction"] == after_project["construction"]
for row in evidence["benchmarks"]:
    assert row["samples"] > 0 and row["mesh_ms"] > 0 and row["cpu_payload_bytes"] > 0
print(f'M7 verified: {len(fixtures)} fixtures, {2*len(fixtures)+12} actual PNG captures including live-edit workflows, finite nondegenerate OBJ triangles, arc stations, inverted hardware, viewport differences and cache/upload evidence. Build: {evidence["build"]}.')
