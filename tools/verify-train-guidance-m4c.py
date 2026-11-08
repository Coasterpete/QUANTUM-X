"""Independent M4C SVD, dense reactions and acceleration residuals.

Usage: python tools/verify-train-guidance-m4c.py target/m4c/release.log
NumPy is optional development tooling, never an application dependency.
The Rust suite independently checks gamma with second differences of c.
"""
import json
import sys
from pathlib import Path

import numpy as np


def main():
    systems = {}
    for line in Path(sys.argv[1]).read_text(encoding="utf-8-sig").splitlines():
        if "M4C_SYSTEM " not in line:
            continue
        data = json.loads(line.split("M4C_SYSTEM ", 1)[1])
        name = data["name"]
        assert name not in systems, name
        systems[name] = data
        j = np.asarray(data["j"], dtype=np.float64)
        v = np.asarray(data["v"], dtype=np.float64)
        gamma = np.asarray(data["gamma"], dtype=np.float64)
        loads = np.asarray(data["loads"], dtype=np.float64)
        acceleration = np.asarray(data["acceleration"], dtype=np.float64)
        assert j.shape == (41, 42), (name, j.shape)
        assert all(np.isfinite(x).all() for x in (j, v, gamma, loads, acceleration))
        singular = np.linalg.svd(j, compute_uv=False)
        for tolerance in (1e-5, 1e-7, 1e-9):
            assert np.count_nonzero(singular > tolerance) == 41, (name, singular)
        inv = np.tile([0.01] * 3 + [0.05] * 3, 7)
        gravity = np.tile([0.0, -9.81, 0.0, 0.0, 0.0, 0.0], 7)
        # NumPy LAPACK, rather than the Rust Gauss-Jordan implementation.
        independent = np.linalg.solve((j * inv) @ j.T, -gamma - j @ gravity)
        assert np.max(np.abs(loads - independent)) < 1e-7, name
        assert np.max(np.abs(acceleration - (gravity + inv * (j.T @ independent)))) < 1e-8, name
        residual = np.max(np.abs(j @ acceleration + gamma))
        velocity = np.max(np.abs(j @ v))
        power = abs(float(loads @ (j @ v)))
        assert residual < 1e-7 and velocity < 1e-10 and power < 1e-7, name
        print(f"{name}: rank=41 mobility=1 reaction_error={np.max(np.abs(loads-independent)):.3e} "
              f"acceleration_residual={residual:.3e} velocity_residual={velocity:.3e} power_W={power:.3e}")
    assert set(systems) == {"Straight", "Turn", "Bank", "Helix", "Loop", "Inverted"}, set(systems)
    print("Independent M4C NumPy verification: 6 systems passed")


if __name__ == "__main__":
    main()
