# Actual document fixture, isolated package; no application dependency changes.
$ErrorActionPreference = 'Stop'
$repositoryPath = Split-Path -Parent $PSScriptRoot
$probePath = Join-Path $repositoryPath 'target/m4f/fixture-probe'
New-Item -ItemType Directory -Force (Join-Path $probePath 'src') | Out-Null
@'
[package]
name = "m4f-fixture-probe"
version = "0.0.0"
edition = "2024"
[workspace]
[features]
default = ["experimental-graph-source"]
experimental-graph-source = []
[dependencies]
qx-document = { path = "../../../crates/qx-document" }
qx-geometry = { path = "../../../crates/qx-geometry", features = ["experimental-graph-source"] }
'@ | Set-Content -LiteralPath (Join-Path $probePath 'Cargo.toml') -Encoding utf8
$testPath = (Join-Path $repositoryPath 'crates/qx-geometry/tests/graph_dense_extension.rs').Replace('\', '/')
$header = '#[allow(dead_code, unused_imports)] #[path = "' + $testPath + '"] mod dense;'
$body = @'
fn main() {
    let p = qx_document::sample::foundation_project();
    dense::measure("foundation", &p.initial, &p.sections[..2]);
}
'@
($header + "`n" + $body) | Set-Content -LiteralPath (Join-Path $probePath 'src/main.rs') -Encoding utf8
& cargo run --manifest-path (Join-Path $probePath 'Cargo.toml') --offline --release
if ($LASTEXITCODE -ne 0) { throw "M4F fixture probe failed: $LASTEXITCODE" }
