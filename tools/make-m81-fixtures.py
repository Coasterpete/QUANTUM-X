"""Generate original M8.1 development GLBs, using Python's standard library.
No production train artwork or third-party geometry. Units: metres, +X forward,
+Y up, +Z lateral. Run from any directory; output is assets/development/train/.
"""
import json
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "assets/development/train"


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xffffffff)


def make(name, color, parts):
    vertices, normals, uvs, indices = [], [], [], []
    for n, u, v in [((1, 0, 0), (0, 1, 0), (0, 0, 1)),
                    ((-1, 0, 0), (0, 0, 1), (0, 1, 0)),
                    ((0, 1, 0), (0, 0, 1), (1, 0, 0)),
                    ((0, -1, 0), (1, 0, 0), (0, 0, 1)),
                    ((0, 0, 1), (1, 0, 0), (0, 1, 0)),
                    ((0, 0, -1), (0, 1, 0), (1, 0, 0))]:
        base = len(vertices)
        for x, y in [(-1, -1), (1, -1), (1, 1), (-1, 1)]:
            vertices.append(tuple((n[i] + u[i]*x + v[i]*y)*0.5 for i in range(3)))
            normals.append(n)
            uvs.append(((x+1)/2, (y+1)/2))
        indices.extend(base+i for i in [0, 1, 2, 0, 2, 3])
    binary = bytearray()
    views = []
    for data in [struct.pack("<72f", *(c for p in vertices for c in p)),
                 struct.pack("<72f", *(c for p in normals for c in p)),
                 struct.pack("<48f", *(c for p in uvs for c in p)),
                 struct.pack("<36H", *indices)]:
        views.append({"buffer": 0, "byteOffset": len(binary), "byteLength": len(data)})
        binary.extend(data)
    # A tiny original checker texture also exercises embedded image decoding.
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 2, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(b"\x00" + bytes([255, 255, 255, 255, 180, 180, 180, 255]) + b"\x00" + bytes([180, 180, 180, 255, 255, 255, 255, 255]))) + chunk(b"IEND", b"")
    views.append({"buffer": 0, "byteOffset": len(binary), "byteLength": len(png)})
    binary.extend(png)
    nodes = [{"name": "DEVELOPMENT ONLY root", "translation": [0, 0.2, 0], "children": list(range(1, len(parts)+1))}]
    for part, position, scale in parts:
        nodes.append({"name": part, "mesh": 0, "translation": position, "scale": scale})
    data = {"asset": {"version": "2.0", "generator": "QUANTUM-X original M8.1 verification fixture"},
            "scene": 0, "scenes": [{"nodes": [0]}], "nodes": nodes,
            "buffers": [{"byteLength": len(binary)}], "bufferViews": views,
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 24, "type": "VEC3", "min": [-0.5]*3, "max": [0.5]*3},
                          {"bufferView": 1, "componentType": 5126, "count": 24, "type": "VEC3"},
                          {"bufferView": 2, "componentType": 5126, "count": 24, "type": "VEC2"},
                          {"bufferView": 3, "componentType": 5123, "count": 36, "type": "SCALAR"}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2}, "indices": 3, "material": 0}]}],
            "materials": [{"name": "Development checker", "pbrMetallicRoughness": {"baseColorFactor": color + [1], "metallicFactor": 0.15, "roughnessFactor": 0.6, "baseColorTexture": {"index": 0}}}],
            "textures": [{"source": 0}], "images": [{"bufferView": 4, "mimeType": "image/png"}]}
    encoded = json.dumps(data, separators=(",", ":")).encode()
    encoded += b" " * (-len(encoded) % 4)
    binary.extend(b"\x00" * (-len(binary) % 4))
    body = struct.pack("<I4s", len(encoded), b"JSON") + encoded + struct.pack("<I4s", len(binary), b"BIN\x00") + binary
    (ROOT / name).write_bytes(struct.pack("<4sII", b"glTF", 2, len(body)+12) + body)


ROOT.mkdir(parents=True, exist_ok=True)
make("development-body.glb", [0.85, 0.48, 0.12], [
    ("floor", [0, 0.1, 0], [3, 0.2, 1.45]),
    ("left shell", [0, 0.55, -0.67], [3, 0.7, 0.12]),
    ("right shell mirrored", [0, 0.55, 0.67], [-3, 0.7, 0.12]),
    ("nose", [1.4, 0.45, 0], [0.2, 0.5, 1.45]),
    ("seat reference", [-0.45, 0.4, 0], [0.2, 0.55, 1.15]),
])
make("development-bogie.glb", [0.15, 0.6, 0.47], [
    ("cross beam", [0, -0.2, 0], [0.65, 0.16, 1.1]),
    ("left block", [0, -0.18, -0.55], [0.45, 0.42, 0.18]),
    ("right block", [0, -0.18, 0.55], [0.45, 0.42, 0.18]),
])
print(ROOT)
