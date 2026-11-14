"""Decompose the M4H R8 tick-563 velocity residual.

R = J(x1) v1 (the gate quantity) is split exactly as
    R = -beta c0/h            (stabilization target)
      + r                     (finite-sweep residual, J0 v1 - target)
      + h (gamma(x0,v1) - gamma0)   (first-order change of curvature term)
      + rem                   (Taylor remainder, should be O(h^2))
The independent truncation prediction uses ONLY the recorded gamma history:
    c_{k+1} = (1-beta) c_k - h^2 gamma_k + h^2/2 gamma_{k+1}
    R_{k+1} = -beta c_k / h + h (gamma_{k+1} - gamma_k)
started from the measured c at the first substep of tick 1. No J, impulse or
PGS data enter the prediction.
"""
import json, sys
import numpy as np

BETA = 0.2
DT = 1 / 240


def main(path):
    rows = [json.loads(l) for l in open(path)]
    meta = next(r for r in rows if r["kind"] == "meta")
    names = meta["rows"]
    ticks = [r for r in rows if r["kind"] == "tick"]
    h = DT / 4
    # flatten substeps
    C, Gm, SW, RR, C1 = [], [], [], [], []
    for t in ticks:
        for s in t["subs"]:
            C.append(s["c"]); Gm.append(s["gamma"]); SW.append(s["sweep"]); RR.append(s["R"]); C1.append(s["c1"])
    C, Gm, SW, RR, C1 = map(np.array, (C, Gm, SW, RR, C1))
    nsub = len(C)
    # independent recurrence prediction from gamma history only
    cp = np.zeros_like(C); Rp = np.zeros_like(C)
    cp[0] = C[0]
    for k in range(nsub - 1):
        cp[k + 1] = (1 - BETA) * cp[k] - h * h * Gm[k] + 0.5 * h * h * Gm[k + 1]
        Rp[k] = -BETA * cp[k] / h + h * (Gm[k + 1] - Gm[k])
    last = nsub - 1
    Rp[last] = -BETA * cp[last] / h  # gamma_{k+1} unavailable for final substep

    cand = slice(nsub - 4, nsub)
    k_max = int(np.argmax(np.abs(RR[cand]).max(axis=1))) + nsub - 4
    i_max = int(np.argmax(np.abs(RR[k_max])))
    print("candidate tick 563: gate quantity max|R| = %.10e at substep %d, row %d %s" % (abs(RR[k_max, i_max]), k_max - (nsub - 4), i_max, names[i_max]))
    print("max|c1| at candidate = %.10e" % np.abs(C1[cand]).max())

    subs = [r for r in rows if r["kind"] == "candidate_sub"]
    print("\nPer-substep decomposition of R for row %d (%s):" % (i_max, names[i_max]))
    print(" sub |      R       | -beta c0/h  |  sweep r    | h(dgamma)   | Taylor rem  | R_converged | 1/2 h gamma0 | recurrence R")
    for j, s in enumerate(subs):
        k = nsub - 4 + j
        gv1 = np.array(s["gamma_v1"]); Rc = np.array(s["R_converged"])
        stab = -BETA * C[k] / h
        dg = h * (gv1 - Gm[k])
        rem = RR[k] - (stab + SW[k] + dg)
        i = i_max
        print(" %d | %+.4e | %+.4e | %+.4e | %+.4e | %+.4e | %+.4e | %+.4e | %+.4e" % (
            j, RR[k, i], stab[i], SW[k, i], dg[i], rem[i], Rc[i], 0.5 * h * Gm[k, i], Rp[k, i]))
    # all-row norms at the candidate
    print("\nAll rows, candidate substeps (max abs over rows):")
    for j, s in enumerate(subs):
        k = nsub - 4 + j
        gv1 = np.array(s["gamma_v1"]); Rc = np.array(s["R_converged"])
        stab = -BETA * C[k] / h
        dg = h * (gv1 - Gm[k])
        rem = RR[k] - (stab + SW[k] + dg)
        print(" sub %d: |R| %.3e  |stab| %.3e  |sweep| %.3e  |h dgamma| %.3e  |rem| %.3e  |R-Rconv| %.3e  |R-Rrecur| %.3e  |impulse gap|/|impulse| %.2e" % (
            j, np.abs(RR[k]).max(), np.abs(stab).max(), np.abs(SW[k]).max(), np.abs(dg).max(), np.abs(rem).max(),
            np.abs(RR[k] - Rc).max(), np.abs(RR[k] - Rp[k]).max(),
            np.abs(s["impulse_gap"]).max() / np.abs(s["impulse"]).max()))
    # explanation quality over the whole run
    over = np.abs(RR[:-1] - Rp[:-1]).max(axis=1) / np.maximum(np.abs(RR[:-1]).max(axis=1), 1e-12)
    print("\nRecurrence (gamma-only) prediction vs measured R over all %d substeps: median rel err %.3e, p99 %.3e, max %.3e" % (
        nsub - 1, np.median(over), np.quantile(over, 0.99), over.max()))
    cerr = np.abs(C - cp).max(axis=1) / np.maximum(np.abs(C).max(axis=1), 1e-12)
    print("Recurrence prediction vs measured c: median rel err %.3e, p99 %.3e (after tick 10)" % (np.median(cerr[40:]), np.quantile(cerr[40:], 0.99)))
    sweep_rel = np.abs(SW).max(axis=1) / np.maximum(np.abs(RR).max(axis=1), 1e-12)
    print("Finite-sweep residual / gate quantity: median %.3e, max %.3e" % (np.median(sweep_rel), sweep_rel.max()))
    # steady formula per row at candidate
    k = k_max
    ratio = RR[k] / (0.5 * h * Gm[k])
    big = np.abs(Gm[k]) > 0.1 * np.abs(Gm[k]).max()
    print("R / (h gamma/2) on rows with significant gamma at the max substep: min %.3f max %.3f" % (ratio[big].min(), ratio[big].max()))
    # which rows dominate over the run
    worst = np.argmax(np.abs(RR), axis=1)
    kinds = {}
    for w in worst:
        kinds[names[w].split("/")[0][:7]] = kinds.get(names[w].split("/")[0][:7], 0) + 1
    print("rows giving the per-substep max |R| (by prefix):", kinds)
    print("gate row %s: gamma0 = %.6e (=> curvature-type acceleration), c0 = %.4e" % (names[i_max], Gm[k_max, i_max], C[k_max, i_max]))

    # candidate geometry/bearing checks
    c = next(r for r in rows if r["kind"] == "candidate")
    sup = np.array(c["supports"], dtype=float)
    print("\nProjection at candidate: min D %.4f, min window margin %.4f m, min distance to source end %.4f m, max |gap| %.3e m, max source refinement diff %.3e m / %.3e rad, charts %s" % (
        sup[:, 4].min(), sup[:, 5].min(), sup[:, 6].min(), np.abs(sup[:, 3]).max(), sup[:, 7].max(), sup[:, 8].max(), sorted(set(sup[:, 9].astype(int)))))
    b = c["bearings"]
    print("Bearings: max swing %.4f rad (limit %.2f), max anchor gap %.3e m, max roll twist %.3e rad" % (
        max(x[2] for x in b), meta["max_swing"], max(x[3] for x in b), max(abs(x[4]) for x in b)))
    print("Energy drift at candidate: %.6f J" % c["energy_drift"])

    # trajectory error via Richardson (first order) from global runs at tick 563
    g = {(r["n"], r["tick"]): r for r in rows if r["kind"] == "global" and "x" in r}
    for tick in (562, 563):
        x4, x16, x64 = (np.array(g[(n, tick)]["x"]) for n in (4, 16, 64))
        v4, v16, v64 = (np.array(g[(n, tick)]["vel"]) for n in (4, 16, 64))
        xs = x64 + (x64 - x16) / 3; vs = v64 + (v64 - v16) / 3
        pos = lambda x: x.reshape(-1, 7)[:, :3]
        e4 = np.linalg.norm(pos(x4) - pos(xs), axis=1).max(); e16 = np.linalg.norm(pos(x16) - pos(xs), axis=1).max()
        ratio = np.linalg.norm(pos(x4) - pos(x16), axis=1).max() / np.linalg.norm(pos(x16) - pos(x64), axis=1).max()
        print("tick %d trajectory error vs Richardson reference: n=4 %.4e m, n=16 %.4e m; successive-difference ratio %.3f (4 => first order); n=4 velocity err %.3e m/s" % (
            tick, e4, e16, ratio, np.abs(v4 - vs).max()))
    loc = {r["n"]: r for r in rows if r["kind"] == "local"}
    print("\nLocal replay of tick 563 only, from the n=4 accepted state: n=16 v=%.4e, n=64 v=%.4e (inherited c0 makes beta*c0/h grow as h shrinks)" % (loc[16]["v"], loc[64]["v"]))


if __name__ == "__main__":
    main(sys.argv[1])
