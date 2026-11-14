# Actual qx-document fixtures, with no feature/dependency changes to application
# crates. As in M4E-B, the generated Cargo probe stays in ignored target/.
$ErrorActionPreference = 'Stop'
$repositoryPath = Split-Path -Parent $PSScriptRoot
$probePath = Join-Path $repositoryPath 'target/m4e-c/fixture-probe'
New-Item -ItemType Directory -Force (Join-Path $probePath 'src') | Out-Null
@'
[package]
name = "m4ec-fixture-probe"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
qx-document = { path = "../../../crates/qx-document" }
qx-geometry = { path = "../../../crates/qx-geometry", features = ["experimental-graph-source"] }
'@ | Set-Content -LiteralPath (Join-Path $probePath 'Cargo.toml') -Encoding utf8
$supportPath = (Join-Path $repositoryPath 'crates/qx-geometry/tests/support/m4e_c_extension.rs').Replace('\', '/')
$header = '#[path = "' + $supportPath + '"] mod extension;'
$body = @'
use qx_document::sample;
use qx_geometry::{heartline::AuthoredReference, solver::prepared::PreparedGraph};
fn main() {
    let p = sample::foundation_project();
    extension::measure("foundation", &p.initial, &p.sections[..2], &[0.2, 0.1, 0.05]);
    let source = extension::prepare(&p.initial, &p.sections[..3], 0.1);
    assert!(!source.boundaries().last().unwrap().compatible);
    assert!(source.validate_interval(extension::ID, 0., source.length_m()).is_err());
    println!("M4EC_FOUNDATION_REJECT {:?}", source.boundaries().last().unwrap());
    assert!(PreparedGraph::prepare(extension::ID, AuthoredReference::TrackCenterline, &p.initial, &p.sections, extension::options(0.1)).is_err());
    // Compile and run the shared fixture code too; avoids dead-code allowances.
    assert_eq!(extension::fixtures().len(), 6);
}
'@
($header + "`n" + $body) | Set-Content -LiteralPath (Join-Path $probePath 'src/main.rs') -Encoding utf8
& cargo run --manifest-path (Join-Path $probePath 'Cargo.toml') --offline --release
if ($LASTEXITCODE -ne 0) { throw "M4E-C fixture probe failed: $LASTEXITCODE" }
