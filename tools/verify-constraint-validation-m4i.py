"""Independent M4I raw-state references and local NumPy solves.

python tools/verify-constraint-validation-m4i.py target/m4i/long.log
References use authored constants and raw states, never solver projections,
Jacobians or impulses. Local linear-solve checks intentionally consume J, and
are a separate validation category. NumPy is an offline verification dependency.
"""
import copy
import json
import math
from pathlib import Path
import sys
import numpy as np


def require(condition, message):
    if not condition:
        raise ValueError(message)


def finite(value):
    if isinstance(value, dict):
        return all(finite(x) for x in value.values())
    if isinstance(value, list):
        return all(finite(x) for x in value)
    return not isinstance(value, (int, float)) or math.isfinite(value)


def records(path):
    result = {}
    for line in Path(path).read_text(encoding='utf-8-sig').splitlines():
        if 'M4I_' not in line or ' {' not in line:
            continue
        prefix, body = line.split(' {', 1)
        tag = prefix.split('M4I_', 1)[1]
        data = json.loads('{' + body)
        require(finite(data), f'nonfinite {tag}')
        result.setdefault(tag, []).append(data)
    return result


def reference_metrics(rows):
    first = rows[0]
    case, mode, engine = (first[k] for k in ('case', 'mode', 'engine'))
    require([r['tick'] for r in rows] == list(range(1441)), 'incomplete six-second reference')
    x, v, w = (np.array([r[k] for r in rows]) for k in ('x', 'v', 'w'))
    time = np.arange(len(rows)) / 240
    speed = np.linalg.norm(v, axis=1)
    energy = 50*np.sum(v*v, axis=1)+10*np.sum(w*w, axis=1)+981*x[:, 1]
    if case == 'incline':
        tangent = np.array([math.cos(.1), -math.sin(.1), 0.])
        station = x @ tangent
        path_error = float(np.max(np.linalg.norm(x-station[:, None]*tangent, axis=1)))
        expected_speed = np.sqrt(25+2*9.81*math.sin(.1)*(station-5))
        expected_station = 5+5*time+.5*9.81*math.sin(.1)*time*time
        force = np.tile(100*(9.81*math.sin(.1)*tangent+np.array([0., 9.81, 0.])), (1440, 1))
        if engine == 'production':
            discrete_drift = -.5*100*(9.81*math.sin(.1))**2*(1/(240*first['n']))*6
            require(abs(energy[-1]-energy[0]-discrete_drift) < 1e-6, 'incline discrete energy reference')
    else:
        angle = np.unwrap(np.arctan2(x[:, 0], 20-x[:, 2]))
        station = 20*angle
        radial = np.hypot(x[:, 0], x[:, 2]-20)
        # B1's finite five-support geometry seats at a different COM radius.
        # Measure that offset explicitly. Its conditional orbit prediction uses
        # this initial raw radius, not a solver projection or derivative.
        radius = radial[0] if engine == 'b1' else math.sqrt(400-(.3 if mode.startswith('carriage') else 0)**2)
        path_error = float(max(np.max(abs(radial-radius)), np.max(abs(x[:, 1]-x[0, 1]))))
        expected_speed = np.full(len(rows), 15.)
        expected_station = 5+20*15*time/radius
        midpoint = (x[1:]+x[:-1])/2 - np.array([0., 0., 20.])
        inward = -midpoint
        inward[:, 1] = 0
        inward /= np.linalg.norm(inward, axis=1)[:, None]
        force = 100*(225/radius*inward+np.array([0., 9.81, 0.]))
        # A COM-only guide constrains no attitude; its free body does not yaw
        # with the curve. Carriage and five-support B1 models do.
        omega_ref = 0. if mode == 'point' else -15/radius
        if mode == 'carriage-zero-omega':
            omega_ref *= 100*radius*radius/(100*radius*radius+20)
        require(abs(w[-1, 1]-omega_ref) < 5e-5, 'circle angular-speed reference')
        require(path_error < 3.2e-5, 'circle radial/height error')
    actual_force = 100*np.diff(v, axis=0)*240+np.array([0., 981., 0.])
    relative = np.linalg.norm(actual_force-force, axis=1)/np.linalg.norm(force, axis=1)
    max_speed = float(np.max(abs(speed-expected_speed)))
    require(max_speed < (.008 if mode == 'carriage-zero-omega' else .001), 'analytical speed comparison')
    require(float(np.max(relative[24:])) < .002, 'analytical reaction comparison')
    result = dict(speed_error=max_speed, path_error_m=path_error,
                  time_station_error_m=float(station[-1]-expected_station[-1]),
                  energy_drift_J=float(energy[-1]-energy[0]),
                  initial_step_energy_J=float(energy[1]-energy[0]),
                  reaction_initial_error_N=float(np.linalg.norm(actual_force[0]-force[0])),
                  reaction_steady_relative=float(np.max(relative[24:])))
    if case != 'incline':
        a = station[-1]/20
        inward = np.array([-math.sin(a), 0., math.cos(a)])
        tangent = np.array([math.cos(a), 0., math.sin(a)])
        bank = .6 if case == 'banked' else 0.
        normal = np.array([0., math.cos(bank), 0.])+math.sin(bank)*inward
        lateral = np.cross(tangent, normal)
        result['normal_lateral_N'] = [float(actual_force[-1] @ normal), float(actual_force[-1] @ lateral)]
        result['reference_normal_lateral_N'] = [float(force[-1] @ normal), float(force[-1] @ lateral)]
        result['initial_COM_radius_minus_R_m'] = float(radial[0]-20)
    return result


