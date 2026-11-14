"""Summarize actual serial complete-step observations; no real-time guarantee."""
import json
import math
from pathlib import Path
import sys


def stats(values):
    values = sorted(values)
    def percentile(p):
        return values[max(0, math.ceil(p*len(values))-1)]
    return dict(count=len(values), minimum_ms=values[0], median_ms=percentile(.5),
                p95_ms=percentile(.95), p99_ms=percentile(.99), maximum_ms=values[-1])


def summarize(path):
    steps = [json.loads(line.split('M4G_STEP_COST ', 1)[1])
             for line in Path(path).read_text(encoding='utf-8-sig').splitlines()
             if 'M4G_STEP_COST ' in line]
    assert len(steps) == 2400
    assert {(s['run'], s['tick']) for s in steps} == {(r, t) for r in range(1, 6) for t in range(1, 481)}
    phases = ('rail_ms', 'constraints_ms', 'impulses_ms', 'validation_tracking_ms', 'other_ms')
    for s in steps:
        assert all(math.isfinite(s[k]) and s[k] >= 0 for k in (*phases, 'complete_ms'))
        assert abs(sum(s[k] for k in phases)-s['complete_ms']) < 1e-5
    result = {k: stats([s[k] for s in steps]) for k in ('control_ms', 'complete_ms', *phases)}
    result['deadline_ms'] = 1000/240
    result['observed_misses'] = sum(s['complete_ms'] > 1000/240 for s in steps)
    result['control_misses'] = sum(s['control_ms'] > 1000/240 for s in steps)
    Path(path).with_suffix('.summary.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    summarize(sys.argv[1])
