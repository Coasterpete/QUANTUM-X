"""Qualify query derivatives, without providing a substitute rail implementation.

Usage: python tools/verify-graph-extension-m4e-c.py EXTENSION_LOG FIXTURE_LOG
Requires NumPy (as do M4B--M4E-B's verifiers). Decimal/Fraction are stdlib.
The high precision control is ONLY a closed, fixed-axis normalized-RK4 step.
It cannot integrate a graph, define general normals, or drive B1. It separates
restart defects from f64 stencil noise, after checking actual Rust query poses.
"""
import ast
from collections import defaultdict
from decimal import Decimal as D, localcontext
from fractions import Fraction
import json
import math
from pathlib import Path
import sys

import numpy as np


def norm(v):
    return sum(x*x for x in v).sqrt()


def cross(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]


def add(a, b):
    return [x+y for x, y in zip(a, b)]


def mul(v, a):
    return [a*x for x in v]


def sub(a, b):
    return add(a, mul(b, -1))


def dot(a, b):
    return sum(x*y for x, y in zip(a, b))


def normalized(v):
    return mul(v, 1/norm(v))


def rotation_product(a, b):
    return [a[0]*b[0]-a[1]*b[1], a[0]*b[1]+a[1]*b[0]]


def rolling_factor(h):
    """Closed two-component reduction of Context::step for constant .4 roll.

    Every intermediate RHS normalizes q; using the usual linear RK4 stability
    polynomial would differentiate a DIFFERENT method. No source rate jets enter.
    """
    def rhs(q):
        a, b = normalized(q)
        return [-D('.2')*b, D('.2')*a]
    q = [D(1), D(0)]
    a = rhs(q)
    b = rhs(add(q, mul(a, h/2)))
    c = rhs(add(q, mul(b, h/2)))
    d = rhs(add(q, mul(c, h)))
    return normalized(add(q, mul(add(add(a, mul(add(b, c), 2)), d), h/6)))


def rolling_pose(s, relative, anchor=(D(1), D(0))):
    a, b = rotation_product(anchor, rolling_factor(relative))
    co, si = a*a-b*b, 2*a*b
    z, o = D(0), D(1)
    return [s, z, z, o, z, z, z, co, si, z, -si, co]


def weights(nodes, derivative):
    # Exact Lagrange interpolation coefficients, no floating Vandermonde solve.
    result = []
    for i in nodes:
        p = [Fraction(1)]
        den = Fraction(1)
        for j in nodes:
            if i != j:
                v = [Fraction(0)]*(len(p)+1)
                for k, coefficient in enumerate(p):
                    v[k] -= j*coefficient
                    v[k+1] += coefficient
                p = v
                den *= i-j
        f = p[derivative]*math.factorial(derivative)/den
        result.append(D(f.numerator)/D(f.denominator))
    return result


def differentiate(function, center, delta, nodes):
    values = [function(center+D(i)*delta) for i in nodes]
    # Subtract a constant before summation; weights sum exactly to zero.
    base = function(center)
    return [[sum(w*(v[j]-base[j]) for w, v in zip(weights(nodes, d), values))/delta**d
             for j in range(len(base))] for d in (1, 2, 3)]


def angular(pose, derivatives):
    axes = [pose[i:i+3] for i in (3, 6, 9)]
    d = [[row[i:i+3] for i in (3, 6, 9)] for row in derivatives]
    k = [sum(cross(a, b)[j] for a, b in zip(axes, d[0]))/2 for j in range(3)]
    dk = [sum(cross(a, b)[j] for a, b in zip(axes, d[1]))/2 for j in range(3)]
    ddk = [sum(cross(b, c)[j]+cross(a, e)[j]
               for a, b, c, e in zip(axes, d[0], d[1], d[2]))/2 for j in range(3)]
    return [k, dk, ddk]


OFFSET = [D('.1'), D('-.08'), D('.6')]


def offset(pose):
    return [pose[j]+sum(OFFSET[i]*pose[3+3*i+j] for i in range(3)) for j in range(3)]