def check_class(d):
    h = d['h']
    c0, c1, gamma, frozen, r, stab, sweep, curve, ec, er = (
        np.array(d[k]) for k in ('c0', 'c1', 'gamma', 'frozen', 'r', 'stabilization',
                                'sweep', 'curvature', 'unexplained_c', 'unexplained_r'))
    require(np.max(abs(stab+.2*c0/h)) < 1e-14, 'stabilization decomposition')
    require(np.max(abs(curve-h*(frozen-gamma))) < 1e-14, 'curvature decomposition')
    require(np.max(abs(r-stab-sweep-curve-er)) < 1e-14, 'velocity closure')
    predicted = .8*c0+h*sweep-h*h*gamma+.5*h*h*frozen
    require(np.max(abs(c1-predicted-ec)) < 1e-14, 'gap closure')
    for key in ('gap_second', 'gap_second_refined'):
        require(np.max(abs(np.array(d[key])-frozen)) < 2e-5, 'position-only independent curvature')
    return dict(max_unexplained_c=float(np.max(abs(ec))), max_unexplained_r=float(np.max(abs(er))))


def check_local(d, results):
    j, inverse, u, target, exact = (np.array(d[k]) for k in ('j', 'inv', 'u', 'target', 'exact'))
    a = (j*inverse) @ j.T
    independent = np.linalg.solve(a, target-j@u)
    require(np.linalg.matrix_rank(j, tol=1e-9) == len(j), 'local full row rank')
    require(np.max(abs(independent-exact)) < 1e-8, 'independent local solution')
    require(np.max(abs(a@exact-(target-j@u))) < 1e-9, 'direct residual')
    require([r['sweeps'] for r in results] == [32, 128, 512, 2048], 'missing sweep result')
    errors = []
    for result, emitted in zip(results, d['errors']):
        lam, v = np.array(result['lambda']), np.array(result['v'])
        require(np.max(abs(u+inverse*(j.T@lam)-v)) < 2e-11, 'local impulse application')
        error = j@v-target
        relative = np.linalg.norm(lam-independent)/np.linalg.norm(independent)
        require(abs(np.max(abs(error))-emitted[1]) < 1e-11, 'local row error')
        require(abs(relative-emitted[3]) < 1e-8, 'local impulse error')
        if result['sweeps'] == 32:
            require(np.max(abs(lam-d['original_impulse'])) < 1e-8, 'original impulse replay')
        errors.append(dict(sweeps=result['sweeps'], max_row_error=float(np.max(abs(error))),
                           bearing_error=float(np.max(abs(error[d['rail_rows']:]))),
                           relative_impulse_error=float(relative),
                           max_multiplier_rate_error=float(np.max(abs(lam-independent))/d['h']),
                           rail_force_error_N=float(np.max(abs(lam[:d['rail_rows']]-independent[:d['rail_rows']])
                               *np.linalg.norm(j[:d['rail_rows']].reshape(d['rail_rows'], -1, 6)[:, :, :3], axis=(1, 2)))/d['h'])))
    require(errors[-1]['max_row_error'] < 1e-10, 'converged sweep residual')
    return dict(label=d['label'], sub=d['sub'], condition=float(np.linalg.cond(a)), errors=errors)


