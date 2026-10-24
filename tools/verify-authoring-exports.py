"""Independent standard-library checks of `quantum-x examples` output.

Usage: python tools/verify-authoring-exports.py target/pass2-release
"""
import csv
import json
import math
import sys
from pathlib import Path
from xml.etree import ElementTree


def rows(path):
    with path.open(newline="", encoding="utf-8") as source:
        return list(csv.DictReader(source))


def vector(row, prefix):
    return [float(row[prefix + axis]) for axis in "xyz"]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


root = Path(sys.argv[1])
report = {}
for name in ("spline-comparison", "mixed-authoring", "numerical-reference"):
    directory = root / name
    samples = rows(directory / "centerline.csv")
    measurements = rows(directory / "measurements.csv")
    sections = rows(directory / "sections.csv")
    methods = {row["method"] for row in sections}
    assert {"bezier", "b_spline", "nurbs", "catmull_rom"} <= methods
    assert len(measurements) + 1 == len(samples)
    norm_error = dot_error = 0.0
    previous = -1.0
    for row in samples:
        assert all(math.isfinite(float(value)) for value in row.values())
        distance = float(row["distance_m"])
        assert distance > previous
        previous = distance
        t = vector(row, "tangent_")
        n = vector(row, "rider_normal_")
        l = vector(row, "lateral_")
        norm_error = max(norm_error, *(abs(dot(v, v) - 1) for v in (t, n, l)))
        dot_error = max(dot_error, abs(dot(t, n)), abs(dot(t, l)), abs(dot(n, l)))
    assert norm_error < 1e-12 and dot_error < 1e-12
    for filename in ("trajectory.svg", "measurements.svg", "transitions.svg"):
        ElementTree.parse(directory / filename)
    report[name] = dict(samples=len(samples), length_m=previous,
                        frame_squared_norm_error=norm_error, frame_dot_error=dot_error)
    if name == "numerical-reference":
        total_error = abs(previous - (80 + 10 * math.pi))
        circle = next(s for s in sections if s["method"] == "nurbs")
        point_error = 0.0
        for row in samples[int(circle["start_sample"]):int(circle["end_sample"]) + 1]:
            theta = (float(row["distance_m"]) - 50) / 20
            exact = (50 + 20 * math.sin(theta), 20 * (1 - math.cos(theta)), 0)
            point_error = max(point_error, math.dist(
                [float(row[a + "_m"]) for a in "xyz"], exact))
        assert total_error < 1e-8 and point_error < 1e-7
        report[name].update(analytic_total_error_m=total_error,
                            circle_position_error_m=point_error,
                            circle_curvature_error_per_m=max(abs(float(g["curvature_per_m"]) - 0.05)
                                for g in measurements if g["section_id"] == circle["section_id"]))
print(json.dumps(report, indent=2))
