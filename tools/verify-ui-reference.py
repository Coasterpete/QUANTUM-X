"""Validate fresh ui-reference-scenario GPU captures. No third-party packages."""
import json
import math
import struct
import sys
import zlib
from pathlib import Path


def pixels(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    compressed = bytearray()
    offset = 8
    while offset < len(data):
        size = int.from_bytes(data[offset:offset + 4], "big")
        tag, body = data[offset + 4:offset + 8], data[offset + 8:offset + 8 + size]
        if tag == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert (depth, color, interlace) == (8, 6, 0)
        elif tag == b"IDAT":
            compressed.extend(body)
        offset += size + 12
    raw = zlib.decompress(compressed)
    stride = width * 4
    previous = bytearray(stride)
    rows = []
    for y in range(height):
        start = y * (stride + 1)
        kind = raw[start]
        row = bytearray(raw[start + 1:start + 1 + stride])
        for x in range(stride):
            a, b, c = (row[x - 4] if x >= 4 else 0), previous[x], (previous[x - 4] if x >= 4 else 0)
            if kind == 1:
                predictor = a
            elif kind == 2:
                predictor = b
            elif kind == 3:
                predictor = (a + b) // 2
            elif kind == 4:
                p = a + b - c
                distances = (abs(p - a), abs(p - b), abs(p - c))
                predictor = (a, b, c)[distances.index(min(distances))]
            else:
                assert kind == 0
                predictor = 0
            row[x] = (row[x] + predictor) & 255
        rows.append(row)
        previous = row
    return (width, height), rows


root = Path(sys.argv[1])
evidence = json.loads((root / "evidence.json").read_text())
assert evidence["automated_offscreen"] and not evidence["manual_desktop_tested"]
assert evidence["project_unchanged"] and evidence["no_implicit_document"]
assert evidence["schema_version"] == 6
palette = [(164, 255, 0), (100, 132, 255), (188, 112, 255),
           (52, 255, 97), (255, 83, 91), (255, 139, 54)]
for capture in evidence["captures"]:
    path = root / capture["file"]
    size, rows = pixels(path)
    assert list(size) == capture["size"]
    meta = json.loads(path.with_suffix(".capture.json").read_text())
    assert meta["size"] == list(size)
    x, y, w, h = meta["viewport"]
    assert all(math.isfinite(v) for v in (x, y, w, h))
    assert x >= 0 and y >= 0 and w > 0 and h > 0
    assert x + w <= size[0] + 1 and y + h <= size[1] + 1
    if capture["page"] == "welcome":
        counts = {color: 0 for color in palette}
        black = 0
        for row in rows:
            for i in range(0, len(row), 4):
                color = tuple(row[i:i + 3])
                if color in counts:
                    counts[color] += 1
                if color == (0, 0, 0):
                    black += 1
        assert min(counts.values()) > 30, (path, counts)
        assert black / (size[0] * size[1]) > .75, path
for page in ("welcome", "editor", "train"):
    actual = {(tuple(c["size"]), c["scale"]) for c in evidence["captures"] if c["page"] == page}
    assert actual == {((1440, 960), 1), ((960, 640), 1), ((1440, 960), 1.5),
                      ((1440, 960), 2), ((960, 640), 1.5), ((960, 640), 2)}, (page, actual)
print(f"Verified {len(evidence['captures'])} real GPU captures: sizes/viewports, six Welcome colors on black, 100/150/200% UI scaling, and unchanged schema-6 project. Native desktop interaction was not tested.")
