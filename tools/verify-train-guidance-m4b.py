"""Independent SVD and mass-weighted conditioning of the M4B research matrices.

Usage: python tools/verify-train-guidance-m4b.py target/m4b/research-release.txt
NumPy is optional developer verification tooling, never a runtime dependency.
"""
import json
import sys
from pathlib import Path

import numpy as np


def main():
    count = 0
    names = set()
    for line in Path(sys.argv[1]).read_text(encoding="utf-8-sig").splitlines():
        if "M4B_MATRIX " not in line:
            continue
        data = json.loads(line.split("M4B_MATRIX ", 1)[1])
        name = data["name"]
        assert name not in names, name
        names.add(name)
        a = np.asarray(data["a"], dtype=np.float64)
        assert np.isfinite(a).all()
        singular = np.linalg.svd(a, compute_uv=False)
        rank = data["rank"]
        for tolerance in (1e-5, 1e-7, 1e-9):
            assert np.count_nonzero(singular > tolerance) == rank, (name, singular)
        _, _, vt = np.linalg.svd(a, full_matrices=True)
        residual = np.max(np.abs(a @ vt[rank:].T)) if rank < a.shape[1] else 0.0
        assert residual < 1e-8, (name, residual)
        # Angular coordinates scaled by characteristic length 1 m. These
        # homogeneous 100 kg / 20 kg m² fixtures use this exact inverse mass.
        inv = np.tile([0.01] * 3 + [0.05] * 3, a.shape[1] // 6)
        mass_singular = np.linalg.svd(a * np.sqrt(inv), compute_uv=False)
        print(f"{name}: rows={a.shape[0]} rank={rank} mobility={a.shape[1]-rank} "
              f"redundant={a.shape[0]-rank} condition_J={singular[0]/singular[rank-1]:.9g} "
              f"condition_Gram={(mass_singular[0]/mass_singular[rank-1])**2:.9g} "
              f"null_residual={residual:.3e}")
        count += 1
    assert count == 29, f"Expected complete 29-matrix suite; received {count}"
    print(f"Independent M4B NumPy SVD: {count} matrices passed")


if __name__ == "__main__":
    main()