def check_regular(d):
    for station, gap, f, denom, margin, chart, dp, dq in d['supports']:
        require(abs(f) < 2e-13 and denom > 1e-8 and station > 0 and margin > 0, 'projection regularity')
        require(chart in (0, 1) and dp < 1e-8 and dq < 1e-8, 'chart/source refinement')
    for _, _, gap, swing, roll in d['bearings']:
        require(gap < 2e-5 and swing < .7 and abs(roll) < 2e-5, 'bearing regularity')


def articulated_metrics(rows):
    require([x['tick'] for x in rows] == list(range(961)), 'incomplete articulated control')
    energies = []
    reconstruction = 0.
    for row in rows:
        total = 0.
        for b in row['bodies']:
            x = np.array(b[:3]); q = np.array(b[3:7]); v = np.array(b[7:10]); w = np.array(b[10:13])
            mass = b[13]; inertia = np.array(b[14:17]); a, i, j, k = q
            require(abs(q@q-1) < 1e-12, 'articulated unit quaternion')
            rotation = np.array([[1-2*(j*j+k*k), 2*(i*j-k*a), 2*(i*k+j*a)],
                                 [2*(i*j+k*a), 1-2*(i*i+k*k), 2*(j*k-i*a)],
                                 [2*(i*k-j*a), 2*(j*k+i*a), 1-2*(i*i+j*j)]])
            local = rotation.T @ w
            total += .5*mass*(v@v)+.5*float(inertia@(local*local))+mass*9.81*x[1]
        reconstruction = max(reconstruction, abs(total-row['energy']))
        require(reconstruction < 1e-7, 'articulated energy reconstruction')
        energies.append(total)
    return dict(bodies=len(rows[0]['bodies']), drift_J=energies[-1]-energies[0],
                first_step_J=energies[1]-energies[0], reconstruction_J=reconstruction,
                outside_original_quality_gate=sum(r['outside_quality_gate'] for r in rows))


