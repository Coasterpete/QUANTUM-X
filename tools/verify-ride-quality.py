"""Verify real 60 FPS frames and summarize native draw/worker/pose measurements.
Usage: python tools/verify-ride-quality.py recording-dir [native-profile-dir ...]
Requires FFmpeg/FFprobe already installed; no third-party Python modules.
"""
import json
import math
import statistics
import subprocess
import sys
from pathlib import Path


def xyz(v):
    return [v[k] for k in "xyz"]


def rotate(q, p):
    w, x, y, z = [q[k] for k in "wxyz"]
    assert abs(w*w+x*x+y*y+z*z-1) < 1e-9
    return [
        (1-2*y*y-2*z*z)*p[0]+2*(x*y-z*w)*p[1]+2*(x*z+y*w)*p[2],
        2*(x*y+z*w)*p[0]+(1-2*x*x-2*z*z)*p[1]+2*(y*z-x*w)*p[2],
        2*(x*z-y*w)*p[0]+2*(y*z+x*w)*p[1]+(1-2*x*x-2*y*y)*p[2],
    ]


def dot(a, b):
    return sum(x*y for x, y in zip(a, b))


def cross(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]


def stats(values):
    values = sorted(v for v in values if v is not None)
    if not values:
        return None
    return dict(median=statistics.median(values), p95=values[math.ceil(len(values)*.95)-1],
                p99=values[math.ceil(len(values)*.99)-1], maximum=max(values), mean=statistics.mean(values))


def camera_error(body, camera):
    position = xyz(body["position_m"])
    q = body.get("rotation", body.get("orientation"))
    eye = [a+b for a, b in zip(position, rotate(q, [0, 1.2, 0]))]
    error = math.dist(eye, xyz(camera["eye_m"]))
    error = max(error, math.dist(rotate(q, [1, 0, 0]), xyz(camera["forward"])),
                math.dist(rotate(q, [0, 1, 0]), xyz(camera["up"])))
    assert error < 1e-8, error
    return error


def orientation_delta(a, b):
    af, au, bf, bu = xyz(a["forward"]), xyz(a["up"]), xyz(b["forward"]), xyz(b["up"])
    return math.degrees(math.acos(max(-1, min(1, (dot(af, bf)+dot(au, bu)+dot(cross(af, au), cross(bf, bu))-1)/2))))


def native(path):
    e = json.loads((path / "native-frames.json").read_text(encoding="utf-8"))
    assert e["native_winit_surface"] and e["hidden_window"]
    assert not e["visible_display_refresh_measured"]
    rows = e["rows"]
    assert rows and all(r["draw"]["presented"] for r in rows)
    assert all(r["draw"]["size"] == [1440, 960] for r in rows)
    sync = max(camera_error(r["pose"]["body"], r["pose"]["camera"]) for r in rows)
    for r in rows:
        pose=r["pose"];body=pose["body"];expected=xyz(body["position_m"])
        asset=pose.get("visual_correction")
        if asset:
            pivot=xyz(asset.get("pivot_m",dict(x=0,y=0,z=0)))
            shift=rotate(asset["rotation"],[v*s for v,s in zip(pivot,xyz(asset["scale"]))])
            local=[t-v for t,v in zip(xyz(asset["translation_m"]),shift)]
            expected=[p+v for p,v in zip(expected,rotate(body["rotation"],local))]
        assert math.dist(pose["instance_origin"],expected)<1e-8
        assert r["pose"]["error"] is None and r["pose"]["worker"]["error"] is None
    measurements, deltas, clock_errors = [], [], []
    for phase in [1, 4]:
        q = [r for r in rows if r["pose"]["phase"] == phase]
        assert q
        q = [r for r in q if r["wall_s"] > q[0]["wall_s"]+.15]
        measurements.extend(q)
        for a, b in zip(q, q[1:]):
            assert b["pose"]["presentation_tick"] >= a["pose"]["presentation_tick"]
            deltas.append(orientation_delta(a["pose"]["camera"], b["pose"]["camera"]))
            if a["pose"].get("sample_wall_s") is not None:
                rendered = (b["pose"]["presentation_tick"]-a["pose"]["presentation_tick"])/240
                wall = b["pose"]["sample_wall_s"]-a["pose"]["sample_wall_s"]
                clock_errors.append(abs(rendered-wall)*1000)
    for phase in [2, 3, 5]:
        q = [r for r in rows if r["pose"]["phase"] == phase]
        assert q and not q[-1]["pose"]["worker"]["running"]
        held=[r for r in q if not r["pose"]["worker"]["running"]]
        assert all(r["pose"]["worker"]["tick"]==held[-1]["pose"]["worker"]["tick"] for r in held)
        assert all(r["pose"]["camera"]==held[-1]["pose"]["camera"] for r in held), "paused camera drift"
        if phase == 3:
            assert q[-1]["pose"]["worker"]["tick"] == 0
            initial=rows[0]["pose"]["body"]
            reset=held[-1]["pose"]["body"]
            assert math.dist(xyz(initial["position_m"]),xyz(reset["position_m"]))<1e-12
            assert math.dist([initial["rotation"][k] for k in "wxyz"],[reset["rotation"][k] for k in "wxyz"])<1e-12
    assert max(deltas) < 90, "camera discontinuity"
    worker = max((r["pose"]["worker"] for r in measurements if r["pose"]["phase"] == 4), key=lambda w: w["steps"])
    summary = dict(path=str(path), target_hz=e["target_hz"], legacy_wait=e["legacy_16ms_wait"],
                   monitor_refresh_millihertz=e.get("monitor_refresh_millihertz"),
                   present_mode=e.get("present_mode"),
                   native_frames=len(rows), measured_frames=len(measurements),
                   frame_interval_ms=stats(r["frame_interval_ms"] for r in measurements),
                   cpu_frame_ms=stats(r["frame_cpu_ms"] for r in measurements),
                   ui_ms=stats(r["ui_ms"] for r in measurements),
                   pose_ms=stats(r["pose"]["pose_update_ms"] for r in measurements),
                   camera_ms=stats(r["pose"]["camera_update_ms"] for r in measurements),
                   acquire_ms=stats(r["draw"]["acquire_ms"] for r in measurements),
                   encode_submit_ms=stats(r["draw"]["encode_submit_ms"] for r in measurements),
                   present_ms=stats(r["draw"]["present_ms"] for r in measurements),
                   pose_clock_error_ms=stats(clock_errors), camera_angle_degrees=stats(deltas),
                   worker_mean_ms=worker["step_total_ms"]/worker["steps"], worker_max_ms=worker["step_max_ms"],
                   worker_over_budget=worker["over_budget_steps"], worker_lag_events=worker["lag_events"],
                   starved_frames=sum(r["pose"]["starved"] for r in measurements),
                   missed_deadlines_including_warmup=e["missed_deadlines"],
                   late_intervals=sum(r["frame_interval_ms"] > 1500/e["target_hz"] for r in measurements),
                   camera_vehicle_max_error=sync, play_pause_reset_verified=True)
    print(f"Native {e['target_hz']} Hz / legacy={e['legacy_16ms_wait']}: "
          f"frame median/p95 {summary['frame_interval_ms']['median']:.3f}/{summary['frame_interval_ms']['p95']:.3f} ms; "
          f"worker max {worker['step_max_ms']:.3f} ms; starved {summary['starved_frames']}")
    return summary


