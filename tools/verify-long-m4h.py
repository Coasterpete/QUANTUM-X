"""Independent accepted-state geometry/energy/reaction checks and exact replay.
python tools/verify-long-m4h.py LONG_LOG REPLAY_LOG
"""
import bisect
import json
import math
from pathlib import Path
import sys
import numpy as np


def require(condition, message):
    if not condition:
        raise ValueError(message)


def records(path):
    result = []
    for line in Path(path).read_text(encoding='utf-8-sig').splitlines():
        for tag in ('SCENE', 'FRAME', 'SYSTEM', 'STOP', 'DONE'):
            marker = f'M4H_LONG_{tag} '
            if marker in line:
                data = json.loads(line.split(marker, 1)[1], parse_constant=lambda s: (_ for _ in ()).throw(ValueError(s)))
                result.append((tag, data))
    return result


def rotation(q):
    w,x,y,z = q
    return np.array([[1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)],
                     [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)],
                     [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)]])


def world(body, local):
    return np.array(body[:3])+rotation(body[3:7]) @ np.array(local)


def verify(path, replay):
    evidence = records(path)
    require(evidence == records(replay), 'long accepted evidence must replay exactly')
    scenes, frames, systems, stops = {}, {}, [], {}
    for tag, data in evidence:
        name = data['fixture']
        if tag == 'SCENE':
            scenes[name] = data
        elif tag == 'FRAME':
            frames.setdefault(name, []).append(data)
        elif tag == 'SYSTEM':
            systems.append(data)
        elif tag == 'STOP':
            stops[name] = data
    require(set(scenes) == set(frames) == {'helix', 'foundation'}, 'missing fixture')
    summary = {}
    for name, clip in frames.items():
        scene = scenes[name]
        require([r['tick'] for r in clip] == list(range(1, len(clip)+1)) and len(clip) >= 480, 'incomplete accepted prefix')
        if len(clip) < 720:
            require(stops[name]['accepted'] == len(clip) and stops[name]['candidate_tick'] == len(clip)+1, 'missing rejected continuation')
        previous = None
        maxima = dict(gap_m=0., bearing_anchor_m=0., swing_rad=0., roll_rad=0., reaction_N=0., energy_closure_J=0., reconstruction_J=0.)
        station_min, station_max = math.inf, -math.inf
        charts = set()
        starts = [c[0] for c in scene['charts']]
        for d in clip:
            bodies, supports = d['bodies'], d['supports']
            require(len(bodies) == 11 and len(supports) == 30 and len(d['velocity']) == 66, 'topology')
            for p in bodies:
                require(abs(np.linalg.norm(p[3:7])-1) < 1e-12, 'nonunit quaternion')
            require(max(abs(x) for x in d['constraints']) < 2e-5 and max(abs(x) for x in d['velocity_rows']) < .003, 'mechanical gate')
            require(d['momentum'] < 1e-6, 'momentum closure')
            for i, s in enumerate(supports):
                owner = int(s[0])
                require(np.max(np.abs(world(bodies[owner], scene['locals'][i])-s[1:4])) < 1e-12, 'support body transform')
                delta = np.array(s[1:4])-s[4:7]
                require(abs(delta @ s[7:10]-s[11]) < 1e-13 and abs(np.linalg.norm(s[7:10])-1) < 1e-12, 'support normal/gap')
                require(abs(s[12]) < 2e-13 and s[13] > 1e-8 and 0 < s[10] < scene['length'], 'projection/domain gate')
                chart = int(scene['charts'][bisect.bisect_right(starts, s[10])-1][2])
                require(chart == int(s[14]), 'stale chart')
                charts.add(chart)
                maxima['gap_m'] = max(maxima['gap_m'], abs(s[11]))
                maxima['reaction_N'] = max(maxima['reaction_N'], s[15])
                station_min, station_max = min(station_min, s[10]), max(station_max, s[10])
                if previous:
                    rate, innovation, half = d['envelopes'][i]
                    movement = s[10]-previous['supports'][i][10]
                    require(abs(movement) <= innovation+1e-12 and abs(movement-rate/240) <= .25*abs(rate)/240+.002+1e-12 and half <= .2, 'station continuation')
            rolls = []
            for index, (edge, local, anchor) in enumerate(zip(scene['edges'], scene['bearing_locals'], d['anchors'])):
                a, b = [bodies[k] for k in edge]
                pa, pb = world(a, local[0]), world(b, local[1])
                require(np.max(np.abs(np.concatenate([pa,pb])-anchor)) < 1e-12, 'bearing anchor transform')
                gap = float(np.linalg.norm(pb-pa))
                require(gap < math.sqrt(3)*2e-5, 'bearing gap')
                qa, qb = np.array(a[3:7]), np.array(b[3:7])
                w = float(qa @ qb)
                v = qa[0]*qb[1:]-qb[0]*qa[1:]-np.cross(qa[1:], qb[1:])
                swing = 2*math.atan2(np.linalg.norm(v), abs(w))
                require(abs(swing-d['swing'][index]) < 1e-12 and swing < .7, 'bearing swing')
                if index in scene['roll_indices']:
                    rolls.append((2*math.atan2(v[0],w)+math.pi)%(2*math.pi)-math.pi)
                maxima['bearing_anchor_m'] = max(maxima['bearing_anchor_m'], gap)
                maxima['swing_rad'] = max(maxima['swing_rad'], swing)
            require(np.max(np.abs(np.array(rolls)-d['roll'])) < 1e-12 and max(abs(x) for x in rolls) < 2e-5, 'bearing roll')
            maxima['roll_rad'] = max(maxima['roll_rad'], max(abs(x) for x in rolls))
            v = np.array(d['velocity']).reshape(-1,6)
            energy = 50*float(np.sum(v[:,:3]**2))+10*float(np.sum(v[:,3:]**2))+981*sum(p[1] for p in bodies)
            error = abs(energy-scene['initial_energy']-d['drift'])
            require(error < 1e-8, 'state energy reconstruction')
            maxima['reconstruction_J'] = max(maxima['reconstruction_J'], error)
            wg, path_work, wc, *parts = d['ledger']
            closure = abs(d['drift']-wg+path_work-wc)
            require(closure < 1e-7 and abs(wc-sum(parts)) < 1e-7, 'extended energy ledger')
            maxima['energy_closure_J'] = max(maxima['energy_closure_J'], closure)
            previous = d
        require(any(s['fixture'] == name and s['tick'] == len(clip) for s in systems), 'missing final accepted independent system')
        summary[name] = dict(accepted_ticks=len(clip), seconds=len(clip)/240, stop=stops.get(name), final_drift_J=clip[-1]['drift'], maxima=maxima, stations=[station_min,station_max], charts=sorted(charts))
    for d in systems:
        j, v, gamma, loads, acceleration = [np.array(d[k]) for k in ('j','v','gamma','loads','acceleration')]
        inverse = np.tile([.01]*3+[.05]*3,11)
        free = np.tile([0.,-9.81,0.,0.,0.,0.],11)
        require(j.shape == (65,66) and min(np.linalg.svd(j,compute_uv=False)) > 1e-5, 'reaction rank')
        independent = np.linalg.solve((j*inverse) @ j.T, -gamma-j@free)
        require(np.max(np.abs(independent-loads)) < 1e-7 and np.max(np.abs(j@acceleration+gamma)) < 1e-7, 'reaction/acceleration solve')
        require(np.max(np.abs(acceleration-free-inverse*(j.T@independent))) < 1e-8, 'mass-weighted acceleration')
        frame = frames[d['fixture']][d['tick']-1]
        require(np.max(np.abs(j@v-frame['velocity_rows'])) < 1e-12, 'independent velocity rows')
        for i, s in enumerate(frame['supports']):
            owner = int(s[0])
            force = np.linalg.norm(j[i,owner*6:owner*6+3]*independent[i])
            require(abs(force-s[15]) < 1e-7, 'independent support reaction force')
    result = dict(fixtures=summary, states=sum(len(v) for v in frames.values()), independent_systems=len(systems), exact_replay=True)
    Path(path).with_suffix('.summary.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result,indent=2))
    print('PASS: bounded continuation, rollback evidence, exact replay and independent state/reaction checks')


if __name__ == '__main__':
    verify(*sys.argv[1:])
