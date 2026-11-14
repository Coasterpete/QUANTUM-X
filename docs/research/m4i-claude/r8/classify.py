"""Prototype truncation-vs-failure classifier (offline, scratch only).

For every row i and internal step k, predict the constraint residuals that the
known discretization alone would produce, from quantities the solver already
has, WITHOUT using the measured residual itself:
    c_pred[k+1] = (1-beta) c[k] - h^2 gamma[k] + h^2/2 gamma[k+1] + h r[k]
    R_pred[k]   = -beta c[k]/h + h (gamma[k+1]-gamma[k]) + r[k]
where c[k] is the measured gap at the start of step k, gamma the solver's
curvature term and r the finite-sweep residual (solver output). The
UNEXPLAINED parts e_c = c[k+1]-c_pred[k+1] and e_R = R[k]-R_pred[k] are O(h^3)
and O(h^2) for a smooth, correctly constrained trajectory. A genuine failure
(projection jump, lost branch, geometric kink, incompatible constraint, state
corruption) appears in e_c / e_R regardless of h.
"""
import json, sys
import numpy as np

BETA = 0.2
H = 1 / 960


def load(path):
    rows = [json.loads(l) for l in open(path)]
    ticks = [r for r in rows if r["kind"] == "tick"]
    C, Gm, SW, RR, C1, T = [], [], [], [], [], []
    for t in ticks:
        for s in t["subs"]:
            C.append(s["c"]); Gm.append(s["gamma"]); SW.append(s["sweep"]); RR.append(s["R"]); C1.append(s["c1"]); T.append(t["tick"])
    return ticks, *map(np.array, (C, Gm, SW, RR, C1, T))


def unexplained(C, Gm, SW, RR, C1):
    ec = C1[:-1] - ((1 - BETA) * C[:-1] - H * H * Gm[:-1] + 0.5 * H * H * Gm[1:] + H * SW[:-1])
    eR = RR[:-1] - (-BETA * C[:-1] / H + H * (Gm[1:] - Gm[:-1]) + SW[:-1])
    return np.abs(ec).max(axis=1), np.abs(eR).max(axis=1)


def handoff(C, C1):
    """Start-of-step gap must equal the previous step's end-of-step gap: same
    pose, so any jump is a state/branch/model discontinuity, never truncation."""
    return np.abs(C[1:] - C1[:-1]).max(axis=1)


for path in sys.argv[1:]:
    ticks, C, Gm, SW, RR, C1, T = load(path)
    ec, eR = unexplained(C, Gm, SW, RR, C1)
    eh = handoff(C, C1)
    gate_R = np.abs(RR).max(axis=1)[:-1]
    for lo, hi in [(1, 300), (301, 301), (302, 563)]:
        m = (T[1:] >= lo) & (T[1:] <= hi)
        if m.any():
            print(" handoff ticks %3d-%3d: max |c_start - c_end(prev)| %.3e m" % (lo, hi, eh[m].max()))
    print("==", path, "substeps", len(C))
    print(" max |c| %.3e   max |R| %.3e   max |sweep r| %.3e" % (np.abs(C1).max(), gate_R.max(), np.abs(SW).max()))
    for lo, hi in [(1, 10), (11, 300), (301, 302), (303, 330), (331, 563)]:
        m = (T[:-1] >= lo) & (T[:-1] <= hi)
        if m.any():
            print(" ticks %3d-%3d: max unexplained |e_c| %.3e m, |e_R| %.3e m/s; max gate |R| %.3e" % (lo, hi, ec[m].max(), eR[m].max(), gate_R[m].max()))
