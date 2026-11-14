"""Independent analytic references for harness output.

Every reference here is computed from the AUTHORED fixture parameters only
(theta, R, bank, s0, v0, m, I, g). Engine output supplies states; no engine
Jacobian, impulse, projection or track sample is used to form a reference.

Single-body assumption: all trajectory/reaction references below assume ONE
rigid body (point mass, or a carriage whose attitude follows the rail frame).
Articulated runs are compared only against exact energy conservation.
"""
import json, math, sys
import numpy as np

DT = 1.0 / 240.0
G = 9.81
M = 100.0
I = 20.0
THETA = 0.1
R = 20.0
BANK = 0.6
S0 = 5.0
V0 = {"incline": 5.0, "circle": 15.0, "banked": 15.0}
GVEC = np.array([0.0, -G, 0.0])


def analytic_frame(case, s):
    if case == "incline":
        t = np.array([math.cos(THETA), -math.sin(THETA), 0.0])
        n = np.array([math.sin(THETA), math.cos(THETA), 0.0])
        return s * t, t, n
    a = s / R
    p = np.array([R * math.sin(a), 0.0, R * (1 - math.cos(a))])
    t = np.array([math.cos(a), 0.0, math.sin(a)])
    inward = np.array([-math.sin(a), 0.0, math.cos(a)])
    phi = BANK if case == "banked" else 0.0
    n = math.cos(phi) * np.array([0.0, 1.0, 0.0]) + math.sin(phi) * inward
    return p, t, n


def angle_err(a, b):
    c = np.clip(np.dot(a, b) / np.linalg.norm(a) / np.linalg.norm(b), -1, 1)
    return math.acos(c)


def station_and_offsets(case, x):
    """Analytic centerline station of a point, plus its radial/normal offsets."""
    if case == "incline":
        _, t, n = analytic_frame(case, 0.0)
        s = float(x @ t)
        perp = x - s * t
        return s, float(np.linalg.norm(perp)), float(perp @ n)
    c = np.array([0.0, 0.0, R])
    d = x - c
    angle = math.atan2(d[0], -d[2])
    r = math.hypot(d[0], d[2])
    return R * angle, r, float(x[1])


def geometry(rows):
    out = {}
    for case in ("incline", "circle", "banked"):
        g = [r for r in rows if r["kind"] == "geom" and r["case"] == case]
        e = {"prod_p": 0, "src_p": 0, "prod_t": 0, "src_t": 0, "prod_n": 0, "src_n": 0}
        for r in g:
            p, t, n = analytic_frame(case, r["s"])
            for k in ("prod", "src"):
                e[k + "_p"] = max(e[k + "_p"], float(np.linalg.norm(np.array(r[k + "_p"]) - p)))
                e[k + "_t"] = max(e[k + "_t"], angle_err(np.array(r[k + "_t"]), t))
                e[k + "_n"] = max(e[k + "_n"], angle_err(np.array(r[k + "_n"]), n))
        out[case] = e
    return out


def energy(x, v, w):
    return 0.5 * M * v @ v + 0.5 * I * w @ w - M * GVEC @ x


