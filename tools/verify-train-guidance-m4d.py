"""Independent position-only derivative checks and LAPACK reaction verification.

Usage: python tools/verify-train-guidance-m4d.py target/m4d/release.log
NumPy is optional developer tooling; no application dependency is introduced.
The reference below has no Rust jets, quaternion interpolator or Newton solver.
"""
import json
import sys
from pathlib import Path

import numpy as np
from numpy.polynomial.polynomial import polyfit


def basis(name, s):
    if name in ("Straight", "Inverted"):
        sign = -1 if name == "Inverted" else 1
        return (np.array([s, 0*s, 0*s]), np.array([1+0*s, 0*s, 0*s]),
                np.array([0*s, sign+0*s, 0*s]), np.array([0*s, 0*s, sign+0*s]))
    if name == "Loop":
        a = s/8
        return (np.array([8*np.sin(a), 8*(1-np.cos(a)), 0*s]),
                np.array([np.cos(a), np.sin(a), 0*s]),
                np.array([-np.sin(a), np.cos(a), 0*s]), np.array([0*s, 0*s, 1+0*s]))
    rise = .3 if name == "Helix" else 0.
    h = np.sqrt(1-rise**2)
    a = s*h/8
    bank = .02*s+.001*s*s if name == "Bank" else .4+.04*s if name == "Helix" else 0*s
    n = np.array([-rise*np.cos(a), h+0*s, -rise*np.sin(a)])
    l = np.array([-np.sin(a), 0*s, np.cos(a)])
    return (np.array([8*np.sin(a), rise*s, 8*(1-np.cos(a))]),
            np.array([h*np.cos(a), rise+0*s, h*np.sin(a)]),
            n*np.cos(bank)+l*np.sin(bank), -n*np.sin(bank)+l*np.cos(bank))


def position(name, s, offset):
    c, t, n, l = basis(name, s)
    return c+t*offset[0]+n*offset[1]+l*offset[2]


def normal(name, s, offset, role):
    # Complex-step P' has no subtractive cancellation. This independently
    # differentiates positions rather than consuming the emitted Rust jets.
    d = position(name, s+1e-20j, offset).imag/1e-20
    _, _, n, l = basis(name, s)
    b = np.cross(l, d) if role == "Running" else np.cross(d, n)
    return b/np.linalg.norm(b)


def rate(name, s):
    axes = basis(name, s)[1:]
    prime = [a.imag/1e-20 for a in basis(name, s+1e-20j)[1:]]
    return sum(np.cross(a, d) for a, d in zip(axes, prime))*.5


def derivatives(function, s):
    # Overdetermined degree-12 polynomial of POSITION values on a 0.2 m
    # stencil, independent of the Rust third-order forward derivatives.
    x = np.linspace(-1., 1., 25)
    c = polyfit(x, np.array([function(s+.1*t) for t in x]), 12)
    return c[0], c[1]/.1, 2*c[2]/.01, 6*c[3]/.001


def main():
    geometry = []
    systems = {}
    worst = {k: 0. for k in ("p", "d", "dd", "ddd", "n", "dn", "ddn", "k", "dk")}
    for line in Path(sys.argv[1]).read_text(encoding="utf-8-sig").splitlines():
        if "M4D_GEOMETRY " in line:
            d = json.loads(line.split("M4D_GEOMETRY ", 1)[1])
            geometry.append(d)
            name, s, offset, role = d["name"], d["s"], d["offset"], d["role"]
            p = derivatives(lambda s: position(name, s, offset), s)
            n = derivatives(lambda s: normal(name, s, offset, role), s)
            k = derivatives(lambda s: rate(name, s), s)
            expected = dict(zip(("p", "d", "dd", "ddd"), p))
            expected.update(dict(zip(("n", "dn", "ddn"), n[:3])))
            expected.update(k=k[0], dk=k[1])
            for key, value in expected.items():
                error = np.max(np.abs(np.array(d[key])-value))
                worst[key] = max(worst[key], error)
                tolerance = 2e-8 if key == "ddd" else 2e-9
                assert error < tolerance, (name, s, role, key, error)
        if "M4D_SYSTEM " not in line:
            continue
        d = json.loads(line.split("M4D_SYSTEM ", 1)[1])
        name = d["name"]
        assert name not in systems, name
        systems[name] = d
        j, v, gamma, loads, acceleration = [np.array(d[k]) for k in ("j", "v", "gamma", "loads", "acceleration")]
        bodies = j.shape[1]//6
        assert j.shape == (6*bodies-1, 6*bodies) and bodies in (7, 11)
        assert all(np.isfinite(a).all() for a in (j, v, gamma, loads, acceleration))
        singular = np.linalg.svd(j, compute_uv=False)
        for threshold in (1e-5, 1e-7, 1e-9):
            assert np.count_nonzero(singular > threshold) == j.shape[0], name
        inv = np.tile([.01]*3+[.05]*3, bodies)
        free = np.tile([0., -9.81, 0., 0., 0., 0.], bodies)
        independent = np.linalg.solve((j*inv)@j.T, -gamma-j@free)
        reaction_error = np.max(np.abs(loads-independent))
        residual = np.max(np.abs(j@acceleration+gamma))
        power = abs(loads@(j@v))
        assert reaction_error < 1e-7 and residual < 1e-7 and power < 1e-7, name
        assert np.max(np.abs(acceleration-free-inv*(j.T@independent))) < 1e-8, name
        assert np.max(np.abs(j@v)) < 1e-9, name
        print(f"{name}: rank={len(loads)} reaction_error={reaction_error:.3e} acceleration_residual={residual:.3e} power_W={power:.3e}")
    assert len(geometry) == 216 and len(systems) == 18, (len(geometry), len(systems))
    assert {d["name"] for d in geometry} == {"Straight", "Turn", "Bank", "Helix", "Loop", "Inverted"}
    print(f"Independent geometry: {len(geometry)} cases, max absolute errors: {worst}")
    print(f"Independent M4D matrix verification: {len(systems)} systems passed")


if __name__ == "__main__":
    main()
