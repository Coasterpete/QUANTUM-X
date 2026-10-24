"""Independent standard-library analytic check of the heartline circle export."""
import csv
import json
import math
from pathlib import Path
import sys

directory = Path(sys.argv[1])
with (directory / "reference-points.csv").open(newline="", encoding="utf-8") as stream:
    rows = [{key: float(value) for key, value in row.items()} for row in csv.DictReader(stream)]
assert rows and all(math.isfinite(value) for row in rows for value in row.values())
assert all(b["source_s_m"] > a["source_s_m"] for a, b in zip(rows, rows[1:]))
radius_error = max(abs(math.hypot(r["x_m"], r["y_m"] - 20, r["z_m"]) - 18) for r in rows)
curvature_error = max(abs(r["curvature_per_m"] - 1 / 18) for r in rows)
speed_error = max(abs(r["design_speed_mps"] - 18) for r in rows)
load_error = max(abs(r["normal_g"] - (18 / 9.80665 + math.cos(r["source_s_m"] / 20))) for r in rows)
length_error = abs(rows[-1]["reference_s_m"] - 18 * math.tau)
assert radius_error < 1e-8, radius_error
assert curvature_error < 2e-7, curvature_error
assert speed_error < 1e-7, speed_error
assert load_error < 1e-5, load_error
assert length_error < 1e-6, length_error
print(json.dumps(dict(samples=len(rows), length_m=rows[-1]["reference_s_m"],
                     radius_error_m=radius_error, curvature_error_per_m=curvature_error,
                     speed_error_mps=speed_error, normal_load_error_g=load_error,
                     length_error_m=length_error), indent=2))
