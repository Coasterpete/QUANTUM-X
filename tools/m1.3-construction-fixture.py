"""Original synthetic inspection data, MIT OR Apache-2.0; no private NL2 input.
Regenerate: python tools/m1.3-construction-fixture.py
This is deliberately banked/inverted solver test geometry, not a designed ride.
"""
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "crates/qx-geometry/tests/import-fixtures"

def cross(a, b):
    return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])

def unit(v):
    length = math.sqrt(sum(x*x for x in v))
    return tuple(x/length for x in v)

rows, stations, points = [], [], []
length = 0.0
for i in range(161):
    a = i / 160 * 2.4
    p = (100 + 35*math.sin(a), 40 + 12*math.sin(2*a), -70 + 35*(1-math.cos(a)))
    t = unit((35*math.cos(a), 24*math.cos(2*a), 35*math.sin(a)))
    lateral = unit(cross(t, (0, 1, 0)))
    n = cross(lateral, t)
    bank = math.pi * math.sin(math.pi*i/160)**2
    n = tuple(n[j]*math.cos(bank)+lateral[j]*math.sin(bank) for j in range(3))
    lateral = cross(t, n)
    if points:
        length += math.dist(points[-1], p)
    points.append(p)
    stations.append(length)
    rows.append([i+1, *p, *t, *(-x for x in lateral), *n])
header = 'No.\tPosX\tPosY\tPosZ\tFrontX\tFrontY\tFrontZ\tLeftX\tLeftY\tLeftZ\tUpX\tUpY\tUpZ'
(ROOT / 'construction.csv').write_text(header+'\n'+'\n'.join('\t'.join(str(x) for x in r) for r in rows)+'\n')
segments = []
for i, (start, end) in enumerate(zip([0.0, 24.25, 70.75], [24.25, 70.75, length])):
    segments.append(dict(id=f"synthetic-{i}", start_m=start, end_m=end, style_id="box-spined-100", variant_id="diagnostic-double" if i == 1 else "source-single"))
(ROOT / 'construction-map.json').write_text(json.dumps(dict(version=1, segments=segments), indent=2)+'\n')
xml = ['<!-- Original synthetic support positions; no verified connector solids. MIT OR Apache-2.0. -->', '<root><supports>']
for i in (12, 48, 100, 144):
    p = points[i]
    xml += [f'<rasc type="2" center_rails_coord="{stations[i]}" custom_track_index="0"><subnode id="rail-{i}"><pos>{p[0]} {p[1]-1} {p[2]}</pos></subnode></rasc>',
            f'<footernode id="footer-{i}" contype="0" basetype="1"><pos>{p[0]} 20 {p[2]}</pos></footernode>',
            f'<beam start="rail-{i}" end="footer-{i}" type="1" size1="0.3"/>']
xml += ['</supports></root>']
(ROOT / 'construction-supports.xml').write_text('\n'.join(xml)+'\n')
print(f'Original synthetic track: {len(rows)} samples, {length:.12f} m; boundaries 24.25, 70.75 m')