def source_derivatives(pose):
    k = [D('.4'), D(0), D(0)]
    result = []
    axes = [pose[i:i+3] for i in (3, 6, 9)]
    for d in (1, 2, 3):
        row = [D(1) if d == 1 else D(0), D(0), D(0)]
        for a in axes:
            for _ in range(d):
                a = cross(k, a)
            row += a
        result.append(row)
    return result


def datum_derivatives(rows):
    return [[row[j]+sum(OFFSET[i]*row[3+3*i+j] for i in range(3)) for j in range(3)] for row in rows]


def decimal_control(precision):
    result = []
    with localcontext() as ctx:
        ctx.prec = precision
        for h in (D('.1'), D('.05'), D('.025')):
            qend = rolling_factor(h)
            left = lambda t: rolling_pose(t, t)
            right = lambda t: rolling_pose(t, t-h, qend)
            p = left(h)
            source = source_derivatives(p)
            # Two one-sided spacings and two Decimal precisions independently
            # constrain differencing error; branch expressions never switch here.
            dl = differentiate(left, h, D('.00001'), list(range(-8, 1)))
            dl_fine = differentiate(left, h, D('.000005'), list(range(-8, 1)))
            dr = differentiate(right, h, D('.000005'), list(range(9)))
            assert max(norm(sub(a, b)) for a, b in zip(dl, dl_fine)) < D('1e-25')
            assert max(norm(sub(a, b)) for a, b in zip(source, dr)) < D('1e-25')
            k = angular(p, dl_fine)
            source_k = [[D('.4'), D(0), D(0)], [D(0)]*3, [D(0)]*3]
            pl, pr, ps = map(datum_derivatives, (dl_fine, dr, source))
            jump = [norm(sub(a, b)) for a, b in zip(pl, pr)]
            # At a true stationary target, e is perpendicular to actual P'.
            # Select the 2 mm displacement maximizing the erroneous source F.
            gap = sub(ps[0], pl[0])
            perpendicular = sub(gap, mul(pl[0], dot(gap, pl[0])/dot(pl[0], pl[0])))
            e = mul(normalized(perpendicular), D('.002'))
            assert abs(dot(e, pl[0])) < D('1e-60')
            false_f = abs(dot(e, ps[0]))
            acceleration = []
            for ds in (D('.00006'), D('.00003'), D('.000015'), D('.000001')):
                plus, minus, middle = map(offset, (right(h+ds), left(h-ds), p))
                second = mul(add(sub(plus, middle), sub(minus, middle)), 1/(ds*ds))
                # A datum traversal consistency diagnostic at ds=6*dt, not a
                # vehicle integrator, B1 residual, reaction, or dynamics result.
                acceleration.append([float(ds), float(norm(sub(second, ps[1]))*36)])
            result.append({
                'fine_cell_m': float(h),
                'frame_jet_defect': [float(max(norm(sub(a[i:i+3], b[i:i+3])) for i in (3, 6, 9)))
                                     for a, b in zip(dl_fine, source)],
                'angular_jet_defect': [float(norm(sub(a, b))) for a, b in zip(k, source_k)],
                'offset_derivative_jump': [float(x) for x in jump],
                'false_projection_F_at_2mm': float(false_f),
                'datum_acceleration_mismatch_at_6mps': acceleration,
            })
        assert result[0]['false_projection_F_at_2mm'] > 2e-13
        assert result[1]['offset_derivative_jump'][2] > 1e-7
        assert result[1]['datum_acceleration_mismatch_at_6mps'][0][1] > 2e-5
        for i in (0, 1):
            ratios = np.array(result[i]['offset_derivative_jump'])/result[i+1]['offset_derivative_jump']
            assert (ratios > [15, 7.5, 3.8]).all(), ratios
    return result