def verify(path):
    d = records(path)
    require(len(d.get('FINAL', [])) == 6 and len(d.get('SWEEP', [])) == 8, 'missing trajectory experiments')
    references = {}
    for row in d.get('REFERENCE', []):
        key = (row['case'], row['engine'], row['mode'], row['n'])
        references.setdefault(key, []).append(row)
    require(len(references) == 24, 'missing analytical reference runs')
    summary = {'references': {'/'.join(map(str, k)): reference_metrics(v) for k, v in sorted(references.items())}}
    articulated = {}
    for row in d.get('ARTICULATED', []):
        articulated.setdefault((row['case'], row['engine']), []).append(row)
    require(len(articulated) == 6, 'missing articulated controls')
    summary['articulated'] = {'/'.join(k): articulated_metrics(v) for k, v in sorted(articulated.items())}
    summary['production_starts'] = d['PRODUCTION_START']
    require({x['case'] for x in d['PRODUCTION_START']} == {'incline', 'circle', 'banked'}, 'missing production start')
    for start in d['PRODUCTION_START']:
        initial = articulated[start['case'], 'production'][0]
        require(all(b[10:13] == [0., 0., 0.] for b in initial['bodies']), 'M4H zero-omega seed control changed')
        if start['case'] == 'incline':
            require(start['anchor_speed'] < 1e-12 and start['guide_speed'] < 1e-12, 'straight production start')
        else:
            # Deliberate baseline characterization of the still-unfixed issue.
            # A separately authorized fix must deliberately revise this control.
            require(start['anchor_speed'] > 1. and start['guide_speed'] > .29, 'curved production initialization control changed')
    summary['classification_checks'] = [check_class(x) for x in d['CLASS']]
    summary['local_systems'] = [check_local(x, [v for v in d['LOCAL_RESULT'] if (v['label'], v['sub']) == (x['label'], x['sub'])]) for x in d['LOCAL']]
    for row in d['REGULAR']:
        check_regular(row)
    for f in d['FINAL']:
        require(f['handoff'] < 1e-12, 'accepted-state continuity')
        require(f['accepted'] == (f['fixture'] != 'helix' or f['n'] != 4), 'original gate outcome')
        require(f['max_p'] < 2e-5 and (f['max_v'] < .003 or (f['fixture'], f['n']) == ('helix', 4)), 'original absolute gate')
    candidate = [x for x in d['CLASS'] if x['label'] == 'helix/4/563' and x['sub'] == 3]
    require(len(candidate) == 1, 'missing exact failing internal step')
    row = candidate[0]
    require(int(np.argmax(abs(np.array(row['r'])))) == 25, 'changed gate row')
    require(abs(row['r'][25]+.003003486171) < 1e-12, 'tick-563 value')
    summary['tick563_row25'] = {k: row[k][25] for k in ('c0', 'c1', 'r', 'gamma', 'stabilization', 'sweep', 'curvature', 'unexplained_c', 'unexplained_r')}
    for item in d['DISTURBANCE']:
        require(item['tick'] == 301 and item['gap_jump'] > 9.9e-6, 'disturbance not detected')
    require(len(d['DISTURBANCE']) == 1, 'missing disturbance')
    for x in d['REFINEMENT']:
        require(x['fine_difference'] < x['coarse_difference'], 'whole trajectory refinement')
        if x['fixture'] == 'helix':
            require(3.5 < x['coarse_difference']/x['fine_difference'] < 4.5, 'R8 first-order refinement')
    summary['trajectories'] = d['FINAL']
    summary['refinement'] = d['REFINEMENT']
    summary['sweeps'] = []
    for x in d['SWEEP']:
        samples = x.pop('times_ms')
        x['timing_ms'] = dict(zip(('median', 'p95', 'p99', 'max'), map(float, np.percentile(samples, [50, 95, 99, 100]))))
        require(x['max_p'] < 2e-5 and x['max_v'] < .003, 'experiment accepted prefix gate')
        require(x['accepted'] >= 480, 'insufficient sweep trajectory')
        summary['sweeps'].append(x)
    # Negative controls exercise the verifier, not production acceptance policy.
    bad_c = copy.deepcopy(row); bad_c['c1'][25] += 1e-5
    bad_r = copy.deepcopy(row); bad_r['sweep'][25] += .001
    bad_reg = copy.deepcopy(d['REGULAR'][0]); bad_reg['supports'][0][2] = 1e-6
    for function, bad in ((check_class, bad_c), (check_class, bad_r), (check_regular, bad_reg)):
        try:
            function(bad)
        except ValueError:
            continue
        raise ValueError('corrupt evidence accepted')
    summary['corrupt_evidence_rejected'] = 3
    destination = Path(path).with_suffix('.summary.json')
    destination.write_text(json.dumps(summary, indent=2, allow_nan=False)+'\n')
    print(f'PASS: {len(references)} analytical runs, {len(d["CLASS"])} decompositions, '
          f'{len(d["LOCAL"])} independently solved local systems; {destination}')


if __name__ == '__main__':
    verify(sys.argv[1])
