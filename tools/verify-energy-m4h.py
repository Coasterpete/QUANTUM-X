"""Independent SI state/impulse energy reconstruction; no Rust energy helper.
python tools/verify-energy-m4h.py target/m4h/stage-a-energy.log
"""
import json
import math
from pathlib import Path
import sys
import numpy as np


def require(value, message):
    if not value:
        raise ValueError(message)


def check_sample(d, rows=None):
    n = d['n']
    h = 1 / (240 * n)
    v0, v1, p = [np.array(d[k]) for k in ('v0', 'v1', 'impulse')]
    p0, p1 = [np.array(d[k]) for k in ('p0', 'p1')]
    lam, c, gamma, speed = [np.array(d[k]) for k in ('lambda', 'c', 'gamma', 'speed')]
    require(all(np.all(np.isfinite(x)) for x in (v0, v1, p, p0, p1, lam, c, gamma, speed)), 'nonfinite sample')
    mass = np.tile([100.] * 3 + [20.] * 3, len(v0) // 6)
    g = np.tile([0., -9.81, 0., 0., 0., 0.], len(v0) // 6)
    u = v0 + h * g
    require(np.max(np.abs(v1 - u - p / mass)) < 2e-12, 'momentum/impulse mismatch')
    require(np.max(np.abs(p1 - p0 - v1.reshape(-1, 6)[:, :3] * h)) < 1e-13, 'pose advancement mismatch')
    wg = float(np.sum(mass * (u*u - v0*v0)) / 2)
    path = float(-981 * np.sum((p1 - p0)[:, 1]))
    wc = float(np.sum(p * (u + v1)) / 2)
    curve = float(-h * lam @ gamma)
    stab = float(-.2 / h * lam @ c)
    residual = float(lam @ (speed + h * gamma + .2 / h * c))
    quadratic = float(-np.sum(p*p / mass) / 2)
    parts = np.array([wg, path, wc, curve, stab, residual, quadratic])
    require(np.max(np.abs(parts - d['parts'])) < 2e-9, 'work reconstruction mismatch')
    require(abs(wc - curve - stab - residual - quadratic) < 2e-9, 'constraint work decomposition mismatch')
    delta = float(np.sum(mass * (v1*v1-v0*v0))/2 - path)
    require(abs(delta - wg + path - wc) < 2e-9, 'discrete energy closure mismatch')
    result = {'closure': abs(delta-wg+path-wc)}
    if rows is not None:
        j = np.zeros((len(lam), len(v0)))
        for i, row in enumerate(rows):
            for k, value in row:
                j[i, int(k)] = value
        require(np.max(np.abs(j.T @ lam - p)) < 2e-10, 'independent generalized impulse mismatch')
        require(np.max(np.abs(j @ v1 - speed)) < 2e-12, 'independent Jv mismatch')

        # Independent replay of finite PGS at one fixed state. The beta=0
        # counterfactual changes only this local target, not an entire trajectory.
        def impulses(beta):
            v = u.copy()
            loads = np.zeros(len(lam))
            diagonal = np.sum(j*j / mass, axis=1)
            for _ in range(32):
                for i, row in enumerate(j):
                    dl = -(row @ v + h*gamma[i] + beta*c[i]/h) / diagonal[i]
                    loads[i] += dl
                    v += row * dl / mass
            return loads, v

        replay, end = impulses(.2)
        require(np.max(np.abs(end-v1)) < 1e-10 and np.max(np.abs(replay-lam)) < 1e-8, 'finite impulse replay mismatch')
        _, no_stab = impulses(0.)
        result['fixed_state_beta_work_difference'] = float(np.sum(mass*(v1*v1-no_stab*no_stab))/2)
        free = g
        force = np.linalg.solve((j / mass) @ j.T, -gamma - j @ free)
        require(np.max(np.abs(j @ (free + (j.T @ force)/mass) + gamma)) < 1e-7, 'independent reaction equation')
        result['instantaneous_power_before'] = float(force @ (j @ v0))
    return result


def verify(path):
    samples, jacobians, series = {}, {}, {}
    raw_count = 0
    max_closure = 0.
    for line in Path(path).read_text(encoding='utf-8-sig').splitlines():
        for tag in ('M4H_SAMPLE ', 'M4H_J ', 'M4H_ENERGY '):
            if tag not in line:
                continue
            d = json.loads(line.split(tag, 1)[1])
            key = (d['fixture'], d['n'])
            if tag == 'M4H_SAMPLE ':
                result = check_sample(d)
                max_closure = max(max_closure, result['closure'])
                raw_count += 1
                if d['sub'] == 0:
                    samples[key + (d['tick'],)] = d
                if d['n'] == 4 and d['sub'] == 3:
                    series.setdefault(key, {}).setdefault('raw_final', {})[d['tick']] = d
            elif tag == 'M4H_J ':
                jacobians[key + (d['tick'],)] = d['rows']
            else:
                series.setdefault(key, {}).setdefault('energy', []).append(d)
    require(set(series) == {(f, n) for f in ('helix', 'foundation') for n in (4, 16, 64)}, 'missing fixture/refinement')
    summary = {'raw_substeps_checked': raw_count, 'maximum_independent_closure_J': max_closure, 'runs': {}, 'fixed_state_checks': []}
    for key, data in series.items():
        frames = data['energy']
        require([d['tick'] for d in frames] == list(range(1, 481)), 'incomplete duration')
        first = samples[key + (1,)]
        v0 = np.array(first['v0']).reshape(-1, 6)
        initial = float(50*np.sum(v0[:, :3]**2)+10*np.sum(v0[:, 3:]**2)+981*np.sum(np.array(first['p0'])[:, 1]))
        for d in frames:
            require(all(math.isfinite(x) for x in d['ledger']+d['energy']+[d['power'], d['drift']]), 'nonfinite frame')
            wg, work_path, wc, curve, stab, residual, quad = d['ledger']
            require(abs(d['drift']-wg+work_path-wc) < 1e-7, 'accumulated ledger mismatch')
            require(abs(wc-curve-stab-residual-quad) < 1e-7, 'accumulated impulse mismatch')
            require(abs(sum(d['energy']) - initial - d['drift']) < 1e-7, 'initial energy mismatch')
            require(d['p'] < 2e-5 and d['v'] < .003, 'mechanical gate')
            if key[1] == 4:
                raw = data['raw_final'][d['tick']]
                v = np.array(raw['v1']).reshape(-1, 6)
                energy = [50*float(np.sum(v[:, :3]**2)), 10*float(np.sum(v[:, 3:]**2)), 981*float(np.sum(np.array(raw['p1'])[:, 1]))]
                require(max(abs(a-b) for a,b in zip(energy, d['energy'])) < 1e-8, 'independent state energy')
        final = dict(frames[-1])
        final['gravity_discretization_J'] = final['ledger'][0]-final['ledger'][1]
        turn = max(frames, key=lambda d: d['station'])
        final['largest_front_station_tick'] = turn['tick']
        final['at_turnaround'] = frames[turn['tick']-1]
        final['pre_turn_drift_J'] = frames[turn['tick']-1]['drift']
        final['post_turn_drift_J'] = final['drift']-frames[turn['tick']-1]['drift']
        final['drift_samples_J'] = {str(t): frames[t-1]['drift'] for t in (120,240,258,259,360,480)}
        summary['runs'][f'{key[0]}-{key[1]}'] = final
    require(raw_count == 2*(480*4+7+7) and len(jacobians) == 42, 'missing raw/checkpoint evidence')
    for key, rows in jacobians.items():
        d = samples[key]
        result = check_sample(d, rows)
        summary['fixed_state_checks'].append({'fixture': key[0], 'n': key[1], 'tick': key[2], **result})
    # Negative verification: corrupt impulses, accounting, state and nonfinite input.
    for field in ('impulse', 'parts', 'v1'):
        d = json.loads(json.dumps(next(iter(samples.values()))))
        d[field][0] += 1.
        try:
            check_sample(d)
        except ValueError:
            pass
        else:
            raise ValueError(f'corrupt {field} accepted')
    d['v0'][0] = float('nan')
    try:
        check_sample(d)
    except ValueError:
        pass
    else:
        raise ValueError('NaN accepted')
    out = Path(path).with_suffix('.summary.json')
    out.write_text(json.dumps(summary, indent=2)+'\n', encoding='utf-8')
    print(json.dumps({k:v for k,v in summary.items() if k != 'fixed_state_checks'}, indent=2))
    print('PASS: independent discrete energy, impulse, stabilization and reaction reconstruction')


if __name__ == '__main__':
    verify(sys.argv[1])
