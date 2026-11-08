"""Independent numerical/pixel checks for ride-camera-scenario evidence."""
import json
import math
import sys
import subprocess
from pathlib import Path
import struct
import zlib


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



def vector(v):
    return [v[k] for k in "xyz"]


def add(a, b):
    return [x + y for x, y in zip(a, b)]


def scale(a, s):
    return [x * s for x in a]


def rotate(q, v):
    # Independent expanded Hamilton rotation, using the serialized unit quaternion.
    w, x, y, z = (q[k] for k in "wxyz")
    assert abs(w*w + x*x + y*y + z*z - 1) < 1e-10
    return [
        (1-2*(y*y+z*z))*v[0] + 2*(x*y-z*w)*v[1] + 2*(x*z+y*w)*v[2],
        2*(x*y+z*w)*v[0] + (1-2*(x*x+z*z))*v[1] + 2*(y*z-x*w)*v[2],
        2*(x*z-y*w)*v[0] + 2*(y*z+x*w)*v[1] + (1-2*(x*x+y*y))*v[2],
    ]


def close(a, b, tolerance=2e-9):
    assert max(abs(x-y) for x, y in zip(a, b)) < tolerance, (a, b)


def unit(v):
    norm = math.sqrt(sum(x*x for x in v))
    assert norm > 1e-9
    return scale(v, 1/norm)


def verify(root):
    evidence = json.loads((root / "evidence.json").read_text())
    project = json.loads((root / "inspection.qx.json").read_text())
    train = next(t for t in project["trains"] if t["id"] == project["heartline"]["train_configuration_id"])
    definitions = {c["id"]: c for c in train["components"]}
    assert evidence["input_unchanged"] and evidence["project_roundtrip"]
    assert evidence["offscreen"] and not evidence["native_desktop_tested"]
    assert evidence["dt_s"] == 1/240 and evidence["fps"] in (12,60)
    assert [r["mode"] for r in evidence["records"]] == ["free", "rider", "front", "rear", "chase", "track"]
    assert len(evidence["frames"]) == 24 if evidence["fps"] == 12 else len(evidence["frames"]) >= 60
    max_residual = 0
    for r in evidence["records"] + evidence["frames"]:
        components = {c["id"]: c for c in r["components"]}
        assert set(components) == set(definitions)
        assert len(r["instances"])==len(components)
        for instance in r["instances"]:
            c = components[instance["id"]]
            d = definitions[c["id"]]
            asset = d.get("asset")
            if asset:
                rotation = asset["rotation"]
                stretch = vector(asset["scale"])
                shift = vector(asset["translation_m"])
                pivot = vector(asset.get("pivot_m", dict(x=0, y=0, z=0)))
            else:
                rotation = dict(w=1, x=0, y=0, z=0)
                stretch = vector(d["dimensions_m"])
                shift, pivot = [0, 0, 0], [0, 0, 0]
            columns = []
            for k in range(3):
                axis = [0, 0, 0]
                axis[k] = stretch[k]
                columns.append(rotate(c["orientation"], rotate(rotation, axis)))
            origin = add(vector(c["position_m"]), rotate(c["orientation"], add(shift, scale(rotate(rotation, [a*b for a, b in zip(pivot, stretch)]), -1))))
            for actual, expected in zip(instance["matrix"], columns + [origin]):
                close(actual[:3], expected)
        camera = r["camera"]
        mode = r.get("mode", "rider")
        if mode == "free":
            assert camera is None
        elif mode != "track":
            body = train["cars"][-1 if mode == "rear" else 0]["body_id"]
            c = components[body]
            offset = [-8, 3, 4] if mode == "chase" else [0, 1.2, 0]
            expected_eye = add(vector(c["position_m"]), rotate(c["orientation"], offset))
            close(vector(camera["eye_m"]), expected_eye)
            forward = rotate(c["orientation"], [1, 0, 0])
            if mode == "chase":
                forward = unit(add(add(vector(c["position_m"]), scale(forward, 2)), scale(expected_eye, -1)))
            close(vector(camera["forward"]), forward)
            if mode != "chase":
                close(vector(camera["up"]), rotate(c["orientation"], [0, 1, 0]))
        if camera:
            f, u = vector(camera["forward"]), vector(camera["up"])
            assert abs(sum(x*y for x, y in zip(f, u))) < 1e-10
            close(unit(f), f)
            close(unit(u), u)
        file = r.get("file", f"{mode}.png")
        metadata = json.loads((root / file).with_suffix(".capture.json").read_text())
        actual = metadata["attached_camera"]
        assert (camera is None) == (actual is None)
        if camera:
            for key in ["eye_m", "forward", "up"]:
                close(vector(actual[key]), vector(camera[key]))
        # All frame transforms/cameras are checked above. Decode every recording
        # PNG with FFmpeg below; sample color content here without a slow Python
        # unfilter loop over hundreds of full-resolution frames.
        if evidence["fps"]==12 or "mode" in r or r.get("frame",0)%20==0:
            size, rows = pixels(root / file)
            assert list(size) == metadata["size"]
            x, y, w, h = map(int, metadata["viewport"])
            colors = set()
            for row in rows[y:y+h:3]:
                colors.update(bytes(row[i:i+3]) for i in range(x*4, (x+w)*4, 12))
            if evidence["fps"]==12 or "mode" in r:
                assert len(colors) > 50, (file,len(colors))
            else:
                # A POV approaching an open endpoint can legitimately show
                # mostly sky and the flat matte car (only a few shaded colors).
                # Require a substantial visible foreground, not color variety.
                background=bytes(rows[y][x*4:x*4+3])
                foreground=sum(bytes(row[i:i+3])!=background for row in rows[y:y+h:3] for i in range(x*4,(x+w)*4,12))
                assert foreground>100,(file,foreground)
        max_residual = max(max_residual, max(j["error_m"] for j in r["joints"]))
    if evidence["fps"] == 12:
        assert [f["tick"] for f in evidence["frames"]] == list(range(0,480,20))
    else:
        assert all(abs(f["presentation_tick"]-(i*4+.5))<1e-5 for i,f in enumerate(evidence["frames"]))
        raw=subprocess.check_output(["ffmpeg","-v","error","-framerate","60","-i",str(root/"pov60/pov-frames/%04d.png"),"-f","framemd5","-"],text=True)
        hashes=[line.split(",")[-1].strip() for line in raw.splitlines() if line and not line.startswith("#")]
        assert len(hashes)==len(evidence["frames"])==len(set(hashes))
    first, last = evidence["frames"][0], evidence["frames"][-1]
    distance = math.dist(vector(first["components"][0]["position_m"]), vector(last["components"][0]["position_m"]))
    assert distance > 0.1
    assert max_residual < 0.02
    print(f'PASS: six modes, {len(evidence["frames"])} physics frames, camera/instance agreement, rendered pixels; displacement {distance:.6f} m, max joint residual {max_residual:.9f} m')


if __name__ == "__main__":
    verify(Path(sys.argv[1]))