def read_evidence(paths):
    records = []
    refinements = defaultdict(list)
    cases = set()
    pending = None
    for path in paths:
        for line in Path(path).read_text(encoding='utf-8-sig').splitlines():
            if 'M4EC_' not in line:
                continue
            line = line[line.index('M4EC_'):]
            if line.startswith('M4EC_CASE '):
                fields = line.split(' ')
                cases.add((fields[1], float(fields[2])))
            elif line.startswith('M4EC_STENCIL '):
                assert pending is None
                _, name, step, label, center, width, mode, rows = line.split(' ', 7)
                pending = (name, float(step), label, float(center), float(width), int(mode), np.array(ast.literal_eval(rows)))
            elif line.startswith('M4EC_SOURCE '):
                assert pending is not None
                records.append((*pending, ast.literal_eval(line[len('M4EC_SOURCE '):])))
                pending = None
            elif line.startswith('M4EC_REFINEMENT '):
                _, name, step, s, position, rotation, angulars = line.split(' ', 6)
                refinements[name, float(step)].append([float(position), float(rotation), *ast.literal_eval(angulars)])
    assert pending is None
    assert cases == {(name, step) for name in ('straight', 'arc', 'helix', 'rolling', 'rider', 'force', 'foundation') for step in (.2, .1, .05)}, cases
    assert len(records) == 621, len(records)
    return records, refinements


