"""Summarize complete-step samples separately from instrumented attribution.
python tools/summarize-performance-m4h.py COST_LOG PROFILE_LOG
"""
import json
import math
from pathlib import Path
import statistics
import sys


def read(path, tag):
    return [json.loads(line.split(tag, 1)[1]) for line in Path(path).read_text(encoding='utf-8-sig').splitlines() if tag in line]


def stats(values):
    values = sorted(values)
    def percentile(q):
        x = (len(values)-1)*q
        a = math.floor(x)
        b = math.ceil(x)
        return values[a]+(values[b]-values[a])*(x-a)
    return dict(samples=len(values), median_ms=statistics.median(values), p95_ms=percentile(.95), p99_ms=percentile(.99), maximum_ms=max(values), deadline_misses=sum(x > 1000/240 for x in values))


def summarize(cost_path, profile_path):
    costs = read(cost_path, 'M4H_COST ')
    profiles = read(profile_path, 'M4H_PROFILE ')
    if len(costs) != 4800 or len(profiles) != 1920:
        raise ValueError('missing repeated-run/profile evidence')
    summary = {}
    for name in ('helix', 'foundation'):
        rows = [r for r in costs if r['fixture'] == name]
        if {(r['run'], r['tick']) for r in rows} != {(run, tick) for run in range(1, 6) for tick in range(1, 481)}:
            raise ValueError('incomplete fixture/run/tick distribution')
        d = {k: stats([r[k+'_ms'] for r in rows]) for k in ('original', 'cached')}
        d['ratio_of_medians'] = d['original']['median_ms']/d['cached']['median_ms']
        d['per_run'] = {str(run): {k: stats([r[k+'_ms'] for r in rows if r['run'] == run]) for k in ('original', 'cached')} for run in range(1, 6)}
        d['attribution'] = {}
        for cached in (False, True):
            p = [r for r in profiles if r['fixture'] == name and r['cached'] == cached]
            if [r['tick'] for r in p] != list(range(1, 481)):
                raise ValueError('missing instrumented fixture ticks')
            def columns(key):
                return [sum(r[key][i] for r in p) for i in range(len(p[0][key]))]
            d['attribution'][str(cached)] = {
                'mechanical_counts': columns('counts'), 'diagnostic_counts': columns('diagnostic_counts'),
                'mechanical_cost_totals_ms': columns('costs_ms'), 'diagnostic_cost_totals_ms': columns('diagnostic_costs_ms'),
                'instrumented_complete': stats([r['complete_ms'] for r in p]),
                'separate_reaction_diagnostic': stats([r['reaction_ms'] for r in p]),
            }
        summary[name] = d
    output = Path(cost_path).with_suffix('.summary.json')
    output.write_text(json.dumps(summary, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(summary, indent=2))
    print('PASS: complete 480-tick timing distributions and separate geometry/reaction attribution')


if __name__ == '__main__':
    summarize(*sys.argv[1:])
