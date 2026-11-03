"""Create an explicit DEVELOPMENT ONLY map from a diagnostic station extent.
Does not parse, edit or reconstruct imported geometry. Output must not exist.
Usage: python tools/m1.3-map-from-report.py report.json new-map.json 250.25 700.75
"""
import json
import math
from pathlib import Path
import sys

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))
assert report["diagnostic_only"] is True
length = report["station_extent_m"]
cuts = [0.0, *(float(s) for s in sys.argv[3:]), length]
assert all(math.isfinite(s) for s in cuts)
assert all(a < b for a, b in zip(cuts, cuts[1:])), "cuts must increase inside track extent"
segments = [dict(id=f"diagnostic-{i}", start_m=a, end_m=b, style_id="box-spined-100",
                 variant_id="source-single" if i % 2 == 0 else "diagnostic-double")
            for i, (a, b) in enumerate(zip(cuts, cuts[1:]))]
with Path(sys.argv[2]).open("x", encoding="utf-8") as output:
    json.dump(dict(version=1, segments=segments), output, indent=2)
    output.write("\n")
print(f"Created {len(segments)} explicit segments, full coverage 0..{length} m")
