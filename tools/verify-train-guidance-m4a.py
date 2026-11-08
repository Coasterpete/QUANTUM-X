"""Independent SVD of test-only M4A matrices (requires NumPy, no app dependency).

Usage: python tools/verify-train-guidance-m4a.py target/m4a/fixtures-release.txt
Generate input with the mechanical_guidance test's --nocapture --test-threads=1.
"""
import json
import sys
from pathlib import Path

import numpy as np


def main():
    count = 0
    for line in Path(sys.argv[1]).read_text(encoding="utf-8-sig").splitlines():
        marker = "M4A_MATRIX "
        if marker not in line:
            continue
        row = json.loads(line.split(marker, 1)[1])
        a = np.array(row["a"], dtype=np.float64)
        assert np.isfinite(a).all()
        _, singular, vt = np.linalg.svd(a, full_matrices=True)
        expected = row["rank"]
        for tolerance in (1e-5, 1e-7, 1e-9):
            actual = int(np.sum(singular > tolerance))
            assert actual == expected, (row["name"], tolerance, actual, expected)
        null = vt[expected:].T
        residual = np.max(np.abs(a @ null)) if null.size else 0.0
        assert residual < 1e-8, (row["name"], residual)
        smallest = singular[expected - 1]
        rejected = singular[expected] if expected < len(singular) else 0.0
        print(f'{row["name"]}: rank={expected}, mobility={a.shape[1]-expected}, '
              f'sigma_min_retained={smallest:.9e}, sigma_max_rejected={rejected:.9e}, '
              f'condition_retained={singular[0]/smallest:.6g}, null_residual={residual:.3e}')
        count += 1
    assert count == 38, f"Expected the complete 38-matrix fixture set, found {count}"
    print(f"Independent NumPy SVD: {count} matrices passed")


if __name__ == "__main__":
    main()
