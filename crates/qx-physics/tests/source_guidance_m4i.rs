//! M4I offline validation. All inherited modules and acceptance policies are controls.
#![cfg(feature = "experimental-source-guidance")]
#[path = "support/m4i_analytic.rs"]
mod analytic;
#[path = "support/m4i_articulated.rs"]
mod articulated;
#[allow(dead_code)]
#[path = "support/m4b_contract.rs"]
mod contract;
#[path = "support/m4i_convergence.rs"]
mod convergence;
#[allow(dead_code)]
#[path = "support/m4h_dynamics.rs"]
mod energy_dynamics;
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/m4e_c_extension.rs"]
mod fixtures;
#[allow(dead_code)]
#[path = "support/m4a_guidance.rs"]
mod guidance;
#[allow(dead_code)]
#[path = "support/m4a_matrix.rs"]
mod matrix;
#[allow(dead_code)]
#[path = "support/m4h_tracking.rs"]
mod measured_tracking;
#[allow(dead_code)]
#[path = "support/m4a_mechanisms.rs"]
mod mechanisms;
#[allow(dead_code)]
#[path = "support/m4b_dynamics.rs"]
mod original_dynamics;
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;
#[path = "support/m4i_references.rs"]
mod references;
#[allow(dead_code)]
#[path = "support/m4d_dynamics.rs"]
mod refined_dynamics;
#[path = "support/m4i_residuals.rs"]
mod residuals;
#[allow(dead_code)]
#[path = "support/m4h_surface.rs"]
mod surface;
#[allow(dead_code)]
#[path = "support/m4d_tracking.rs"]
mod tracking;
use contract::{Kind, Model};
use qx_geometry::{
    authoring::{InitialConditions, Section},
    heartline::AuthoredReference,
    math::Vec3,
    solver::{
        GenerationOptions,
        prepared::{SourceIdentity, dense::ContinuousGraph},
    },
    transition::Side,
};
use qx_physics::FIXED_STEP_SECONDS as DT;
use surface::Rail;
const ID: SourceIdentity = SourceIdentity {
    document: 0x4fb,
    generation: 1,
};
const G: Vec3 = Vec3::new(0., -9.81, 0.);
fn rail(initial: &InitialConditions, sections: &[Section]) -> Rail {
    Rail::new(
        ContinuousGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            initial,
            sections,
            GenerationOptions::default(),
        )
        .unwrap(),
        ID,
    )
    .unwrap()
}
fn model(r: &Rail, lead: f64, shared: bool) -> Model {
    let mut m = mechanisms::Mechanism::new(
        mechanisms::Path::Straight,
        lead,
        false,
        Vec3::new(0.05, 0.12, 0.03),
        shared,
    );
    for p in &mut m.poses {
        let local = Vec3::new(0., p.position_m.y, p.position_m.z);
        let e = r.source.evaluate(ID, p.position_m.x, Side::Right).unwrap();
        p.position_m = e.position_m + e.frame.local_to_world(local);
        p.orientation = e.frame.orientation();
    }
    Model::new(m)
}
fn fixture(name: &str) -> (InitialConditions, Vec<Section>, f64) {
    match name {
        "helix" => {
            let (_, i, s) = fixtures::fixtures().remove(2);
            (i, s, 8.025)
        }
        "foundation" => {
            let p = qx_document::sample::foundation_project();
            (p.initial, p.sections[..2].to_vec(), 11.625)
        }
        _ => panic!("unknown fixture"),
    }
}
fn array(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}
