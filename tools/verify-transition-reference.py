"""Independent standard-library checks of transition-scenario output.

python tools/verify-transition-reference.py <after-directory> [<before-directory>]
"""
import bisect
import csv
import json
import math
from pathlib import Path
import struct
import sys

root = Path(sys.argv[1])


def load(name):
    return json.loads((root / name).read_text(encoding="utf-8"))


def de_boor(net, knots, degree, u):
    span = max(degree, min(len(net)-1, bisect.bisect_right(knots, u)-1))
    work = [list(net[i]) for i in range(span-degree, span+1)]
    for level in range(1, degree+1):
        for j in range(degree, level-1, -1):
            i = span-degree+j
            den = knots[i+degree-level+1]-knots[i]
            alpha = (u-knots[i])/den if den else 0
            work[j] = [(1-alpha)*a+alpha*b for a, b in zip(work[j-1], work[j])]
    return work[degree]


def nurbs(t, station):
    p = t["function"]["profile"]
    degree, knots = p["degree"], p["knots"][:]
    net = [[v*w, w] for v, w in zip(p["values"], p["weights"])]
    u = min(1, max(0, station/t["length"]))
    jets = []
    for _ in range(3):
        jets.append(de_boor(net, knots, degree, u))
        net = [[degree*(b[c]-a[c])/(knots[i+degree+1]-knots[i+1])
                if knots[i+degree+1] != knots[i+1] else 0 for c in range(2)]
               for i, (a, b) in enumerate(zip(net, net[1:]))]
        knots = knots[1:-1]
        degree -= 1
    value = jets[0][0]/jets[0][1]
    first = (jets[1][0]-value*jets[1][1])/jets[0][1]
    second = (jets[2][0]-2*first*jets[1][1]-value*jets[2][1])/jets[0][1]
    return value, first/t["length"], second/t["length"]**2


def evaluate(project, station):
    transitions = project["sections"][0]["kind"]["pitch_rate"]["transitions"]
    start = 0
    for t in transitions:
        if station <= start+t["length"]:
            return nurbs(t, station-start)
        start += t["length"]
    raise AssertionError("station outside channel")


split = load("profile-after-split.qx.json")
with (root / "split-samples.csv").open(newline="", encoding="utf-8") as source:
    samples = list(csv.DictReader(source))
assert len(samples) == 10001
maximum = [0., 0., 0.]
for row in samples:
    station = float(row["station_m"])
    actual = evaluate(split, station)
    expected = [float(row[k]) for k in ("original_value", "original_derivative", "original_second")]
    for i, (a, b) in enumerate(zip(actual, expected)):
        maximum[i] = max(maximum[i], abs(a-b))
assert maximum[0] < 1e-13 and maximum[1] < 1e-12 and maximum[2] < 1e-11, maximum

shaped = load("profile-after-local-shape-edit.qx.json")
pieces = shaped["sections"][0]["kind"]["pitch_rate"]["transitions"]
left = nurbs(pieces[0], pieces[0]["length"])
right = nurbs(pieces[1], 0)
assert all(abs(a-b) < 1e-10 for a, b in zip(left, right))
assert abs(evaluate(shaped, 6)[0]-evaluate(split, 6)[0]) > 1e-5
evidence = load("numerical-evidence.json")
check = evidence["local_shape_check"]
assert check["independent_planar_position_error_m"] < 1e-7
assert check["independent_integrated_pitch_error_rad"] < 1e-8
assert check["authored_graph_error_rad_per_m"] < 1e-14
assert check["midpoint_curvature_difference_per_m"] < 2e-5
assert evidence["maximum_split_error"]["position_m"] < 1e-11
assert evidence["maximum_split_error"]["curvature_per_m"] < 1e-10
for png in root.glob("*.png"):
    raw = png.read_bytes()
    assert raw[:8] == b"\x89PNG\r\n\x1a\n"
    width, height = struct.unpack(">II", raw[16:24])
    capture = load(png.stem+".capture.json")
    assert capture["size"] == [width, height]
    x, y, w, h = capture["viewport"]
    assert all(math.isfinite(v) for v in [x, y, w, h])
    assert x >= 0 and y >= 0 and w > 0 and h > 0
    assert x+w <= width+1 and y+h <= height+1
if len(sys.argv) > 2:
    before = Path(sys.argv[2])
    for fixture in before.glob("*.qx.json"):
        assert json.loads(fixture.read_text(encoding="utf-8")) == load(fixture.name), fixture.name
print(f"Verified 10,001 split samples independently: value/slope/second errors {maximum}; "
      "shared C2 shape edit, changed interior, independent geometry integration, "
      "bounded offscreen captures and unchanged before/after fixture documents.")
