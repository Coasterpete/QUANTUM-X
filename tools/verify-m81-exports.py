"""Independent M8.1 verification; Python standard library, no Rust imports.
Run after quantum-x train-asset-scenario <authored-project> <output-directory>.
Checks GLB bytes, hierarchy, mesh data, component/pivot transforms, joint
residuals, packaged assets, camera bounds and actual rendered train pixels.
This is kinematic inspection, not a physics-playback or desktop acceptance test.
"""
import copy
import json
import math
import struct
import sys
import zlib
from pathlib import Path

root = Path(sys.argv[1])
evidence = json.loads((root / "evidence.json").read_text(encoding="utf-8"))
assert evidence["automated_offscreen"] and not evidence["manual_desktop_tested"]
assert evidence["history_save_reopen_save_as"]
project = json.loads((root / evidence["project"]).read_text(encoding="utf-8"))
relocated = json.loads((root / "save-as/train-inspection.qx.json").read_text(encoding="utf-8"))
assert project == relocated and project["schema_version"] == 6
train = next(t for t in project["trains"] if t["id"] == evidence["train_id"])
mechanical = copy.deepcopy(train)
for component in mechanical["components"]:
    component["asset"] = None
assert mechanical == evidence["mechanical_definition"]
components = {c["id"]: c for c in train["components"]}


def vector(v):
    return [v[k] for k in "xyz"] if isinstance(v, dict) else list(v)


def add(a, b):
    return [x+y for x, y in zip(a, b)]


def sub(a, b):
    return [x-y for x, y in zip(a, b)]


def mul(a, s):
    return [x*s for x in a]


def norm(a):
    return math.sqrt(sum(x*x for x in a))


def cross(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]


def rotate(q, p):
    if not isinstance(q, dict):
        q = dict(zip("xyzw", q))
    v = vector(q)
    return add(p, add(mul(cross(v, p), 2*q["w"]), mul(cross(v, cross(v, p)), 2)))


def close(a, b, eps=1e-9):
    assert len(a) == len(b) and all(math.isfinite(x) and abs(x-y) < eps for x, y in zip(a, b)), (a, b)


def point(m, p):
    return [m[3][i] + sum(m[c][i]*p[c] for c in range(3)) for i in range(3)]


identity = [[float(i == j) for i in range(4)] for j in range(4)]


def matrices(a, b):
    return [[sum(a[k][row]*b[col][k] for k in range(4)) for row in range(4)] for col in range(4)]


models = {}
for model in evidence["models"]:
    path = model["path"]
    binary = (root / path).read_bytes()
    assert binary == (root / "save-as" / path).read_bytes()
    assert binary[:4] == b"glTF" and struct.unpack_from("<II", binary, 4) == (2, len(binary))
    count, kind = struct.unpack_from("<I4s", binary, 12)
    assert kind == b"JSON"
    source = json.loads(binary[20:20+count])
    if "blender" in path:
        assert source["asset"]["generator"].startswith("Khronos glTF Blender I/O")
    size, kind = struct.unpack_from("<I4s", binary, 20+count)
    assert kind == b"BIN\x00"
    data = binary[28+count:28+count+size]

    def accessor(index):
        a = source["accessors"][index]
        view = source["bufferViews"][a["bufferView"]]
        width = {"VEC3": 3, "VEC2": 2, "SCALAR": 1}[a["type"]]
        fmt = {5126: "f", 5123: "H", 5125: "I", 5121: "B"}[a["componentType"]]
        stride = view.get("byteStride", struct.calcsize("<"+fmt)*width)
        offset = view.get("byteOffset", 0) + a.get("byteOffset", 0)
        return [list(struct.unpack_from("<"+fmt*width, data, offset+i*stride)) for i in range(a["count"])]

    raw_primitives = [p for mesh in source["meshes"] for p in mesh["primitives"]]
    assert len(raw_primitives) == len(model["primitives"]) == 1
    p = raw_primitives[0]
    actual = model["primitives"][0]
    assert actual["positions"] == accessor(p["attributes"]["POSITION"])
    assert actual["normals"] == accessor(p["attributes"]["NORMAL"])
    assert actual["uv"] == accessor(p["attributes"]["TEXCOORD_0"])
    assert actual["indices"] == [v[0] for v in accessor(p["indices"])]
    assert len(actual["indices"]) == 36 and len(actual["positions"]) == 24
    for normal in actual["normals"]:
        assert abs(norm(normal)-1) < 1e-12
    nodes = []

    def visit(index, parent):
        n = source["nodes"][index]
        if "matrix" in n:
            local = [n["matrix"][i:i+4] for i in range(0, 16, 4)]
        else:
            local = copy.deepcopy(identity)
            scale = n.get("scale", [1]*3)
            q = n.get("rotation", [0, 0, 0, 1])
            for axis in range(3):
                local[axis][:3] = rotate(q, [scale[axis] if i == axis else 0 for i in range(3)])
            local[3][:3] = n.get("translation", [0]*3)
        world = matrices(parent, local)
        if "mesh" in n:
            nodes.append(world)
        for child in n.get("children", []):
            visit(child, world)

    for index in source["scenes"][source.get("scene", 0)]["nodes"]:
        visit(index, identity)
    assert len(nodes) == len(model["nodes"])
    for a, b in zip(nodes, model["nodes"]):
        close(sum(a, []), sum(b["matrix"], []), 2e-7)
    models[path] = model


