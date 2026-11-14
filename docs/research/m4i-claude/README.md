# Preserved independent research inputs

These are byte-for-byte snapshots of Claude's pre-M4I read-only scratch
investigation, discovered on 2026-11-14. `manifest.json` records original relative
names, byte sizes, SHA-256 digests and retained locations. Original scratch files,
M4H worktree and older generated evidence were left untouched.

Compact source, Cargo manifests/locks, README files, summaries and analysis
outputs are versioned here. The four larger JSONL evidence files are also copied
to ignored `target/m4i/claude-evidence/` (about 45 MB total research inputs).
Their hashes are versioned, so a future missing raw file is detectable rather
than silently treated as reproduced evidence. Build products were not copied.
An additional complete, digest-verified local archive is outside Cargo's target
tree at `C:/Users/coast/.codex/research/quantum-x/m4i-claude-c39706b.zip`
(13,724,403 bytes). It contains all 18 research files plus the manifest, so a
future Cargo clean cannot erase the only retained raw evidence copy. It is a
local research backup, not a committed binary or an uploaded artifact.

Historical Rust/Cargo files retain absolute M4H paths and their original broad
scratch lint allowances. They are archival research, not workspace build inputs
or portable regression tests. No copied file is used as a production backend.
Use the new `source_guidance_m4i` tests and
`tools/verify-constraint-validation-m4i.py` for maintained reproduction.

The historical R8 README says “41 rows”; the emitted actual system has **65**
rows and 66 velocity variables. M4I checks this system directly. The archived
classifier also omits the final substep when summarizing residual maxima; M4I
includes the tick-563 failing substep explicitly. These archival discrepancies
are preserved, not edited away.
