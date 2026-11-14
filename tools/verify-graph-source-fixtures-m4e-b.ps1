# Run actual qx-document fixtures without adding an experimental feature/dependency
# to qx-document or qx-physics. The standalone probe stays under ignored target/.
$ErrorActionPreference = 'Stop'
$repositoryPath = Split-Path -Parent $PSScriptRoot
$probePath = Join-Path $repositoryPath 'target/m4e-b/fixture-probe'
New-Item -ItemType Directory -Force (Join-Path $probePath 'src') | Out-Null
@'
[package]
name = "m4eb-fixture-probe"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
qx-document = { path = "../../../crates/qx-document" }
qx-geometry = { path = "../../../crates/qx-geometry", features = ["experimental-graph-source"] }
'@ | Set-Content -LiteralPath (Join-Path $probePath 'Cargo.toml') -Encoding utf8
@'
use qx_document::sample;
use qx_geometry::{heartline::AuthoredReference, solver::{GenerationOptions,prepared::{PreparedGraph,SourceIdentity}},transition::Side};
fn main() {
    let p=sample::foundation_project();let id=SourceIdentity {document:123,generation:1};
    for count in [2,3,5] {
        let result=PreparedGraph::prepare(id,AuthoredReference::TrackCenterline,&p.initial,&p.sections[..count],GenerationOptions::default());
        match result {
            Ok(source) => {
                println!("FOUNDATION count={count} length={} charts={} boundaries={:?}",source.length_m(),source.charts().len(),source.boundaries());
                for i in 0..997 {source.evaluate(id,source.length_m()*(i as f64+0.137)/997.,Side::Right).unwrap();}
                if count==2 {source.validate_interval(id,0.,source.length_m()).unwrap();}
                else {assert!(source.validate_interval(id,0.,source.length_m()).is_err());}
            }
            Err(e)=> {assert_eq!(count,5);println!("FOUNDATION count={count} rejected={e}");}
        }
    }
}
'@ | Set-Content -LiteralPath (Join-Path $probePath 'src/main.rs') -Encoding utf8
& cargo run --manifest-path (Join-Path $probePath 'Cargo.toml') --offline --release
if ($LASTEXITCODE -ne 0) { throw "Fixture probe failed: $LASTEXITCODE" }
