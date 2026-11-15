"""Read-only SHA-256/inventory check for the immutable R0.3 fixture corpora.

No update mode: additions and changed expectations require deliberate review.
Only Python's standard library is needed. Run from any working directory.
"""
import hashlib
import json
from pathlib import Path

if not __debug__:
    raise RuntimeError("Fixture verification requires assertions: remove -O/PYTHONOPTIMIZE")

ROOT = Path(__file__).resolve().parents[1]
CORPORA = (
    ROOT / "crates/qx-document/tests/fixtures/compatibility",
    ROOT / "crates/qx-physics/tests/fixtures/replay",
)


def verify(directory):
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["format_version"] == 1, f"{directory}: unsupported manifest"
    hashes = manifest["sha256"]
    actual = {p.name for p in directory.iterdir() if p.is_file()} - {"manifest.json"}
    assert set(hashes) == actual, f"{directory}: missing/extra corpus files"
    assert not any(p.is_symlink() or p.is_dir() for p in directory.iterdir())
    for name, expected in hashes.items():
        assert Path(name).name == name and "/" not in name and "\\" not in name
        data = (directory / name).read_bytes()
        assert b"\r" not in data, f"{name}: fixture bytes must use LF"
        digest = hashlib.sha256(data).hexdigest()
        assert digest == expected, f"{name}: changed immutable bytes ({digest} != {expected})"
    if "cases" in manifest:
        cases = manifest["cases"]
        assert {case["schema"] for case in cases} == set(range(1, 8))
        declared = [case[key] for case in cases for key in ("input", "expected")]
        assert len(declared) == len(set(declared))
        assert set(declared) == set(hashes), "unexercised compatibility fixture"
        for case in cases:
            assert case["provenance"]
            source = json.loads((directory / case["input"]).read_text())
            expected = json.loads((directory / case["expected"]).read_text())
            assert source["schema_version"] == case["schema"]
            assert expected["schema_version"] == 7
    print(f"FIXTURES {directory.relative_to(ROOT).as_posix()}: {len(hashes)} immutable files verified")


if __name__ == "__main__":
    for corpus in CORPORA:
        verify(corpus)
