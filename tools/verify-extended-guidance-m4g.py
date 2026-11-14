"""Independent M4G full-tick trace, continuation and reaction verification.
python tools/verify-extended-guidance-m4g.py TRACE_LOG [REPLAY_LOG]
No browser or live simulation. NumPy is used only for independent linear algebra.
"""
import bisect
import json
import math
from pathlib import Path
import sys
import numpy as np


def read(path):
    clips = []
    systems = []
    for line in Path(path).read_text(encoding='utf-8-sig').splitlines():
        for marker in ('M4G_SCENE ', 'M4G_FRAME ', 'M4G_SYSTEM '):
            if marker not in line:
                continue
            data = json.loads(line.split(marker, 1)[1])
            if marker == 'M4G_SCENE ':
                clips.append({'scene': data, 'frames': []})
            elif marker == 'M4G_FRAME ':
                clips[-1]['frames'].append(data)
            else:
                systems.append(data)
    return clips, systems


def world(body, local):
    q = np.array(body[3:7])
    v = np.array(local)
    t = 2 * np.cross(q[1:], v)
    return np.array(body[:3]) + v + q[0] * t + np.cross(q[1:], t)


def verify(path, replay=None):
    clips, systems = read(path)
    assert len(clips) == 2 and len(systems) == 12
    if replay:
        assert (clips, systems) == read(replay), 'recorded states/diagnostics must replay exactly'
    summary = {}
    for clip in clips:
        scene, frames = clip['scene'], clip['frames']
        assert len(frames) == 481 and scene['dt'] == 1/240
        assert scene['roles'].count('Car') == 2 and scene['roles'].count('Bogie') == 3
        assert len(scene['roles']) == 11 and len(scene['running']) == 30
        assert len(scene['edges']) == 10 and scene['source'] == [0x4fb, 1]
        # Shared bogie 3 is one body used by both car-bearing pairs.
        assert [3, 9] in scene['edges'] and [3, 10] in scene['edges']
        starts = [c[0] for c in scene['charts']]
        previous = None
        for tick, f in enumerate(frames):
            assert f['tick'] == tick and abs(f['time']-tick/240) < 5e-16
            assert len(f['bodies']) == 11 and len(f['supports']) == 30
            assert len(f['constraints']) == 65 and len(f['velocity_rows']) == 65
            assert len(f['velocity']) == 66 and len(f['reactions']) == 65
            assert f['max_position'] < 2e-5 and f['max_velocity'] < .003
            assert max(map(abs, f['constraints'])) <= f['max_position'] + 1e-12
            assert max(map(abs, f['velocity_rows'])) <= f['max_velocity'] + 1e-12
            assert f['linear_balance'] < 1e-6 and max(f['swing']) < .7
            for b in f['bodies']:
                assert len(b) == 8 and all(math.isfinite(x) for x in b)
                assert abs(sum(x*x for x in b[3:7])-1) < 1e-12
            for i, s in enumerate(f['supports']):
                assert len(s) == 13 and all(math.isfinite(x) for x in s)
                assert int(s[0]) == s[0] and 0 <= s[0] < 11
                assert np.linalg.norm(world(f['bodies'][int(s[0])], scene['locals'][i])-s[1:4]) < 1e-12
                assert abs(sum(x*x for x in s[7:10])-1) < 1e-12
                gap = sum((s[1+j]-s[4+j])*s[7+j] for j in range(3))
                assert abs(gap-s[11]) < 1e-12
                assert abs(gap-f['constraints'][i]) < 1e-12
                assert scene['domain'][0] < s[10] < scene['domain'][1]
                cell = scene['charts'][bisect.bisect_right(starts, s[10])-1]
                assert cell[0] <= s[10] <= cell[1]
                assert f['charts'][i] == cell[2] and f['sections'][i] == cell[3]
                assert abs(f['stationarity'][i]) < 2e-13 and f['denominators'][i] > 1e-8
                if previous:
                    rate, innovation, half = f['envelopes'][i]
                    ds = s[10]-previous['supports'][i][10]
                    assert abs(ds) <= innovation and half <= .20
                    assert abs(ds-rate/240) <= .25*abs(rate)/240+.002
            for i, (a, b) in enumerate(scene['edges']):
                anchors = f['anchors'][i]
                assert np.linalg.norm(world(f['bodies'][a], scene['bearing_locals'][i][0])-anchors[:3]) < 1e-12
                assert np.linalg.norm(world(f['bodies'][b], scene['bearing_locals'][i][1])-anchors[3:]) < 1e-12
            # Finite-state reaction work is measured, not incorrectly forced to zero.
            power = sum(a*b for a, b in zip(f['reactions'], f['velocity_rows']))
            assert abs(power-f['reaction_power']) < 1e-8
            previous = f
        values = {
            'ticks': 480, 'duration_s': 2.,
            'max_position': max(f['max_position'] for f in frames),
            'max_velocity': max(f['max_velocity'] for f in frames),
            'max_support_gap_m': max(abs(s[11]) for f in frames for s in f['supports']),
            'max_bearing_gap_m': max(f['bearing_gap'] for f in frames),
            'max_roll_error_rad': max(abs(v) for f in frames for v in f['roll']),
            'max_swing_rad': max(v for f in frames for v in f['swing']),
            'max_energy_drift_J': max(abs(f['energy_drift']) for f in frames),
            'final_energy_drift_J': frames[-1]['energy_drift'],
            'max_reaction_N': max(math.sqrt(sum(v*v for v in force)) for f in frames for force in f['reaction_forces']),
            'max_abs_power_W': max(abs(f['reaction_power']) for f in frames),
            'station_range_m': [min(s[10] for f in frames for s in f['supports']), max(s[10] for f in frames for s in f['supports'])],
            'charts': sorted(set(v for f in frames for v in f['charts'])),
            'minimum_D': min(v for f in frames for v in f['denominators']),
            'max_abs_F': max(abs(v) for f in frames for v in f['stationarity']),
            'max_linear_balance_N': max(f['linear_balance'] for f in frames),
        }
        summary[scene['fixture']] = values
    errors = np.zeros(3)
    for system in systems:
        j, v, gamma, loads, a = [np.array(system[k]) for k in ('j', 'v', 'gamma', 'loads', 'acceleration')]
        assert j.shape == (65, 66)
        singular = np.linalg.svd(j, compute_uv=False)
        for threshold in (1e-5, 1e-7, 1e-9):
            assert np.count_nonzero(singular > threshold) == 65
        inverse = np.tile([.01]*3+[.05]*3, 11)
        free = np.tile([0., -9.81, 0., 0., 0., 0.], 11)
        independent = np.linalg.solve((j*inverse)@j.T, -gamma-j@free)
        observed = [np.max(abs(independent-loads)), np.max(abs(j@a+gamma)), np.max(abs(a-free-inverse*(j.T@independent)))]
        assert max(observed) < 1e-7, (system['name'], observed)
        errors = np.maximum(errors, observed)
    summary['independent_matrix_max_errors'] = errors.tolist()
    Path(path).with_suffix('.summary.json').write_text(json.dumps(summary, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(summary, indent=2))
    print('PASS: 962 genuine states, full-tick gates/continuation, 12 independent long-run reaction systems')
    return clips, summary


if __name__ == '__main__':
    verify(*sys.argv[1:])
