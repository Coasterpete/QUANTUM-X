# R0.5 — native Windows desktop verification

R0.5 verifies the existing application on owner hardware. It changes no UI,
physics, timestep, numerical tolerances, golden fixtures or project schemas.
It implements the package/startup/native-smoke portion of the approved
[R0/Tier 3 roadmap](roadmaps/production-readiness-v1.md#9-laptop-development--desktop-distribution-verification-strategy).
[Tier 1 CI](ci.md) remains CPU-only; the Format job also runs verifier regression
tests in Windows PowerShell 5.1 and PowerShell 7. The experimental
[Tier 2 WARP workflow](software-gpu-r0.4.md) is unchanged and non-blocking.

## Run on either Windows computer

Use a trusted, fully extracted x64 Windows development package from a green
[CI run](https://github.com/Coasterpete/QUANTUM-X/actions/workflows/ci.yml).
Keep the complete versioned directory, including both executables and assets.
No Rust, Cargo, Python, Git, SDK, admin access or application rebuild is needed.
Windows PowerShell 5.1+ and its built-in .NET runtime are sufficient. Add-Type
loads the small C# pipe-draining helper; it does not compile QUANTUM Dynamics.
Organizational policies or Constrained Language Mode may prevent scripts,
Add-Type or unsigned binaries; the runner does not bypass those restrictions.

For an existing R0.2–R0.4 package, run the new script from this checkout:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\Dev\QUANTUM-X\tools\verify-desktop.ps1 `
  -PackageDirectory 'C:\Downloads\QUANTUM Dynamics-0.1.0-COMMIT-windows' `
  -OutputDirectory 'C:\scratch\qx-r05-desktop-run1' -MachineLabel discrete-desktop
```

Subsequent packages include the runner and helpers. From the extracted folder:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-desktop.ps1 `
  -PackageDirectory . -OutputDirectory 'C:\scratch\qx-r05-laptop-run1' -MachineLabel intel-laptop
```

Labels are supplied descriptions, not hardware detection or evidence of a test.
The default is `unspecified`. Use a fresh output folder for **every** run, outside
and non-overlapping with the package. Existing output folders and reparse/link
paths are refused. The runner never deletes evidence, alters the package, opens
an owner project or drives desktop input. It launches only existing noninteractive
diagnostics and hidden-window smokes. Preferences/recents use the new evidence
folder; experimental overrides are cleared only in child processes. Package
assets are explicit, so a checkout cannot supply missing content.

Optionally pass `-ExpectedManifestSha256 <64-hex-digest>` from the CI summary to
compare a separately obtained manifest digest. Hashes detect corruption and
accidental substitutions, not publisher authenticity. Run only trusted packages;
this is not an executable sandbox. The script does not query CI or infer that
the downloaded build's other jobs passed.

## Automated evidence

| Check | Required evidence |
| --- | --- |
| Package integrity | Trusted adjacent `verify-package.ps1` checks hashes, ordering, missing/extra/unsafe paths. Preflight rejects links, more than 10,000 entries, more than 4 GiB, manifests over 2 MiB and metadata over 64 KiB. Both executables, catalog and verified release metadata must exist. |
| Build identity/version | Matching executable hashes; `--build-info` agrees with packaged version, full commit/tree, build ID, target, profile and verified-build flag; `--version` agrees. No local Git HEAD is substituted for package provenance. |
| Catalog | `--self-check` exits successfully and produces the catalog report. |
| Startup routes | Public executable `--startup-check` reaches Welcome with no document; compatibility executable `--startup-check edit` reaches a blank Editor with a document. |
| Welcome/Editor native | Existing hidden HWND/wgpu smokes report at least five requested/presented frames, hardware adapter, successful diagnostic exit and released resources. |
| Simulator native | Existing synthetic-fixture Editor → Simulator → Editor script finishes, draws Simulator frames, runs at least 48 fixed ticks, Pauses, matches initial poses after Reset and restores the Editor. This is the existing same-window mode. |
| Windows/GPU | Allowlisted OS version/build/architecture and up to eight installed adapter names/vendor/driver versions/dates. Each native report separately identifies its selected adapter. Integrated and discrete GPUs are supported; CPU/software/unknown identities fail hardware evidence. |

Evidence directory contents:

- `report.json`: versioned automated results, provenance, inventory, per-command
  startup/exit/timeout/capture status, native reports and errors. Partial reports
  are saved after preflight and every case.
- `summary.md`: portable human-readable identity, hardware, integrity and results.
- `<case>.stdout.txt` / `.stderr.txt`: bounded, sanitized diagnostic streams.
- `manual-checklist.md` and `.json`: separate owner checklist, initialized entirely
  to `not_performed`. The runner never imports manual results or marks them passed.
- `state/`: isolated temporary application state; no existing user files copied.

Exit status is **0 only when all eight automated cases, integrity and inventory
pass**; otherwise **1**. Integrity or embedded-identity failure blocks subsequent
launches, recorded as `not_run`. Independent cases continue after later failures.
Invalid invocation/output paths are rejected before creating reports. Abrupt host
shutdown or script cancellation can leave the last partial report `running`;
that is not a pass.

CLI cases have a 30 s outer timeout; native cases have 45 s, in addition to the
unmodified 20 s soft/30 s watchdog deadlines. Override with
`-CommandTimeoutSeconds` / `-NativeTimeoutSeconds` (1–120 s) for diagnosis; this
does not change application deadlines. Pipes drain concurrently with only the
first 64 KiB retained per stream. Excess output fails incomplete evidence.
Timeouts attempt termination of only the spawned process tree, with additional
bounded termination/stream waits. No other application is killed. OS/GPU
inventory has a separate 30 s limit; failure fails the overall automated result.
Native failure JSON is retained even after a nonzero exit. `timed_out` covers
either outer termination or the application's own soft/watchdog report;
`outer_timed_out` and `application_timed_out` distinguish them and preserve the
reported failing stage.

The collector does not read existing logs, projects, profiles, network details,
hostname, serials or monitor EDID. Known local paths/profile/user/computer strings
and other drive-rooted paths are redacted from saved diagnostics; selected-adapter
PCI bus location is omitted. Sanitization is best effort, not a guarantee against
arbitrary secrets emitted by a trusted binary. Review before sharing; nothing
is uploaded automatically.

## Manual owner verification and limits

Follow the separate [manual checklist](../tools/desktop-manual-checklist.md).
Record only actually performed checks, observations, configurations, commit and
test time in generated manual JSON. Leave unperformed checks unchanged. Test
Save/Open/Undo/Redo only on new disposable projects or copies outside the package.
Owner records stay separate; automated `manual_status` stays `not_performed`.

Hidden-window presentation does not prove visible appearance, responsiveness,
button/shortcut usability, FPS, DPI, resize/minimize behavior, multi-monitor
behavior, visible corruption or safe normal shutdown with unsaved work. Those
checks require an owner at the desktop. No interactive Computer Use is performed.
Console behavior, missing persistent logs/recovery and the future separate
Simulator window are unchanged; these belong to later milestones.

Roadmap §9's wider campaigns are **not performed by this package-only runner**
and have explicit `not_run` entries:

- Existing `--gui-smoke-test simulator-timing <disposable-reference-project.qx.json> 12`
  remains an optional owner-hardware diagnostic. Use a trusted copied reference
  with a compatible train and capture its JSON separately. Worker timing is not
  visible FPS; R0.5 introduces no performance gate.
- Cross-machine golden replay remains the [R0.3 workflow](deterministic-regression-r03.md).
  Its test runner is not shipped in the app; invoking Cargo here would defeat
  package-only testing. A smoke does not prove cross-machine determinism.
- Install/uninstall is `not_applicable` until R9.

[Recorded hardware evidence](evidence/r0.5/README.md) identifies exactly which
machine/package ran. No coverage is inferred from labels, CI, WARP or an earlier
milestone. This does not qualify full rides/production physics or certify safety.

## Test without the application

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-desktop-verification.ps1
pwsh -NoProfile -File tools/test-desktop-verification.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-windows-packaging.ps1
cargo fmt --all -- --check
git diff --check
```

Tests create and clean only their uniquely named, absolute-path-validated temp
folder. Synthetic executable bytes are never run. Real child supervision uses
Windows PowerShell itself for failed launch, nonzero startup exit, timeout and
stdout/stderr floods. Synthetic reports test corrupt/missing packages/executables,
identity, false hardware/frame/reset claims, malformed/truncated evidence,
continued independent checks, isolation, privacy filtering and manual separation.
These are verifier tests, not hardware results.
