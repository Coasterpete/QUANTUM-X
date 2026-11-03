"""Audit original M2 source GLBs without rewriting them; standard library only.
Run: python tools/audit-box-spined.py [output.json]
Hashes record the originals inspected before M2 implementation.
"""
import hashlib
import json
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
ORIGINALS = {
    "100/Box-Spine-100.glb": "281bb95f8cc7a64decc92989265e38295649d05531466a685b3a500ada73df45",
    "100/Box-Spine-100-crosstie.glb": "62a0e68c6eb2716142538c291c2a3b61af43d498fe993914111db7ea8de90b73",
    "200/Box-Spine-200.glb": "29cf16ac60629672426a4dfda851e86d42e314f3573e4075b4beed097feac064",
    "200/Box-Spine-200-crosstie.glb": "0ae8da704bf2320593ebaa46c33b54ef5c397f4b77e150b6e1b5b7f275620f1c",
    "300/Box-Spine-300.glb": "b86f4071c35797854a9056327f12c8bd2276a88e099c9cd12c9b2de1f4932a83",
    "300/Box-Spine-300-crosstie.glb": "b398c9082decb3eb86197adcb39e01adfdb64bba8156719fcc1b3a6f28a413a2",
}
records = []
for name, original in ORIGINALS.items():
    path = ROOT / "assets/tracks/box-spined" / name
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    assert digest == original, f"Original asset differs from inspected M2 source: {path}"
    assert data[:4] == b"glTF" and len(data) == struct.unpack_from("<I", data, 8)[0]
    length = struct.unpack_from("<I", data, 12)[0]
    gltf = json.loads(data[20:20+length])
    records.append({
        "path": str(path.relative_to(ROOT)).replace("\\", "/"),
        "style_number_from_filename": int(name.split("/")[0]),
        "role_from_source_material": gltf["materials"][0]["name"],
        "sha256": digest, "unchanged": True, "bytes": len(data),
        "nodes": gltf["nodes"],
        "primitives": [
            {"vertices": gltf["accessors"][p["attributes"]["POSITION"]]["count"],
             "indices": gltf["accessors"][p["indices"]]["count"],
             "source_bounds": [gltf["accessors"][p["attributes"]["POSITION"]][k] for k in ("min", "max")],
             "uv_present": "TEXCOORD_0" in p["attributes"]}
            for mesh in gltf["meshes"] for p in mesh["primitives"]
        ],
        "embedded_images": gltf.get("images", []),
        "installed": name.startswith("100/"),
    })
report = {"original_glbs_unchanged": True, "track_blender_sources": [str(p.relative_to(ROOT)) for p in (ROOT / "assets/tracks").rglob("*.blend")], "assets": records}
text = json.dumps(report, indent=2) + "\n"
if len(sys.argv) > 1:
    Path(sys.argv[1]).write_text(text, encoding="utf-8")
print("Verified all six original Box-Spined GLBs unchanged; installed style 100 source mapping only.")
