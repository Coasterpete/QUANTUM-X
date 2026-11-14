"""Independent bounded R20 mass-metric references from raw production states.

No Rust matrices/Jacobians/impulses are consumed. Circle geometry is analytic;
twist derivatives are centered differences of quaternion scalar gaps. NumPy SVD
solves the whitened least-norm problem, including the affine lead speed row.
This checks initialization, not subsequent articulated centripetal reactions.
"""
import json
import re
import sys
import numpy as np


def multiply(a, b):
    w, x, y, z = a
    s, u, v, t = b
    return np.array([w*s-x*u-y*v-z*t, w*u+x*s+y*t-z*v,
                     w*v-x*t+y*s+z*u, w*t+x*v-y*u+z*s])


def conjugate(q):
    return q * [1, -1, -1, -1]


def rotation(q):
    return np.column_stack([multiply(multiply(q, [0, *a]), conjugate(q))[1:]
                            for a in np.eye(3)])


def axis_angle(axis, angle):
    return np.array([np.cos(angle/2), *(axis*np.sin(angle/2))])


def twist(a, b):
    q = multiply(conjugate(a), b)
    return 2*np.arctan2(q[1], q[0])


def derivative(function, p, q, width):
    result = []
    for axis in np.eye(3):
        result.append((function(p+width*axis, q)-function(p-width*axis, q))/(2*width))
    for axis in np.eye(3):
        result.append((function(p, multiply(axis_angle(axis, width), q))
                       - function(p, multiply(axis_angle(axis, -width), q)))/(2*width))
    return np.array(result)


def reference(case, width):
    raw = np.array(case['seed'])
    size = 6*len(raw)
    rows = []
    targets = []
    def row(parts, target=0):
        r = np.zeros(size)
        for body, v in parts:
            r[6*body:6*body+6] += v
        rows.append(r)
        targets.append(target)
    def point_row(body, local, normal):
        arm = rotation(raw[body, 3:7]) @ local
        return np.r_[normal, np.cross(arm, normal)]
    for entry in case['joints']:
        a, b = map(int, entry[:2])
        for axis in np.eye(3):
            row([(a, -point_row(a, entry[2:5], axis)), (b, point_row(b, entry[5:8], axis))])
    for a, b in case['angular']:
        qa, qb = raw[a, 3:7], raw[b, 3:7]
        grad = derivative(lambda _, q: twist(qa, q), raw[b, :3], qb, width)[3:]
        row([(a, np.r_[np.zeros(3), -grad]), (b, np.r_[np.zeros(3), grad])])
    local_guides = {}
    for i, entry in enumerate(case['guides']):
        body = int(entry[0])
        local, station = np.array(entry[1:4]), entry[4]
        local_guides.setdefault(body, []).append(local)
        theta = station/20
        tangent = np.array([np.cos(theta), 0, np.sin(theta)])
        inward = np.array([-np.sin(theta), 0, np.cos(theta)])
        bank = case['bank']
        normal = np.array([0, np.cos(bank), 0])+inward*np.sin(bank)
        lateral = inward*np.cos(bank)-np.array([0, np.sin(bank), 0])
        for axis in [normal, lateral]:
            row([(body, point_row(body, local, axis))])
        if i == case['reference']:
            speed_row = (body, point_row(body, local, tangent))
    for body, local in local_guides.items():
        local = np.mean(local, axis=0)
        def gap(p, q):
            point = p+rotation(q)@local
            theta = np.arctan2(point[0], 20-point[2])
            frame = multiply(axis_angle(np.array([0, 1, 0]), -theta),
                             axis_angle(np.array([1, 0, 0]), case['bank']))
            return twist(frame, q)
        row([(body, derivative(gap, raw[body, :3], raw[body, 3:7], width))])
    row([speed_row], case['target'])
    jacobian = np.array(rows)
    inv_sqrt = np.zeros((size, size))
    for body, values in enumerate(raw):
        at = 6*body
        inv_sqrt[at:at+3, at:at+3] = np.eye(3)/np.sqrt(values[13])
        rot = rotation(values[3:7])
        inv_sqrt[at+3:at+6, at+3:at+6] = rot @ np.diag(1/np.sqrt(values[14:17])) @ rot.T
    seed = raw[:, 7:13].flatten()
    a = jacobian @ inv_sqrt
    correction, _, rank, singular = np.linalg.lstsq(a, np.array(targets)-jacobian@seed, rcond=1e-7)
    answer = seed + inv_sqrt @ correction
    actual = np.array(case['final'])[:, 7:13].flatten()
    residual = np.max(np.abs(jacobian@answer-targets))
    error = np.max(np.abs(answer-actual))
    metric = float(correction@correction/2)
    assert residual < 2e-7, (case['name'], 'reference feasibility', residual)
    assert error < 2e-6, (case['name'], 'solution discrepancy', error, singular)
    assert abs(metric-case['metric']) < 2e-5, (case['name'], 'mass metric', metric, case['metric'])
    assert np.array_equal(raw[:, :7], np.array(case['final'])[:, :7]), 'pose changed'
    return answer, rank, residual, error, abs(metric-case['metric'])


def main():
    cases = []
    for line in open(sys.argv[1], encoding='utf-8-sig'):
        if 'M4J_REFERENCE ' in line:
            cases.append(json.loads(line.split('M4J_REFERENCE ', 1)[1]))
    assert len(cases) == 4, f'expected all four references, found {len(cases)}'
    for case in cases:
        coarse = reference(case, 2e-6)
        fine = reference(case, 1e-6)
        refinement = float(np.max(np.abs(coarse[0]-fine[0])))
        assert refinement < 1e-6, (case['name'], refinement)
        print(json.dumps(dict(name=case['name'], rank=int(fine[1]), residual=fine[2],
                              max_velocity_error=fine[3], metric_error_j=fine[4],
                              stencil_difference=refinement)))
    print('PASS: four independent analytic-geometry/SVD velocity references')
    if len(sys.argv) == 4:
        controls = []
        for path in sys.argv[2:4]:
            value = re.findall(r'M4J_LEGACY_FINGERPRINT ([0-9a-f]{16})', open(path, encoding='utf-8-sig').read())
            assert len(value) == 1, (path, value)
            controls.append(value[0])
        assert controls[0] == controls[1], controls
        print('PASS: untouched M4I / M4J legacy fingerprints match:', controls[0])


if __name__ == '__main__':
    main()
