"""Independently differentiate emitted poses; no graph RHS or Rust jet algebra.

Usage: python tools/verify-graph-source-m4e-b.py target/m4e-b/graph-release.log
Requires NumPy. Fits positions and the three rotated unit axes on a degree-10
overdetermined stencil. k and its derivatives follow from orthogonal-frame
identities, not from the graph controls or quaternion integrator.
"""
import ast
import math
import sys
from pathlib import Path

import numpy as np
from numpy.polynomial import polynomial as poly


def verify(path):
    count = 0
    worst = np.zeros(6)
    worst_offset = np.zeros(3)
    offset_count = 0
    pending = None
    for line in Path(path).read_text(encoding="utf-8-sig").splitlines():
        if "M4EB_STENCIL " in line:
            line = line[line.index("M4EB_STENCIL "): ]
            _, name, center, h, rows = line.split(" ", 4)
            pending = (name, float(center), float(h), np.array(ast.literal_eval(rows)))
        elif line.startswith("M4EB_JET "):
            assert pending is not None
            name, center, h, rows = pending
            expected = np.array(ast.literal_eval(line[len("M4EB_JET "):])).reshape(6, 3)
            # Normalize the stencil to [-1,1] before fitting, subtracting the
            # central position to avoid fitting a large irrelevant constant.
            width = 8 * h
            coordinates = np.arange(-8, 9) / 8
            shifted = rows.copy()
            shifted[:, :3] -= rows[8, :3]
            coefficients = poly.polyfit(coordinates, shifted, 10)
            derivatives = [coefficients[order] * math.factorial(order) / width**order
                           for order in range(1, 4)]
            axes = rows[8, 3:].reshape(3, 3)
            first, second, third = [d[3:].reshape(3, 3) for d in derivatives]
            k = np.cross(axes, first).sum(axis=0) / 2
            dk = np.cross(axes, second).sum(axis=0) / 2
            ddk = (np.cross(first, second) + np.cross(axes, third)).sum(axis=0) / 2
            observed = np.array([d[:3] for d in derivatives] + [k, dk, ddk])
            errors = np.linalg.norm(observed - expected, axis=1)
            assert np.isfinite(errors).all()
            # Position-only third differentiation is conditioned by f64 input
            # noise. These are independent check tolerances, not B1 tolerances.
            tolerances = np.array([2e-9, 2e-8, 2e-6, 2e-9, 2e-8, 2e-6])
            assert (errors < tolerances).all(), (name, center, errors)
            worst = np.maximum(worst, errors)
            count += 1
            pending = None
        elif line.startswith("M4EB_OFFSET "):
            offset_jet = np.array(ast.literal_eval(line[len("M4EB_OFFSET "):])).reshape(4, 3)
            offsets = rows[:, 3:].reshape(17, 3, 3)
            positions = rows[:, :3] + np.einsum("ijk,j->ik", offsets, offset_jet[0])
            fitted = poly.polyfit(coordinates, positions-positions[8], 10)
            derived = np.array([fitted[j] * math.factorial(j) / width**j for j in range(1, 4)])
            errors = np.linalg.norm(derived-offset_jet[1:], axis=1)
            assert (errors < [2e-9, 2e-8, 2e-6]).all(), (name, center, errors)
            worst_offset = np.maximum(worst_offset, errors)
            offset_count += 1
    assert pending is None
    assert count == 8, f"missing stencil evidence: {count}/8"
    assert offset_count == 8, f"missing constant-offset evidence: {offset_count}/8"
    print(f"PASS {count} pose-only stencil cases; max C'/C''/C'''/k/k'/k'' errors {worst.tolist()}")
    print(f"PASS {offset_count} constant-offset position stencils; max P'/P''/P''' errors {worst_offset.tolist()}")


if __name__ == "__main__":
    verify(sys.argv[1])