def verify(paths):
    records, refinements = read_evidence(paths)
    worst = defaultdict(lambda: np.zeros(9))
    uncertainty = defaultdict(lambda: np.zeros(6))
    verified_rolling = 0
    analytic_pose_error = defaultdict(float)
    analytic_frame_error = defaultdict(float)
    analytic_source_jet_error = defaultdict(lambda: np.zeros(6))
    decimal_pose_error = 0.
    for name, step, label, s, width, mode, rows, source in records:
        base = np.array(source[0])
        expected = np.array(source[1]).reshape(3, 12)
        rhs_k = np.array(source[2]).reshape(3, 3)
        coordinates = rows[:, 0]/width
        # Fit the ACTUAL numerical queries, never the emitted source jets.
        a = np.polynomial.polynomial.polyvander(coordinates, 8)
        inverse = np.linalg.pinv(a)
        shifted = rows[:, 1:]-base
        derivatives = np.array([inverse[d]@shifted*math.factorial(d)/width**d for d in (1, 2, 3)])
        errors = np.linalg.norm((derivatives-expected).reshape(3, 4, 3), axis=2)
        axes = base[3:].reshape(3, 3)
        first, second, third = derivatives[:, 3:].reshape(3, 3, 3)
        k = np.cross(axes, first).sum(axis=0)/2
        dk = np.cross(axes, second).sum(axis=0)/2
        ddk = (np.cross(first, second)+np.cross(axes, third)).sum(axis=0)/2
        angular_errors = np.linalg.norm(np.array([k, dk, ddk])-rhs_k, axis=1)
        key = (name, step, label)
        observed = np.r_[errors[:, 0], errors[:, 1:].max(axis=1), angular_errors]
        worst[key] = np.maximum(worst[key], observed)
        # Conditioning estimate for f64 input perturbations. This is NOT an
        # interval error certificate or a relaxed B1 acceptance threshold.
        noise = np.array([np.abs(inverse[d]).sum()*math.factorial(d)/width**d
                          for d in (1, 2, 3)]) * np.finfo(float).eps * 16
        conditioning = np.r_[noise*max(1, np.abs(base[:3]).max()), noise]
        uncertainty[key] = np.maximum(uncertainty[key], conditioning)
        assert np.isfinite(observed).all()
        # No tolerance here converts these noisy measurements into qualification.
        # The known exact line is a conditioning control for the fitting itself.
        if name == 'straight':
            assert (observed[:6] < 8*conditioning).all(), (key, observed, conditioning)
        if name in ('straight', 'arc', 'helix', 'rolling'):
            co, si = math.cos(.6), math.sin(.6)
            bank = np.array([[1., 0., 0.], [0., co, -si], [0., si, co]])
            if name == 'rolling':
                bank = np.eye(3)
            local = {'arc': [0., -1/12, 0.], 'helix': [.04, 0., .125], 'rolling': [.4, 0., 0.], 'straight': [0., 0., 0.]}[name]
            omega = bank@np.array(local)
            w = np.linalg.norm(omega)
            if w == 0:
                analytic = np.array([s, 0., 0.])
                rotation = np.eye(3)
            else:
                axis = omega/w
                parallel = axis*np.dot(axis, [1., 0., 0.])
                perpendicular = np.array([1., 0., 0.])-parallel
                analytic = parallel*s+perpendicular*(math.sin(w*s)/w)+np.cross(axis, perpendicular)*((1-math.cos(w*s))/w)
                x, y, z = axis
                skew = np.array([[0., -z, y], [z, 0., -x], [-y, x, 0.]])
                rotation = np.eye(3)+math.sin(w*s)*skew+(1-math.cos(w*s))*(skew@skew)
            axes_exact = (rotation@bank).T
            frame_error = np.max(np.linalg.norm(base[3:].reshape(3, 3)-axes_exact, axis=1))
            analytic_frame_error[name, step] = max(analytic_frame_error[name, step], frame_error)
            analytic_pose_error[name, step] = max(analytic_pose_error[name, step], np.linalg.norm(base[:3]-analytic))
            t = axes_exact[0]
            spatial = np.array([t, np.cross(omega, t), np.cross(omega, np.cross(omega, t))])
            angular_exact = np.array([omega, np.zeros(3), np.zeros(3)])
            jet_error = np.r_[np.linalg.norm(expected[:, :3]-spatial, axis=1), np.linalg.norm(rhs_k-angular_exact, axis=1)]
            analytic_source_jet_error[name, step] = np.maximum(analytic_source_jet_error[name, step], jet_error)
            assert analytic_pose_error[name, step] < 2e-9
            assert frame_error < 1e-8 and (jet_error < 1e-8).all()
        if name == 'rolling':
            # Check the high-precision reduced expression against actual queries,
            # including many-cell checkpoints, before trusting its derivatives.
            with localcontext() as ctx:
                ctx.prec = 60
                h = D('12')/D(math.ceil(12/(step*.5)))
                for row in rows:
                    x = D(str(s))+D(str(row[0]))
                    cell = int(x//h)
                    anchor = [D(1), D(0)]
                    factor = rolling_factor(h)
                    exponent = cell
                    while exponent:
                        if exponent % 2:
                            anchor = rotation_product(anchor, factor)
                        factor = rotation_product(factor, factor)
                        exponent //= 2
                    exact = np.array(list(map(float, rolling_pose(x, x-D(cell)*h, anchor))))
                    error = np.max(np.abs(exact-row[1:]))
                    assert error < 2e-13
                    decimal_pose_error = max(decimal_pose_error, error)
                verified_rolling += 1
    high = decimal_control(90)
    control = decimal_control(70)
    assert np.allclose(np.array([a['offset_derivative_jump'] for a in high]),
                       np.array([a['offset_derivative_jump'] for a in control]), rtol=1e-20, atol=1e-35)
    summary = {
        'stencils': len(records), 'rolling_stencils_checked_against_decimal': verified_rolling,
        'columns': ['C1', 'C2', 'C3', 'R1_max_axis', 'R2_max_axis', 'R3_max_axis', 'k', 'k1', 'k2'],
        'observed': {'/'.join(map(str, k)): v.tolist() for k, v in sorted(worst.items())},
        'f64_conditioning_estimate': {'/'.join(map(str, k)): v.tolist() for k, v in sorted(uncertainty.items())},
        'raw_refinement_maxima': {'/'.join(map(str, k)): np.max(v, axis=0).tolist() for k, v in sorted(refinements.items())},
        'independent_position_errors': {'/'.join(map(str, k)): v for k, v in sorted(analytic_pose_error.items())},
        'independent_frame_axis_errors': {'/'.join(map(str, k)): v for k, v in sorted(analytic_frame_error.items())},
        'independent_source_spatial_and_angular_jet_errors': {'/'.join(map(str, k)): v.tolist() for k, v in sorted(analytic_source_jet_error.items())},
        'max_rust_vs_decimal_pose_component_error': decimal_pose_error,
        'decimal_restart_control': high,
    }
    destination = Path(paths[0]).parent/'independent-summary.json'
    destination.write_text(json.dumps(summary, indent=2)+'\n', encoding='utf-8')
    print(f'PASS evidence completeness: {len(records)} actual-query stencils; {verified_rolling} rolling stencils agree with independent Decimal poses')
    print('PASS 70/90-digit, two-spacing one-sided derivative controls; resolved restart defects retained')
    for case in high:
        print(json.dumps(case))
    print(f'Numerical qualification: FAIL differential restart contract; no adapter/B1 qualification granted. Evidence: {destination}')


if __name__ == '__main__':
    verify(sys.argv[1:])