def recording(path):
    e = json.loads((path / "evidence.json").read_text(encoding="utf-8"))
    frames = e["frames"]
    project=json.loads((path / "inspection.qx.json").read_text(encoding="utf-8"))
    train=next(t for t in project["trains"] if t["id"]==project["heartline"]["train_configuration_id"])
    attached_id=train["cars"][0]["body_id"]
    assert e["fps"] == 60 and e["width"] == 1440 and e["height"] == 960
    assert e["input_unchanged"] and e["offscreen"]
    assert len(frames) == e["frame_count"] >= 120
    assert len({f["render"]["pixel_hash"] for f in frames}) == len(frames), "duplicate rasterized frames"
    errors, deltas = [], []
    for i, f in enumerate(frames):
        assert f["frame"] == i and abs(f["output_time_s"]-i/60) < 1e-10
        assert abs(f["presentation_tick"]-(i*4+.5)) < 1e-5
        assert abs(f["alpha"]-.5) < 1e-5
        body=next(c for c in f["components"] if c["id"]==attached_id)
        errors.append(camera_error(body, f["camera"]))
        assert (path / f["file"]).exists()
        if i:
            deltas.append(orientation_delta(frames[i-1]["camera"], f["camera"]))
    assert max(deltas) < 90
    video = path / "physics-rider-pov-60.mp4"
    stream = json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height,r_frame_rate,nb_frames,duration", "-of", "json", str(video)]))["streams"][0]
    assert stream["r_frame_rate"] == "60/1" and int(stream["nb_frames"]) == len(frames)
    assert [stream["width"], stream["height"]] == [1440, 960]
    digests = subprocess.check_output(["ffmpeg", "-v", "error", "-i", str(video), "-map", "0:v:0", "-f", "framemd5", "-"], text=True)
    hashes = [line.split(",")[-1].strip() for line in digests.splitlines() if line and not line.startswith("#")]
    assert len(hashes) == len(frames) == len(set(hashes)), "duplicate decoded video frames"
    pts = json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "frame=best_effort_timestamp_time", "-of", "json", str(video)]))["frames"]
    assert len(pts) == len(frames)
    assert all(abs(float(p["best_effort_timestamp_time"])-i/60) < 2e-6 for i, p in enumerate(pts))
    result = dict(frame_count=len(frames), fps=60, duration_s=float(stream["duration"]), unique_rasterized_frames=len(frames), unique_decoded_frames=len(set(hashes)), camera_vehicle_max_error=max(errors), camera_angle_degrees=stats(deltas),gpu_ms=stats(f["render"]["gpu_ms"] for f in frames[8:]),encode_submit_ms=stats(f["render"]["encode_submit_ms"] for f in frames[8:]),readback_ms=stats(f["render"]["wait_readback_ms"] for f in frames[8:]),png_ms=stats(f["render"]["png_ms"] for f in frames[8:]),stop_error=e["stop_error"])
    print(f"Video PASS: {len(frames)} unique rendered AND decoded frames, 1440x960 at 60 FPS; no frame duplication or synthesized video frames")
    return result


if __name__ == "__main__":
    root = Path(sys.argv[1])
    summary = dict(recording=recording(root), native=[native(Path(p)) for p in sys.argv[2:]])
    (root / "verification.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