def run_metrics(case, rows):
    v0 = V0[case]
    x = np.array([r["x"] for r in rows])
    v = np.array([r["v"] for r in rows])
    w = np.array([r["w"] for r in rows])
    ticks = np.array([r["tick"] for r in rows])
    t = ticks * DT
    speed = np.linalg.norm(v, axis=1)
    so = [station_and_offsets(case, xi) for xi in x]
    s = np.array([a for a, _, _ in so])
    if case != "incline":
        s = R * np.unwrap(s / R)  # continuous angle beyond +/- pi
    radial = np.array([b for _, b, _ in so])
    normal_off = np.array([c for _, _, c in so])
    m = {}
    # --- speed versus station (reference from authored geometry only)
    if case == "incline":
        acc = G * math.sin(THETA)
        vref = np.sqrt(np.maximum(v0 ** 2 + 2 * acc * (s - S0), 0))
        r0 = 0.0
    else:
        vref = np.full_like(s, v0)
        r0 = radial[0]
    m["speed_err_max"] = float(np.max(np.abs(speed - vref)))
    m["speed_err_end"] = float(speed[-1] - vref[-1])
    m["speed_err_tick1"] = float(speed[1] - vref[1])
    # --- travel time to station checkpoints
    errs = []
    for target in np.arange(S0 + 10, s.max(), 10.0):
        k = int(np.argmax(s >= target))
        if k == 0:
            continue
        te = t[k - 1] + (target - s[k - 1]) / (s[k] - s[k - 1]) * DT
        if case == "incline":
            acc = G * math.sin(THETA)
            tr = (-v0 + math.sqrt(v0 ** 2 + 2 * acc * (target - S0))) / acc
        else:
            tr = (target - S0) / R * r0 / v0
        errs.append(te - tr)
    m["travel_time_err_last_s"] = float(errs[-1]) if errs else None
    m["travel_time_err_max_s"] = float(max(errs, key=abs)) if errs else None
    m["station_end"] = float(s[-1])
    # --- independent path deviation (constraint error measured analytically)
    if case == "incline":
        m["path_dev_max_m"] = float(np.max(radial))
        m["initial_offset_m"] = float(radial[0])
    else:
        m["initial_radius_minus_R_m"] = float(r0 - R)
        m["radius_dev_max_m"] = float(np.max(np.abs(radial - r0)))
        m["height_dev_max_m"] = float(np.max(np.abs(normal_off - normal_off[0])))
    # --- reactions: tick-average constraint force from momentum change
    F = M * (v[1:] - v[:-1]) / DT - M * GVEC
    Fref = []
    for k in range(1, len(rows)):
        if case == "incline":
            _, tt, _ = analytic_frame(case, 0.0)
            Fref.append(M * (G * math.sin(THETA) * tt - GVEC))
        else:
            xm = 0.5 * (x[k] + x[k - 1])
            d = xm - np.array([0.0, 0.0, R])
            inward = -np.array([d[0], 0.0, d[2]]) / math.hypot(d[0], d[2])
            Fref.append(M * (v0 ** 2 / r0) * inward - M * GVEC)
    Fref = np.array(Fref)
    err = np.linalg.norm(F - Fref, axis=1)
    scale = np.linalg.norm(Fref, axis=1)
    m["reaction_tick1_err_N"] = float(err[0])
    m["reaction_tick1_N"] = float(np.linalg.norm(F[0]))
    steady = slice(min(24, len(err) - 1), None)
    m["reaction_steady_rel_max"] = float(np.max(err[steady] / scale[steady]))
    m["reaction_steady_rel_rms"] = float(np.sqrt(np.mean((err[steady] / scale[steady]) ** 2)))
    m["reaction_ref_N"] = float(scale[-1])
    # track-frame decomposition (banked: normal vs lateral)
    if case != "incline":
        k = len(F) - 1
        sk, _, _ = station_and_offsets(case, x[-1])
        _, tt, nn = analytic_frame(case, sk)
        ll = np.cross(tt, nn)
        mean_F = F[steady].mean(axis=0)
        mean_ref = Fref[steady].mean(axis=0)
        m["reaction_normal_lateral_N"] = [float(F[k] @ nn), float(F[k] @ ll)]
        m["reaction_ref_normal_lateral_N"] = [float(Fref[k] @ nn), float(Fref[k] @ ll)]
    # --- angular velocity reference (attitude follows rail frame)
    if case != "incline":
        m["omega_end"] = float(np.linalg.norm(w[-1]))
        m["omega_ref"] = float(v0 / r0)
    # --- energy
    E = np.array([energy(x[k], v[k], w[k]) for k in range(len(rows))])
    m["E0_J"] = float(E[0])
    m["energy_drift_J"] = float(E[-1] - E[0])
    m["energy_init_step_J"] = float(E[1] - E[0])
    m["energy_drift_after_tick1_J"] = float(E[-1] - E[1])
    m["energy_drift_rel"] = float((E[-1] - E[0]) / E[0])
    m["duration_s"] = float(t[-1])
    m["engine_perr_max"] = float(max(r.get("perr", 0) for r in rows))
    if "aerr" in rows[0]:
        m["engine_aerr_max"] = float(max(r.get("aerr", 0) for r in rows))
    if "verr" in rows[0]:
        m["engine_verr_max"] = float(max(r.get("verr", 0) for r in rows))
    return m


def main(path):
    rows = [json.loads(l) for l in open(path)]
    fails = [r for r in rows if r["kind"] == "fail"]
    result = {"geometry": geometry(rows), "fails": fails, "runs": {}, "articulated": {}}
    keys = sorted({(r["case"], r["engine"], r["variant"]) for r in rows if r["kind"] == "state"})
    for case, engine, variant in keys:
        rs = [r for r in rows if r["kind"] == "state" and r["case"] == case and r["engine"] == engine and r["variant"] == variant]
        rs.sort(key=lambda r: r["tick"])
        result["runs"][f"{case}/{engine}/{variant}"] = run_metrics(case, rs)
    for case in ("incline", "circle", "banked"):
        for engine in ("prod", "b1"):
            rs = [r for r in rows if r["kind"] == "artic" and r["case"] == case and r["engine"] == engine]
            if not rs:
                continue
            E = np.array([r["energy"] for r in rs])
            result["articulated"][f"{case}/{engine}"] = {
                "bodies": rs[0].get("bodies"),
                "ticks": rs[-1]["tick"],
                "E0_J": float(E[0]),
                "drift_J": float(E[-1] - E[0]),
                "init_step_J": float(E[1] - E[0]),
                "drift_after_tick1_J": float(E[-1] - E[1]),
                "rel": float((E[-1] - E[0]) / E[0]),
                "perr_max": float(max(r.get("perr", 0) for r in rs)),
            }
    json.dump(result, sys.stdout, indent=1)


if __name__ == "__main__":
    main(sys.argv[1])