def png_pixels(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    cursor, compressed = 8, bytearray()
    while cursor < len(data):
        size, kind = struct.unpack_from(">I4s", data, cursor)
        body = data[cursor+8:cursor+8+size]
        assert zlib.crc32(kind+body) & 0xffffffff == struct.unpack_from(">I", data, cursor+8+size)[0]
        if kind == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert (depth, color, interlace) == (8, 6, 0)
        if kind == b"IDAT":
            compressed.extend(body)
        cursor += size+12
    raw = zlib.decompress(compressed)
    stride = width*4
    rows, prior = [], bytearray(stride)
    for y in range(height):
        method = raw[y*(stride+1)]
        row = bytearray(raw[y*(stride+1)+1:(y+1)*(stride+1)])
        for x in range(stride) if method else []:
            a, b, c = row[x-4] if x >= 4 else 0, prior[x], prior[x-4] if x >= 4 else 0
            if method == 4:
                d = [abs(a+b-c-v) for v in (a, b, c)]
                predictor = (a, b, c)[d.index(min(d))]
            else:
                predictor = {1: a, 2: b, 3: (a+b)//2}[method]
            row[x] = (row[x]+predictor) & 255
        rows.append(row)
        prior = row
    return (width, height), rows


records = evidence["records"]
assert len(records) == 2 and records[0]["station"] != records[1]["station"]
for record in records:
    poses = {p["id"]: p for p in record["components"]}
    instances = {i["component"]: i for i in record["instances"]}
    assert len(poses) == len(instances) == 6
    for car in train["cars"]:
        body = poses[car["body_id"]]
        assert abs(body["station"] - (record["station"]-car["offset_m"])) < 1e-10
        front, rear = [poses[id] for id in car["bogie_ids"]]
        close(vector(body["position"]), mul(add(vector(front["position"]), vector(rear["position"])), 0.5))
        chord = sub(vector(front["position"]), vector(rear["position"]))
        close(rotate(body["orientation"], [1, 0, 0]), mul(chord, 1/norm(chord)))
        for bogie in [front, rear]:
            close(vector(bogie["position"]), vector(bogie["track_position"]))
            close([bogie["orientation"][k] for k in "wxyz"], [bogie["track_orientation"][k] for k in "wxyz"])
    for id, pose in poses.items():
        assert abs(sum(v*v for v in pose["orientation"].values())-1) < 1e-12
        c = components[id]
        asset = c["asset"]
        m = instances[id]["matrix"]
        scale, translation, pivot = map(vector, (asset["scale"], asset["translation_m"], asset["pivot_m"]))

        def expected(p):
            local = [scale[i]*(p[i]-pivot[i]) for i in range(3)]
            return add(vector(pose["position"]), rotate(pose["orientation"], add(translation, rotate(asset["rotation"], local))))

        for p in [[0, 0, 0], [1, 2, 3], [-0.3, 0.7, -1.1]]:
            close(point(m, p), expected(p))
        model = models[asset["path"]]
        vertices = [point(m, point(n["matrix"], v)) for n in model["nodes"] for v in model["primitives"][n["primitive"]]["positions"]]
        close(vector(instances[id]["bounds"][0]), [min(v[i] for v in vertices) for i in range(3)])
        close(vector(instances[id]["bounds"][1]), [max(v[i] for v in vertices) for i in range(3)])
    for connection, joint in zip(train["connections"], record["joints"]):
        anchors = []
        for suffix in "ab":
            pose = poses[connection["component_"+suffix]]
            anchors.append(add(vector(pose["position"]), rotate(pose["orientation"], vector(connection["anchor_"+suffix+"_m"]))))
        close(anchors[0], vector(joint["a"]))
        close(anchors[1], vector(joint["b"]))
        assert abs(norm(sub(*anchors))-joint["error"]) < 1e-10
    for capture in record["captures"]:
        path = root / capture["file"]
        meta = json.loads(path.with_suffix(".capture.json").read_text())
        assert meta["train_fit"] and meta["unique_glb_resources"] == 2 and meta["primitive_instances"] == 22
        assert meta["unique_glb_textures"] == 2 and meta["asset_upload_ms"] >= 0
        size, rows = png_pixels(path)
        assert size in [(1440, 960), (960, 640)] and list(size) == meta["size"]
        x, y, width, height = meta["viewport"]
        corners = [p for box in meta["projected_bounds"] for p in box["corners"]]
        assert len(corners) == 48
        assert all(x <= p[0] <= x+width and y <= p[1] <= y+height for p in corners)
        assert max(p[0] for p in corners)-min(p[0] for p in corners) > width*0.45
        for box in meta["projected_bounds"]:
            if not box["component"].startswith("car-"):
                continue
            a, b = [int(min(p[i] for p in box["corners"])) for i in range(2)]
            c, d = [int(max(p[i] for p in box["corners"])) for i in range(2)]
            gold = sum(rows[j][i*4] > 80 and rows[j][i*4] > rows[j][i*4+1]*1.1 and rows[j][i*4+1] > rows[j][i*4+2]*1.1 for j in range(b, d) for i in range(a, c))
            assert gold > 100, (capture["file"], box["component"], gold)

for a, b in zip(records[0]["components"], records[1]["components"]):
    assert norm(sub(vector(a["position"]), vector(b["position"]))) > 5
    assert norm(sub(rotate(a["orientation"], [1, 0, 0]), rotate(b["orientation"], [1, 0, 0]))) > 0.01

# Visible meshes must remain when all train overlays are disabled.
path = root / "assets-without-overlays.png"
meta = json.loads(path.with_suffix(".capture.json").read_text())
_, rows = png_pixels(path)
x, y, width, height = map(int, meta["viewport"])
gold = green = 0
for j in range(y, y+height):
    for i in range(x, x+width):
        r, g, b = rows[j][i*4:i*4+3]
        gold += r > 80 and r > g*1.1 and g > b*1.1
        green += g > 65 and g > r*1.2 and g > b*1.04
assert gold > 1000 and green > 100
print(f"M8.1 verified: GLB bytes/hierarchy, six rigid component transforms, pivots, joints, history/save/reopen/Save As evidence, focused captures at both sizes; {gold} body and {green} bogie pixels without overlays.")
print(f"Measured fixture file read + import: {evidence['import_and_file_read_ms']:.3f} ms. No manual desktop acceptance or physics playback claimed.")
