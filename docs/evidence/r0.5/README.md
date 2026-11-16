# R0.5 hardware evidence

R0.4 PR [#20](https://github.com/Coasterpete/QUANTUM-X/pull/20) was merged before
this branch was created from `c9a004771ba41b99847838e4072e44bcce2f326d`.
All four jobs in its [main CI run](https://github.com/Coasterpete/QUANTUM-X/actions/runs/43789445266)
were confirmed green before starting implementation.

The [initial discrete-GPU record](initial-discrete-gpu.json) comes from actually
running the downloaded package from that green run, without rebuilding. Its
times are UTC (the owner-local test date was 2026-11-15). This record predates
the final script review; it is evidence of the executed trial, not a claim that
every later verifier revision was rerun on hardware.

During later local testing the entry-point file disappeared and Windows denied
recreation of that exact path. No Windows Defender detection or Restart Manager
file lock was reported; the cause has not been established. Security settings
were not changed. The entry-point source is preserved in Git for review and
hosted verification; a final local hardware rerun is pending resolution.

| Configuration | Automated checks | Manual owner checks |
| --- | --- | --- |
| Windows 11 Pro 10.0.26300, RTX 4070, NVIDIA driver 32.0.16.1742; native Vulkan selected | Eight of eight passed; native Welcome/Editor presented 5 frames each; Simulator presented 21 frames, paused at tick 52, reset matched and Editor restored | not_performed |
| Intel integrated-GPU laptop | not_run during R0.5 | not_performed |

The package source commit, executable and manifest hashes, inventory and case
exit results are in the JSON. PCI bus location is omitted. Full local evidence
is under ignored `target/desktop-r05-local-main/`; logs, binaries and captures
are not committed. The optional independently supplied manifest hash was not
used for this initial run; internal manifest verification passed.

These are hidden-window native checks, not visible quality, FPS, responsiveness,
DPI, multi-monitor, project-editing or full-ride results. No active desktop input
was used. No R0.5 laptop coverage or cross-machine determinism is claimed.
See the [owner workflow and limitations](../../desktop-verification-r0.5.md).
