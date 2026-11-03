"""Verify authentic Editor evidence, measured dynamics and distinct ride cameras."""
import csv
import hashlib
import json
import pathlib
import sys


def verify(directory):
    root = pathlib.Path(directory)
    data = json.loads((root / "evidence.json").read_text(encoding="utf-8"))
    assert data["offscreen_actual_editor"] and not data["desktop_control"]
    assert data["project_roundtrip"] and data["completion_undo_redo"]
    assert abs(data["fixed_step_s"] - 1 / 240) < 1e-15
    closure = data["closure"]
    assert closure["valid"] and closure["position_m"] < 0.001
    assert closure["tangent_rad"] < 0.001 and closure["frame_rad"] < 0.001
    files = sorted(root.glob("[0-9][0-9]-*.png"))
    assert len(files) >= 8
    hashes = {}
    for file in files:
        assert file.read_bytes()[:8] == b"\x89PNG\r\n\x1a\n"
        metadata = json.loads(file.with_suffix(".capture.json").read_text())
        assert metadata  # emitted by the actual offscreen renderer
        hashes[file.name] = hashlib.sha256(file.read_bytes()).hexdigest()
    measurements = {m["capture"]: m for m in data["measurements"]}
    chase = measurements["06-launch-chase.png"]
    rider = measurements["07-launch-rider.png"]
    brake = measurements["08-brake-chase.png"]
    for measurement in measurements.values():
        assert measurement["device_force_n"] > 0
        pose = measurement["camera_and_pose"]
        assert pose["camera"] is not None
        assert pose["components"] and pose["instances"]
        assert max(j["error_m"] for j in pose["joints"]) < 0.001
    assert chase["camera_and_pose"]["camera"] != rider["camera_and_pose"]["camera"]
    assert hashes[chase["capture"]] != hashes[rider["capture"]]
    assert chase["camera_and_pose"]["components"] == rider["camera_and_pose"]["components"]
    rows = list(csv.DictReader((root / "physics.csv").open(newline="")))
    assert len(rows) == data["physics_samples"] == 1440
    for index, row in enumerate(rows):
        assert abs(float(row["time_s"]) - (index + 1) / 240) < 1e-8
    assert float(rows[239]["signed_speed_mps"]) > float(rows[0]["signed_speed_mps"]) + 1
    assert float(rows[1439]["signed_speed_mps"]) < float(rows[959]["signed_speed_mps"]) - 2
    assert len(data["supports"]) == 2
    assert data["supports"][0]["members"] == 3 and data["supports"][1]["members"] > 100
    report = {"verified": True, "captures": len(files), "physics_steps": len(rows),
              "distinct_cameras": True, "sha256": hashes}
    (root / "verification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "sha256"}))


if __name__ == "__main__":
    verify(sys.argv[1])
