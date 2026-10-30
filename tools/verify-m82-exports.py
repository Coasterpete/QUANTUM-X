"""Independent M8.2 snapshot/transform/constraint checks; standard library only.
Run after quantum-x train-playback-scenario <output-directory>.
This verifies synthetic fixtures, not vehicle-family realism or native desktop UX.
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
assert evidence["fixed_dt_s"] == 1 / 240


def vector(v):
    return [v[k] for k in "xyz"]


def add(a, b):
    return [x + y for x, y in zip(a, b)]


def sub(a, b):
    return [x - y for x, y in zip(a, b)]


def norm(v):
    return math.sqrt(sum(x * x for x in v))


def cross(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]


def rotate(q, p):
    v = vector(q)
    return add(p, add([2*q["w"]*x for x in cross(v, p)], [2*x for x in cross(v, cross(v, p))]))


def near(a, b, tolerance=1e-8):
    assert norm(sub(a, b)) < tolerance, (a, b)


def pixels(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    cursor, compressed = 8, bytearray()
    while cursor < len(data):
        size, kind = struct.unpack_from(">I4s", data, cursor)
        body = data[cursor+8:cursor+8+size]
        assert zlib.crc32(kind+body) & 0xffffffff == struct.unpack_from(">I", data, cursor+8+size)[0]
        if kind == b"IHDR":
            w, h, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert (depth, color, interlace) == (8, 6, 0)
        if kind == b"IDAT":
            compressed.extend(body)
        cursor += size+12
    raw = zlib.decompress(compressed)
    stride, previous, rows = w*4, bytearray(w*4), []
    for y in range(h):
        method = raw[y*(stride+1)]
        row = bytearray(raw[y*(stride+1)+1:(y+1)*(stride+1)])
        for x in range(stride) if method else []:
            a, b, c = row[x-4] if x >= 4 else 0, previous[x], previous[x-4] if x >= 4 else 0
            if method == 4:
                distances = [abs(a+b-c-v) for v in (a, b, c)]
                predictor = (a, b, c)[distances.index(min(distances))]
            else:
                predictor = {1: a, 2: b, 3: (a+b)//2}[method]
            row[x] = (row[x]+predictor) & 255
        rows.append(row)
        previous = row
    return (w, h), rows


def capture(path, count, size):
    metadata = json.loads(path.with_suffix(".capture.json").read_text())
    assert metadata["size"] == list(size) and metadata["train_fit"]
    assert metadata["unique_glb_resources"] == 2
    assert len(metadata["projected_bounds"]) == count
    x, y, w, h = metadata["viewport"]
    assert w > 200 and h > 150
    for b in metadata["projected_bounds"]:
        assert len(b["corners"]) == 8
        assert all(x <= px <= x+w and y <= py <= y+h for px, py in b["corners"])
    actual_size, rows = pixels(path)
    assert actual_size == size
    # Count actual shaded asset colors in the viewport, independently of captions.
    orange = blue = 0
    for row in rows[int(y):int(y+h)]:
        for i in range(int(x)*4, int(x+w)*4, 4):
            r, g, b = row[i:i+3]
            orange += r > 100 and r > g*1.3 and g > b*1.2
            blue += b > 70 and b > r*1.3 and g > r*1.1
    assert orange > 100 and blue > 100, (orange, blue, path)


assert {c["name"] for c in evidence["cases"]} == {"conventional", "below-track", "rotating-seats"}
for case in evidence["cases"]:
    directory = root / case["name"]
    project = json.loads((root / case["project"]).read_text())
    assert project["schema_version"] == 6
    train = project["trains"][0]
    assert len(train["cars"]) == 3
    components = {c["id"]: c for c in train["components"]}
    for c in components.values():
        assert (directory / c["asset"]["path"]).read_bytes()[:4] == b"glTF"
    start, end = case["records"]
    assert (start["ticks"], end["ticks"], end["time_s"]) == (0, 120, 0.5)
    assert all(case[k] < 0.02 for k in ("maximum_joint_m", "maximum_guide_m", "maximum_angular_rad"))
    for record in case["records"]:
        assert record["train"] == train and not record["running"]
        poses = {p["id"]: p for p in record["components"]}
        assert set(poses) == set(components)
        for p in poses.values():
            assert abs(sum(p["rotation"][k]**2 for k in "wxyz")-1) < 1e-10
        for j, report in zip(train["connections"], record["joints"]):
            a, b = poses[j["component_a"]], poses[j["component_b"]]
            pa = add(vector(a["position_m"]), rotate(a["rotation"], vector(j["anchor_a_m"])))
            pb = add(vector(b["position_m"]), rotate(b["rotation"], vector(j["anchor_b_m"])))
            near(pa, vector(report["a"]))
            near(pb, vector(report["b"]))
            assert abs(norm(sub(pa, pb))-report["error_m"]) < 1e-9
        for instance in record["instances"]:
            p, c = poses[instance["id"]], components[instance["id"]]
            a = c["asset"]
            # M8.2 fixtures retain identity visual rotation and zero model pivots.
            assert a["rotation"] == {"w": 1.0, "x": 0.0, "y": 0.0, "z": 0.0}
            near(instance["matrix"][3][:3], add(vector(p["position_m"]), rotate(p["rotation"], vector(a["translation_m"]))))
            for axis in range(3):
                v = [a["scale"]["xyz"[axis]] if i == axis else 0 for i in range(3)]
                near(instance["matrix"][axis][:3], rotate(p["rotation"], v))
        capture(directory / record["capture"], len(components), (1440, 960))
    a, b = start["components"][0], end["components"][0]
    displacement = norm(sub(vector(a["position_m"]), vector(b["position_m"])))
    assert displacement > 0.9
    if case["name"] != "rotating-seats":
        expected_speed = 2 + 9.81*math.sin(0.2)*0.5
        assert all(abs(v-expected_speed) < 1e-5 for v in end["speed_mps"])
    else:
        seats = [p for p in end["components"] if components[p["id"]]["kind"] == "passenger_assembly"]
        assert len(seats) == 3
        first = next(p for p in start["components"] if p["id"] == seats[0]["id"])
        assert norm([seats[0]["rotation"][k]-first["rotation"][k] for k in "wxyz"]) > 0.02
        assert len(end["diagnostics"]["angular_torque_nm"]) == 3
    capture(directory / case["compact_capture"], len(components), (960, 640))
    print(f"PASS {case['name']}: {len(components)} simulated bodies, 3 captures, {displacement:.6f} m motion")
print("M8.2 independent fixture verification passed; native desktop acceptance was not performed.")
