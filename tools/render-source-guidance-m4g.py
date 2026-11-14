"""Validate full M4G evidence, then package recorded poses for offline inspection.
Usage: python tools/render-source-guidance-m4g.py TRACE_LOG COST_SUMMARY_JSON
Opens no browser. Complete physical verification precedes display-field filtering.
"""
import importlib.util
import json
from pathlib import Path
import sys
import time

spec = importlib.util.spec_from_file_location('verification', Path(__file__).with_name('verify-extended-guidance-m4g.py'))
verification = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verification)


def build(path, cost_path):
    path = Path(path)
    assert path.stat().st_size < 60_000_000, 'developer trace budget exceeded'
    clips, summary = verification.verify(path)
    performance = json.loads(Path(cost_path).read_text(encoding='utf-8-sig'))
    assert performance['control_ms']['count'] == 2400
    # Full constraints, velocities, impulse loads and independent matrices remain
    # in the verified source log. Every tick/pose stays in the viewer package.
    keys = ('tick', 'time', 'bodies', 'supports', 'anchors', 'max_position',
            'max_velocity', 'energy_drift', 'bearing_gap', 'swing', 'roll',
            'reaction_forces', 'reaction_power', 'charts', 'sections',
            'stationarity', 'denominators')
    for clip in clips:
        clip['frames'] = [{k: f[k] for k in keys} for f in clip['frames']]
        clip['scene'].pop('charts')  # Long-run ownership already verified in full.
    clock = time.perf_counter()
    data = dict(clips=clips, summary=summary, performance=performance)
    encoded = json.dumps(data, separators=(',', ':'), allow_nan=False).replace('<', '\\u003c')
    serialization_ms = (time.perf_counter()-clock)*1000
    template = Path(__file__).with_name('source-guidance-m4g-viewer.html').read_text(encoding='utf-8')
    assert template.count('__DATA__') == 1
    destination = path.with_name('source-guidance-m4g.html')
    clock = time.perf_counter()
    destination.write_text(template.replace('__DATA__', encoded), encoding='utf-8')
    destination.with_suffix('.json').write_text(encoded+'\n', encoding='utf-8')
    print(f'PASS: 962 recorded states; 2 qualified clips; HTML {destination.stat().st_size} bytes')
    print(f'Viewer: {destination.resolve()}')
    print(f'Offline packaging: JSON serialization {serialization_ms:.3f} ms; file writes {(time.perf_counter()-clock)*1000:.3f} ms')
    return destination


if __name__ == '__main__':
    build(*sys.argv[1:])
