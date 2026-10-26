"""Independent checks of the opt-in M6 workflow exports; standard library only.

Run: python tools/verify-m6-exports.py target/m6/scenario
These are design-speed geometry checks, never ride-safety or dynamics checks.
"""
import csv
import json
import math
import struct
import sys
from pathlib import Path


def rows(path):
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source))


root = Path(sys.argv[1])
project = json.loads((root / "project.qx.json").read_text(encoding="utf-8"))
assert project["schema_version"] == 3
sections = rows(root / "sections.csv")
assert [s["method"] for s in sections] == ["straight", "force"]
assert sections[0]["end_sample"] == sections[1]["start_sample"]
samples = rows(root / "centerline.csv")
assert len(samples) == 901
assert abs(float(samples[-1]["distance_m"]) - 90) < 1e-9
previous_distance = -1
norm_error = orthogonal_error = 0
for row in samples:
    assert all(math.isfinite(float(value)) for value in row.values())
    distance = float(row["distance_m"])
    assert distance > previous_distance
    previous_distance = distance
    axes = [[float(row[prefix + axis]) for axis in "xyz"]
            for prefix in ("tangent_", "rider_normal_", "lateral_")]
    norm_error = max(norm_error, *(abs(sum(v*v for v in a) - 1) for a in axes))
    orthogonal_error = max(orthogonal_error, *(abs(sum(x*y for x, y in zip(axes[i], axes[j])))
                                             for i, j in ((0, 1), (0, 2), (1, 2))))
assert norm_error < 1e-12 and orthogonal_error < 1e-12
peak = max(float(s["y_m"]) for s in samples)
assert 10 < peak < 11
assert any(float(s["tangent_y"]) < -0.1 for s in samples)
join = samples[int(sections[1]["start_sample"])]
assert abs(float(join["x_m"]) - 10) < 1e-12
assert abs(float(join["y_m"])) < 1e-12 and abs(float(join["z_m"])) < 1e-12

# Independent evaluation of the deliberately authored quintic profiles at every
# exported interval midpoint. Compare those inputs to independently derived loads.
profiles = {
    "normal": [(0, 22, 1, 2), (22, 40, 2, 0), (40, 60, 0, 0), (60, 80, 0, 1)],
    "lateral": [(0, 40, 0, .1), (40, 80, .1, 0)],
    "roll": [(0, 40, 0, .006), (40, 80, .006, 0)],
}


def value(channel, x):
    a, b, start, end = next(p for p in profiles[channel] if p[0] <= x <= p[1])
    u = (x - a) / (b - a)
    return start + (end - start) * (6*u**5 - 15*u**4 + 10*u**3)


measurements = [r for r in rows(root / "measurements.csv") if r["section_id"] == "2"]
profile_error = load_error = lateral_error = 0
for row in measurements:
    x = float(row["input"])
    for channel, key in (("normal", "authored_normal_g"),
                         ("lateral", "authored_lateral_g"), ("roll", "authored_roll_rate")):
        profile_error = max(profile_error, abs(float(row[key]) - value(channel, x)))
    load_error = max(load_error, abs(float(row["derived_normal_g"]) - float(row["authored_normal_g"])))
    lateral_error = max(lateral_error, abs(float(row["derived_lateral_g"]) - float(row["authored_lateral_g"])))
assert profile_error < 1e-12
assert load_error < 2e-5 and lateral_error < 2e-6
for row in rows(root / "transitions.csv"):
    if row["section_id"] == "2" and row["endpoint_side"] != "interior":
        assert abs(float(row["derivative"])) < 1e-12

for width, height in ((1440, 960), (960, 640)):
    for stem in ("new-project", "hill-normal", "hill-lateral", "hill-roll"):
        image = (root / f"{stem}-{width}.png").read_bytes()
        assert image[:8] == b"\x89PNG\r\n\x1a\n"
        assert struct.unpack(">II", image[16:24]) == (width, height)

report = dict(length_m=previous_distance, samples=len(samples), peak_elevation_m=peak,
              frame_squared_norm_error=norm_error, frame_dot_error=orthogonal_error,
              authored_profile_max_error=profile_error, normal_load_residual_g=load_error,
              lateral_load_residual_g=lateral_error, verified_pngs=8)
(root / "verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
